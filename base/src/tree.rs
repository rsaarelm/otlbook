use std::{
    collections::VecDeque,
    convert::TryFrom,
    fmt,
    hash::Hash,
    ops::{Deref, DerefMut},
    sync::{self, Arc, RwLock, Weak},
};

// Inspired by rctree in https://github.com/SimonSapin/rust-forest/

// XXX: The dirty flag handling is a bit too manual and state-wrangly. Will
// probably cause bugs at some point.

#[derive(Default)]
struct Node<T> {
    pub data: T,

    parent: Option<Weak<RwLock<Node<T>>>>,
    child: Option<Arc<RwLock<Node<T>>>>,
    sibling: Option<Arc<RwLock<Node<T>>>>,
    /// Starts out false, set to true when data is first borrowed mutably.
    dirty: bool,
}

impl<T> From<T> for Node<T> {
    fn from(data: T) -> Self {
        Node {
            data,
            parent: Default::default(),
            child: Default::default(),
            sibling: Default::default(),
            dirty: false,
        }
    }
}

/// Reference-counted tree data structure.
#[derive(Default)]
pub struct NodeRef<T>(Arc<RwLock<Node<T>>>);

impl<T> Clone for NodeRef<T> {
    fn clone(&self) -> Self {
        NodeRef(self.0.clone())
    }
}

impl<T> From<&Arc<RwLock<Node<T>>>> for NodeRef<T> {
    fn from(r: &Arc<RwLock<Node<T>>>) -> Self {
        NodeRef(r.clone())
    }
}

impl<T> From<Arc<RwLock<Node<T>>>> for NodeRef<T> {
    fn from(r: Arc<RwLock<Node<T>>>) -> Self {
        NodeRef(r)
    }
}

impl<T> From<Node<T>> for NodeRef<T> {
    fn from(n: Node<T>) -> Self {
        NodeRef(Arc::new(RwLock::new(n)))
    }
}

impl<T> TryFrom<&Weak<RwLock<Node<T>>>> for NodeRef<T> {
    type Error = ();

    fn try_from(w: &Weak<RwLock<Node<T>>>) -> Result<Self, Self::Error> {
        Ok(NodeRef::from(w.upgrade().ok_or(())?))
    }
}

impl<T> From<T> for NodeRef<T> {
    fn from(data: T) -> Self {
        Node::from(data).into()
    }
}

impl<T> NodeRef<T> {
    /// Immutable access to node data.
    ///
    /// Will panic if node is already mutably borrowed.
    pub fn borrow(&self) -> Ref<T> {
        Ref(self.0.read().unwrap())
    }

    /// Mutable access to node data.
    ///
    /// Will panic if any borrows to node already exist.
    ///
    /// Calling this will mark the node as dirty, regardless of whether node
    /// data is actually changed.
    pub fn borrow_mut(&self) -> RefMut<T> {
        let mut node = self.0.write().unwrap();
        node.dirty = true;
        RefMut(node)
    }

    /// Mark the tree as dirty.
    pub fn taint(&self) {
        self.borrow_mut();
    }

    /// Mark the tree as clean.
    pub fn cleanse(&self) {
        self.0.write().unwrap().dirty = false;
        for c in self.children() {
            c.cleanse();
        }
    }

    /// Return parent of node, if any.
    pub fn parent(&self) -> Option<NodeRef<T>> {
        self.0
            .read()
            .unwrap()
            .parent
            .as_ref()
            .map(|w| NodeRef::try_from(w).ok())
            .flatten()
    }

    /// Return first child of node, if any.
    pub fn child(&self) -> Option<NodeRef<T>> {
        self.0
            .read()
            .unwrap()
            .child
            .as_ref()
            .map(|r| NodeRef::from(r))
    }

    /// Return next sibling of node, if any.
    pub fn sibling(&self) -> Option<NodeRef<T>> {
        // Only report sibling if parent is still valid.
        if self.parent().is_some() {
            self.0
                .read()
                .unwrap()
                .sibling
                .as_ref()
                .map(|r| NodeRef::from(r))
        } else {
            None
        }
    }

    /// Detach node from its parent and sibling.
    pub fn detach(&self) {
        if let Some(parent) = self.parent() {
            if parent.child().expect("Invalid tree state").ptr() == self.ptr() {
                // Detaching first child, second child is new first child.
                parent.0.write().unwrap().child =
                    self.0.read().unwrap().sibling.clone();
            } else {
                let mut n = parent.child();
                while let Some(node) = n {
                    // Detaching a sibling, cut from the chain.
                    if node.sibling().map(|n| n.ptr()) == Some(self.ptr()) {
                        node.0.write().unwrap().sibling =
                            self.0.read().unwrap().sibling.clone();
                        break;
                    } else {
                        n = node.sibling();
                    }
                }
            }
        }

        {
            let mut node = self.0.write().unwrap();
            node.parent = None;
            node.sibling = None;
        }
    }

    /// Insert node as first child.
    pub fn prepend(&self, child: NodeRef<T>) {
        child.detach();
        {
            let mut child = child.0.write().unwrap();
            child.parent = Some(Arc::downgrade(&self.0));
            child.sibling = self.0.read().unwrap().child.clone();
        }
        self.taint();
        self.0.write().unwrap().child = Some(child.0.clone());
    }

    /// Insert node as last child.
    pub fn append(&self, child: NodeRef<T>) {
        match self.child() {
            None => self.prepend(child),
            Some(mut node) => {
                self.taint();
                child.detach();
                child.0.write().unwrap().parent = Some(Arc::downgrade(&self.0));

                while let Some(next) = node.sibling() {
                    node = next;
                }
                node.0.write().unwrap().sibling = Some(child.0.clone());
            }
        }
    }

    /// Breadth first iteration of a tree's nodes.
    pub fn iter(&self) -> BreadthFirstNodes<T> {
        BreadthFirstNodes {
            next: Some(self.clone()),
            pending: Default::default(),
        }
    }

    /// Iterate through immediate children of this node.
    pub fn children(&self) -> impl Iterator<Item = NodeRef<T>> {
        let mut n = self.child();
        std::iter::from_fn(move || {
            if let Some(node) = n.as_ref().map(|n| n.clone()) {
                n = node.sibling();
                Some(node)
            } else {
                None
            }
        })
    }

    /// Determine whether any node in tree has been changed after creation.
    ///
    /// Can be expensive to query as long as tree isn't dirty.
    pub fn is_dirty(&self) -> bool {
        if self.0.write().unwrap().dirty == true {
            return true;
        }

        for n in self.children() {
            if n.is_dirty() {
                // Dirtyfy this node as well, subsequent queries will be fast.
                self.0.write().unwrap().dirty = true;
                return true;
            }
        }

        false
    }

    /// Helper method for comparing by pointer identity.
    fn ptr(&self) -> *const RwLock<Node<T>> {
        &*(self.0)
    }
}

impl<T: Clone> NodeRef<T> {
    /// Return a detached deep copy of the current node.
    pub fn deep_clone(&self) -> Self {
        let data: T = self.borrow().clone();
        let ret = NodeRef::from(data);
        for child in self.children() {
            ret.append(child.deep_clone());
        }
        ret
    }
}

impl<T: PartialEq> PartialEq for NodeRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter()
            .zip(other.iter())
            .all(|(a, b)| *a.borrow() == *b.borrow())
    }
}

impl<T: Eq> Eq for NodeRef<T> {}

impl<T: Hash> Hash for NodeRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.iter().for_each(|n| n.borrow().hash(state));
    }
}

impl<T: fmt::Display> fmt::Display for NodeRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print<'a, T: fmt::Display>(
            f: &mut fmt::Formatter<'_>,
            indent: usize,
            node: &NodeRef<T>,
        ) -> fmt::Result {
            for _ in 0..indent {
                write!(f, "  ")?;
            }
            writeln!(f, "{}", *node.borrow())?;
            let mut n = node.child();
            while let Some(node) = n {
                print(f, indent + 1, &node)?;
                n = node.sibling();
            }
            Ok(())
        }

        print(f, 0, self)
    }
}

/// Wrapper to `Deref` directly into node data.
pub struct Ref<'a, T: 'a>(sync::RwLockReadGuard<'a, Node<T>>);

impl<'a, T> Deref for Ref<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0.data
    }
}

/// Wrapper to `DerefMut` directly into node data.
pub struct RefMut<'a, T: 'a>(sync::RwLockWriteGuard<'a, Node<T>>);

impl<'a, T> Deref for RefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0.data
    }
}

impl<'a, T> DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.data
    }
}

/// Standard tree iterator.
pub struct BreadthFirstNodes<T> {
    pub(crate) next: Option<NodeRef<T>>,
    pub(crate) pending: VecDeque<NodeRef<T>>,
}

impl<T> Iterator for BreadthFirstNodes<T> {
    type Item = NodeRef<T>;

    fn next(&mut self) -> Option<Self::Item> {
        // If out of siblings, start iterating the first node from the pending
        // list.
        if self.next.is_none() {
            self.next = self.pending.pop_front();
        }

        // Push next node's first child to pending list, move next node cursor
        // to its next sibling and yield the next node.
        if let Some(node) = self.next.as_ref().map(|n| n.clone()) {
            self.next = node.sibling();
            if let Some(child) = node.child() {
                self.pending.push_back(child);
            }
            Some(node)
        } else {
            None
        }
    }
}
