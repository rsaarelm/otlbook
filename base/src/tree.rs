use std::{cell::Cell, fmt};

pub struct Node<'a, T: 'a> {
    pub data: T,

    parent: Cell<Option<&'a Node<'a, T>>>,
    child: Cell<Option<&'a Node<'a, T>>>,
    sibling: Cell<Option<&'a Node<'a, T>>>,
}

// TODO: Children iter (VecDeque for pending nodes)
// TODO: Breadth-first iter
//
// TODO: Back and forth type conv between Section and Node

impl<'a, T: fmt::Display> fmt::Display for Node<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print<'a, T: fmt::Display>(
            f: &mut fmt::Formatter<'_>,
            indent: usize,
            node: &Node<'a, T>,
        ) -> fmt::Result {
            for _ in 0..indent {
                write!(f, "  ")?;
            }
            writeln!(f, "{}", node.data)?;
            let mut n = node.child();
            while let Some(node) = n {
                print(f, indent + 1, node)?;
                n = node.sibling();
            }
            Ok(())
        }

        print(f, 0, self)
    }
}

/// Shorthand for converting pointer to reference to pointer for comparing ref
/// equality.
fn p<T>(a: &T) -> *const T {
    a as *const T
}

impl<'a, T> Node<'a, T> {
    pub fn new(data: T) -> Node<'a, T> {
        Node {
            data,

            parent: Default::default(),
            child: Default::default(),
            sibling: Default::default(),
        }
    }

    pub fn parent(&self) -> Option<&'a Node<'a, T>> {
        self.parent.get()
    }

    pub fn child(&self) -> Option<&'a Node<'a, T>> {
        self.child.get()
    }

    pub fn sibling(&self) -> Option<&'a Node<'a, T>> {
        self.sibling.get()
    }

    /// Remove node from parent node it's attached to, if any.
    pub fn detach(&self) {
        let parent = self.parent.take();
        let next = self.sibling.take();

        if let Some(parent) = parent {
            if parent.child().map(p) == Some(p(self)) {
                parent.child.set(next);
            } else {
                let mut n = parent.child();
                while let Some(node) = n {
                    if node.sibling().map(p) == Some(p(self)) {
                        node.sibling.set(next);
                        break;
                    } else {
                        n = node.sibling();
                    }
                }
            }
        }
    }

    pub fn prepend(&'a self, child: &'a Node<'a, T>) {
        child.detach();
        child.parent.set(Some(self));
        child.sibling.set(self.child());
        self.child.set(Some(child));
    }

    pub fn append(&'a self, child: &'a Node<'a, T>) {
        match self.child() {
            None => self.prepend(child),
            Some(mut node) => {
                child.detach();
                child.parent.set(Some(self));

                while let Some(next) = node.sibling() {
                    node = next;
                }
                node.sibling.set(Some(child));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree() {
        let arena = typed_arena::Arena::new();
        let new = |name: &str| arena.alloc(Node::new(name.to_string()));

        let root = new("Root");
        let a = new("A");
        root.append(a);
        root.append(new("B"));

        println!("{}", root);
        a.detach();
        println!("{}", root);

        assert!(false);
    }
}
