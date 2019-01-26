const configuration = {
  // Replace [_] and [X] with unicode checkboxes
  prettifyTodoBoxes: true,
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
      ret[word] = `#/${word}`;
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
      [/^:/, ':', 1, false],              // Wrapped text
      [/^ /, ' ', 1, false],              // Wrapped text (leading space)
      [/^;/, ';', 1, false],              // Preformatted text
      [/^&gt;\S+/, '>', 4, true],         // User-defined wrapped text type
      [/^&gt;( |$)/, '>', 5, false],      // User-defined wrapped text body
      [/^&lt;\S+/, '<', 4, true],         // User preformatted text type
      [/^&lt;( |$)/, '<', 5, false],      // User preformatted text body
      [/^\|/, '|', 0, false],             // Table
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
      if (match = /\[(\.\/.*?)\]/.exec(input)) {
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
      if (this.isFirst) {
        // Don't show the metadata line at all.
        return '';
      }
      return `<code>${this.body}</code><br/>`;
    }

    let line = this.body;

    if (this.isWrapping() && line.match(/^\s*$/)) {
      // Paragraph break on empty line
      return '</p><p>';
    }

    // Escape HTML.
    // (Don't do it for the ' ' block prefix that we use by convention on
    // the HTML footer so it'll stay invisible)
    if (this.prefix !== ' ') {
      line = line.replace(/<(.*?)>/g, '&lt;$1&gt;');
    }

    // Prettify votl todo boxes
    if (configuration.prettifyTodoBoxes) {
      line = line.replace(/^\[_\] /, '☐');
      line = line.replace(/^\[X\] /, '☑');
    }

    if (line.match(/^(([A-Z][a-z0-9]+){2,})$/) || (tags && line in tags)) {
      // Either a WikiWord or explicitly marked as heading.
      return `<strong id="${line}"><a class="modlink" href="#/${line}">${this.title()}</a></strong>`;
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
        item.appendChild(children[i].doc);
        // Rebind
        children[i].doc = item.children[0];
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
    ret.parent = null;
    return [ret, pos];
  }

  // Does node have no children?
  isStub() { return this.children.length === 0; }

  // Non-stub with valid title
  isGoodArticle() {
    return !this.isStub() && this.title.match(/^([A-Z][a-z0-9]+){2,}$/);
  }

  // Show as titled article
  asArticle() {
    let doc = document.createDocumentFragment();
    if (this.title) {
      let h = doc.appendChild(document.createElement('h1'));
      if (this.isToplevel) {
        h.innerText = wikiWordSpaces(this.title);
      } else {
        h.innerHTML = `<a class="modlink" href="#${this.title}">/</a>${wikiWordSpaces(this.title)}`;
      }
    }
    if (this.title && !this.isToplevel) {
      // Title is derived from the topmost item in non-toplevel entities,
      // don't repeat it in the body.
      if (this.doc.lastElementChild
          && this.doc.lastElementChild.tagName === 'UL') {
        doc.appendChild(this.doc.lastElementChild.cloneNode(true));
      }
    } else {
      doc.appendChild(this.doc.cloneNode(true));
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

  function applyStyle() {
    const style = `
    body{margin:auto;max-width:50em;
    font-family:"Noto Sans",Verdana,sans-serif;}
    code{white-space:pre;}
    h1{text-align:center;}
    p{font-family:"Times New Roman",Times,serif;
    text-indent:1em;margin-top:0.2em;margin-bottom:0.2em;color:#333}
    .hanging{text-indent:-1em;padding-left:1em;}
    .modlink{text-decoration:none;color:green;}
    ul{padding-left:0.5em;line-height:1.3;list-style-type:none;list-style outside;}
    ul ul{margin-left:0.5em;border-left:1px solid #CCC;}
    .undefined-word {color: Red;}
    `;
    let sheet = document.getElementsByTagName("style")[0];
    if (!sheet) {
      sheet = document.body.appendChild(document.createElement('style'));
    }
    sheet.innerHTML = style;
  }

  function onHashChanged() {
    // #/: show entire document
    // #/PageTitle: show subpage for PageTitle
    // #PageTitle: show entire document scrolled to PageTitle
    // default: show front page if there's a valid one, otherwise full page
    document.body.innerHTML = '';
    const hash = window.location.hash.slice(1);

    let frontPage = null;
    if (topLevel.children.length > 0 && topLevel.children[0].isGoodArticle()) {
      frontPage = topLevel.children[0];
    }

    // If not hash and frontpage show frontpage
    let fullPage = false;

    let subPage = null;
    if (hash[0] === '/') {
      subPage = topLevel.findTitle(hash.slice(1));
    }

    let article;
    if (subPage) {
      article = subPage;
    } else if (frontPage && hash === '') {
      article = frontPage;
    } else {
      article = topLevel;
    }

    document.body.appendChild(article.asArticle());
    if (article.title) {
      document.title = wikiWordSpaces(article.title);
    } else {
      document.title = 'Otlbook';
    }

    applyStyle();

    if (hash[0] !== '/') {
      const elt = document.getElementById(hash);
      if (elt) { elt.scrollIntoView(); }
    }
  }

  document.title = wikiWordSpaces(filename());

  // Replace the initial plaintext style with our own.
  if (document.styleSheets.length > 0) {
    document.styleSheets[0].disabled = true;
  }

  window.onhashchange = onHashChanged;
  onHashChanged();
}

otlb(document);
