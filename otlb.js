const configuration = {
  // Replace [_] and [X] with unicode checkboxes
  prettifyTodoBoxes: true,
  // How large does a child page need to be to get folded.
  // Set to 0 to fold all child articles.
  foldThreshold: 8,
};

function filename() {
  let name = window.location.pathname;
  name = name.slice(name.lastIndexOf('/') + 1);
  name = name.slice(0, name.indexOf('.'));
  return name;
}

function wikiWordSpaces(wikiWord) {
  if (!wikiWord.match(/([A-Z][a-z0-9]+){2,}/)) {
    return wikiWord;
  }
  let ret = '';
  for (let i = 0; i < wikiWord.length; i += 1) {
    ret += wikiWord[i];
    if (wikiWord[i + 1]) {
      if (wikiWord[i + 1].match(/([A-Z])/)) {
        ret += ' ';
      } else if (wikiWord[i + 1].match(/[0-9]/) && !wikiWord[i].match(/[0-9]/)) {
        ret += ' ';
      }
    }
  }
  return ret;
}

function extractTags(lines) {
  let ret = {};
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i].match(/^\t*(([A-Z][a-z0-9]+){2,})$/)) {
      const word = lines[i].replace(/\s*/, '');
      ret[word] = `#${word}`;
    }
  }
  return ret;
}

/**
 * Partially parsed wrapper for lines in file.
 *
 * Can be constructed without knowledge of neighboring lines.
 * */
class Line {
  constructor(text) {
    this.depth = 0;
    while (text[this.depth] === '\t') { this.depth += 1; }

    const bodyText = text.slice(this.depth);

    const rules = [
      [/^ /,         ' ', 1, false],  // Wrapped text (leading space)
      [/^:\S/,       ':', 1, true],   // Wrapped text type
      [/^&gt;\S/,    '>', 4, true],
      [/^:( |$)/,    ':', 2, false],  // Wrapped text body
      [/^&gt;( |$)/, '>', 5, false],
      [/^;\S/,       ';', 1, true],   // Preformatted text type
      [/^&lt;\S/,    '<', 4, true],
      [/^;( |$)/,    ';', 2, false],  // Preformatted text body
      [/^&lt;( |$)/, '<', 5, false],
      [/^\|/,        '|', 0, false],  // Table
    ];
    for (let i = 0; i < rules.length; i += 1) {
      if (bodyText.match(rules[i][0])) {
        this.prefix = rules[i][1];
        this.body = bodyText.slice(rules[i][2]);
        this.isFirst = rules[i][3];
        return;
      }
    }
    // Normal outline lines
    this.prefix = ''
    this.body = bodyText;
    this.isFirst = true;
  }

  isPreformatted() {
    return this.prefix === ';' || this.prefix === '<' || this.prefix === '|';
  }

  isWrapping() {
    return this.prefix === ' ' || this.prefix === '>' || this.prefix === ':';
  }

  // Can the next line be joined after this to make a single entity
  joinsWith(nextLine) {
    return typeof nextLine !== 'undefined'
      && !nextLine.isFirst
      && nextLine.depth === this.depth
      && nextLine.prefix === this.prefix;
  }

  title() {
    if (this.prefix !== '') {
      return null;
    }
    return wikiWordSpaces(this.body);
  }

  /** Do in-place formatting */
  format(tags) {
    function formatLineSegment(input) {
      function format(str, ...args) {
        return str.replace(/\$(\d+)/g, (match, number) => {
          if (typeof args[number] !== 'undefined') {
            return args[number];
          }
          return match;
        });
      }

      function splice(match, replace) {
        const head = input.slice(0, match.index);
        const tail = input.slice(match.index + match[0].length);
        return formatLineSegment(head, tags)
          + format(replace, ...match)
          + formatLineSegment(tail, tags);
      }

      let match;
      if (match = /`(.*?)`/.exec(input)) {
        // Escape formatting.
        return splice(match, '<code>$1</code>');
      }
      if (match = /\b(https?|ftp):\/\/[-A-Z0-9+&@#\/%?=~_|!:,.;()]*[-A-Z0-9+&@#\/%=~_|()]/i.exec(input)) {
        // Hyperlink
        return splice(match, '<a href="$0">$0</a>');
      }
      if (match = /!\[(.*?)\]/.exec(input)) {
        // Inline image
        return splice(match, `<img src="${match[1]}" />`);
      }
      if (match = /\[(\.\.?\/.*?)\]/.exec(input)) {
        // Local link
        return splice(match, `<a href="${match[1]}">${match[1]}</a>`);
      }
      if (match = /\b(([A-Z][a-z0-9]+){2,})\b/.exec(input)) {
        // Wikiword
        const wikiWord = match[1];
        if (tags && wikiWord in tags) {
          return splice(match, `<a href="${tags[wikiWord]}">$0</a>`);
        }
        return splice(match, '<span class="undefined-word">$0</span>');
      }
      return input;
    }

    if (this.isPreformatted()) {
      // Treat preformatted boxes as verbatim text
      // Don't show the metadata line at all.
      if (this.isFirst) { return ''; }
      return `<code>${this.body}</code><br/>`;
    }

    let line = this.body;

    if (this.isWrapping()) {
      // Metadata line again.
      if (this.isFirst) { return ''; }
      // Paragraph break on empty line
      if (line.match(/^\s*$/)) { return '</p><p>'; }
    }

    if (!this.isWrapping() && line.match(/^\s*-{4,}\s*$/)) {
      // hline
      return '<hr/>'
    }

    // Escape HTML.
    // (Don't do it for the ' ' block prefix that we use by convention on
    // the HTML footer so it'll stay invisible)
    if (this.prefix !== ' ') {
      line = line.replace(/<(.*?)>/g, '&lt;$1&gt;');
    }

    if (!this.isWrapping()) {
      // Prettify votl todo boxes
      if (configuration.prettifyTodoBoxes) {
        line = line.replace(/^\[_\] /, '☐');
        line = line.replace(/^\[X\] /, '☑');
      }

      if (line.match(/^(([A-Z][a-z0-9]+){2,})$/)) {
        // Only a WikiWord on a line, this is a heading.
        return `<span id="${line}"><a class="modlink" href="#${line}">${this.title()}</a></span>`;
      }
    }

    line = formatLineSegment(line);

    if (this.prefix === '' && line.match(/ \*$/)) {
      // Important topic
      line = `<mark>${line.replace(/ \*$/, '')}</mark>`;
    }

    return line;
  }
}

class Entity {
  // Return [parsedEntity, newIdx]
  static parse(lines, startIdx, tags) {
    let ret = new Entity();
    let bodyLines;
    let currentDepth;
    let pos = 0;

    if (startIdx === -1) {
      // Special case, process the entire file.
      bodyLines = [];
      currentDepth = -1;
    } else {
      bodyLines = [lines[startIdx]];
      currentDepth = bodyLines[0].depth;

      for (pos = startIdx + 1; lines[pos - 1].joinsWith(lines[pos]); pos += 1) {
        bodyLines.push(lines[pos]);
      }
    }

    let numVisibleLines = bodyLines.length;

    let children = [];
    while (lines[pos] && lines[pos].depth > currentDepth) {
      const [child, newPos] = Entity.parse(lines, pos, tags);
      child.parent = ret;
      pos = newPos;
      children.push(child);
    }

    let doc = document.createDocumentFragment();

    ret.prefix = bodyLines.length > 0 ? bodyLines[0].prefix : null;
    let doc2 = doc.appendChild(document.createElement('div'));
    let doc3 = doc2;
    if (ret.prefix === '') {
      doc3 = doc2.appendChild(document.createElement('div'));
      doc3.setAttribute('class', 'hanging');
    }

    const html = bodyLines.map(x => x.format(tags)).join('\n');
    if (bodyLines.length > 0 && bodyLines[0].isWrapping()) {
      let p = doc3.appendChild(document.createElement('p'));
      p.innerHTML = html;
    } else {
      doc3.innerHTML = html;
    }

    if (children.length > 0) {
      let list = doc2.appendChild(document.createElement('ul'));
      for (let i = 0; i < children.length; i += 1) {
        let item = list.appendChild(document.createElement('li'));
        if (children[i].isGoodArticle() && children[i].numVisibleLines > configuration.foldThreshold) {
          // Non-stub article shown folded.
          const title = children[i].title;
          item.innerHTML =
            `<strong id="${title}">+<a class="modlink" href="#${title}">${wikiWordSpaces(title)}</a></strong>`;

          // XXX: Looks like you have to attach the element somewhere or it
          // can't be displayed. Attaching it to a dummy element here.
          const dummy = document.createElement('li');
          dummy.appendChild(children[i].doc);
          children[i].doc = dummy.children[0];

          numVisibleLines += 1;
        } else {
          item.appendChild(children[i].doc);
          // Rebind
          children[i].doc = item.children[0];

          numVisibleLines += children[i].numVisibleLines;
        }
      }
      doc2.appendChild(list);
    }

    let title = '';
    if (startIdx === -1) {
      title = filename();
    } else if (bodyLines.length === 1 && bodyLines[0].prefix === '') {
      title = bodyLines[0].body;
    }

    ret.title = title;
    ret.doc = doc;
    ret.children = children;
    ret.isToplevel = startIdx === -1;
    ret.numVisibleLines = numVisibleLines;
    ret.parent = null;
    return [ret, pos];
  }

  // Does node have no children?
  isStub() { return this.children.length === 0; }

  isArticle() { return this.title.match(/^([A-Z][a-z0-9]+){2,}$/); }

  // Non-stub with valid title
  isGoodArticle() {
    return !this.isStub() && this.isArticle();
  }

  parentArticle() {
    if (!this.parent) { return null; }
    if (this.parent.isArticle() || this.parent.isToplevel) { return this.parent; }
    return this.parent.parentArticle();
  }

  // Show as titled article
  asArticle() {
    let article = this;
    if (this.isArticle() && this.isStub()) {
      // Reroute stubs to their parent article.
      let parent = this.parentArticle();
      if (parent) {
        article = parent;
        parent = article.parentArticle();
      }
    }

    let doc = document.createDocumentFragment();
    if (article.title) {
      let h = doc.appendChild(document.createElement('h1'));

      // Construct links to parent articles
      let text = wikiWordSpaces(article.title);
      for (let parent = article.parentArticle(); parent; parent = parent.parentArticle()) {
        if (parent.isToplevel) {
          text = `<a class="modlink" href="">/</a>${text}`;
        } else {
          text = `<a class="modlink" href="#${parent.title}">/</a>${text}`;
        }
      }
      h.innerHTML = text;
    }
    if (article.title && !article.isToplevel) {
      // Title is derived from the topmost item in non-toplevel entities,
      // don't repeat it in the body.
      if (article.doc.lastElementChild
          && article.doc.lastElementChild.tagName === 'UL') {
        doc.appendChild(article.doc.lastElementChild.cloneNode(true));
      }
    } else {
      doc.appendChild(article.doc.cloneNode(true));
    }
    return doc;
  }

  findTitle(title) {
    if (!title) {
      return null;
    }

    if (this.title === title) {
      return this;
    }
    for (let i = 0; i < this.children.length; i += 1) {
      const ret = this.children[i].findTitle(title);
      if (ret) {
        return ret;
      }
    }
    return null;
  }
}

function otlb(document) {
  // Split document to lines for processing.
  let lines = document.getElementsByTagName('body')[0].innerHTML.split(/\r?\n/);
  let tags = extractTags(lines);
  // Convert text lines to Line objects.
  lines = lines.filter(x => !x.match(/^\s*$/)).map(x => new Line(x));
  let topLevel = Entity.parse(lines, -1, tags)[0];

  function onHashChanged() {
    document.body.innerHTML = '';

    const hash = window.location.hash.slice(1);
    let subPage = topLevel.findTitle(hash);

    let article = topLevel;
    if (subPage) {
      article = subPage;
    }

    document.body.appendChild(article.asArticle());
    if (article.title) {
      document.title = wikiWordSpaces(article.title);
    } else {
      document.title = 'Otlbook';
    }

    const elt = document.getElementById(hash);
    if (elt) { elt.scrollIntoView(); }
  }

  document.head.appendChild(document.createElement('style')).innerHTML = `
  body{margin:auto;max-width:50em;
  white-space:normal !important;
  font-family:"Noto Sans",Verdana,sans-serif !important;}
  code{white-space:pre;}
  h1{text-align:center;}
  p{font-family:"Times New Roman",Times,serif;
  text-indent:1em;margin-top:0.2em;margin-bottom:0.2em;color:#333}
  .hanging{text-indent:-0.5em;padding-left:0.5em;}
  .modlink{text-decoration:none;color:green;}
  ul{padding-left:1em;line-height:1.5;list-style-type:none;list-style outside;}
  ul ul{margin-left:1em;border-left:1px solid #CCC;}
  .undefined-word {color: Red;}
  `;

  window.onhashchange = onHashChanged;
  onHashChanged();
}

otlb(document);
