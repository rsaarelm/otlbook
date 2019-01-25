function format(str, ...args) {
  return str.replace(/\$(\d+)/g, (match, number) => {
    if (typeof args[number] !== 'undefined') {
      return args[number];
    }
    return match;
  });
}

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

function depth(line) {
  let i = 0;
  while (line[i] === '\t') { i += 1; }
  return [i, line.slice(i)];
}

// Return [prefix or null, remaining line, remaining line is user block type]
function blockPrefix(line) {
  const line2 = line.replace(/^\t*/, '');

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
    if (line2.match(rules[i][0])) {
      return [rules[i][1], line2.slice(rules[i][2]), rules[i][3]];
    }
  }
  return [null, line2, false];
}

function isPreformattedBlock(prefix) {
  return prefix === ';' || prefix === '<' || prefix === '|';
}

function isWrappingBlock(prefix) {
  return prefix === ' ' || prefix === '>' || prefix === ':';
}

function formatLineSegment(input, tags) {
  function splice(match, replace) {
    const head = input.slice(0, match.index);
    const tail = input.slice(match.index + match[0].length);
    return formatLineSegment(head, tags) + format(replace, ...match) + formatLineSegment(tail, tags);
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
  if (match = /\b(([A-Z][a-z0-9]+){2,})\b/.exec(input)) {
    // Wikiword
    const wikiWord = match[1];
    if (wikiWord in tags) {
      return splice(match, `<a href="${tags[wikiWord]}">$0</a>`);
    }
    return splice(match, '<span class="undefined-word">$0</span>');
  }
  if (match = /!\[(.*?)\]\((.*?)\)/.exec(input)) {
    // Inline image
    return splice(match, `<img alt="${match[1]}" src="${match[2]}" />`);
  }

  return input;
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

function processLines(lines, tags) {
  let currentDepth = 0;
  let ret = [];
  let currentBlockPrefix = null;

  ret.push(`<h1>${wikiWordSpaces(filename())}</h1>`);
  ret.push('<ul>');
  for (let i = 0; i < lines.length; i += 1) {
    let [lineDepth, line] = depth(lines[i]);
    let doNotFormat = false;

    if (lineDepth !== currentDepth && currentBlockPrefix) {
      // Exit block when depth changes.
      ret.push(isPreformattedBlock(currentBlockPrefix) ? '</pre></li>' : '</p></li>');
      currentBlockPrefix = null;
    }

    while (lineDepth > currentDepth) {
      ret.push('<ul style="list-style-type:none">');
      currentDepth += 1;
    }
    while (lineDepth < currentDepth) {
      ret.push('</ul>');
      currentDepth -= 1;
    }

    const [linePrefix, lineText, isUserType] = blockPrefix(line);
    if (linePrefix !== currentBlockPrefix) {
      // User block boundary.
      if (currentBlockPrefix) {
        // Out from the previus one.
        ret.push(isPreformattedBlock(currentBlockPrefix) ? '</li>' : '</p></li>');
      }
      if (linePrefix) {
        ret.push(isPreformattedBlock(linePrefix) ? '<li>' : '<li><p>');
      }
      currentBlockPrefix = linePrefix;
    }
    line = lineText;
    if (isUserType) {
      // This is metadata for the block formatter, we don't want to show
      // it.
      continue;
    }

    // Escape HTML.
    // (Don't do it for the ' ' block prefix that we use by convention on
    // the HTML footer so it'll stay invisible)
    if (linePrefix !== ' ') {
      line = line.replace(/<(.*?)>/g, '&lt;$1&gt;');
    }

    // Prettify votl todo boxes
    line = line.replace(/^(\t*)\[_\] /, '$1☐');
    line = line.replace(/^(\t*)\[X\] /, '$1☑');

    if (isWrappingBlock(linePrefix) && line.match(/^\s*$/)) {
      // Paragraph break on empty line
      ret.push('</p><p>');
      continue;
    }

    if (line.match(/^\t*(([A-Z][a-z0-9]+){2,})$/)) {
      // Wiki caption, add an anchor.
      line = line.replace(/\s*/, '');
      line = `<strong id="${line}">${wikiWordSpaces(line)}</strong>`;
      doNotFormat = true;
    }

    if (!doNotFormat) {
      // Match wikiwords etc. items
      line = formatLineSegment(line, tags);
    }

    if (linePrefix && isPreformattedBlock(linePrefix)) {
      line = `<code>${line}</code><br/>`;
    } else if (!linePrefix) {
      if (line.match(/ \*$/)) {
        // Important item
        line = `<mark>${line.replace(/ \*$/, '')}</mark>`;
      }

      line = `<li class="hanging">${line}</li>`;
    }

    ret.push(line);
  }

  if (currentBlockPrefix) {
    // Out from the previus one.
    ret.push(isPreformattedBlock(currentBlockPrefix) ? '</pre>' : '</p>');
  }
  // XXX: If the file ends in deep nesting, there should be multiple
  // list closings here. Though in practice we can just be sloppy and leave
  // the end-of-document tags unclosed.
  ret.push('</ul>');

  return ret;
}

// Split document to lines for processing.
let lines = document.getElementsByTagName('body')[0].innerHTML.split(/\r?\n/);
lines = processLines(lines, extractTags(lines));
document.getElementsByTagName('body')[0].innerHTML = lines.join('\n');
document.title = wikiWordSpaces(filename());

// Replace the initial plaintext style with our own.
if (document.styleSheets.length > 0) {
  document.styleSheets[0].disabled = true;
}
let sheet = document.createElement('style')
sheet.innerHTML = `
body{margin:auto;max-width:50em;
 font-family:"Noto Sans",Verdana,sans-serif;}
code{white-space:pre;}
h1{text-align:center;}
p{font-family: "Times New Roman",Times,serif;
 text-indent:1em;margin-top:0.2em;margin-bottom:0.2em;color:#333}
.hanging{text-indent:-1em;padding-left:1em;}
ul{padding-left:0.5em;line-height:1.3;list-style-type:none;list-style outside;}
ul ul{margin-left:0.5em;border-left:1px solid #CCC;}
.undefined-word {color: Red;}
`;
document.body.appendChild(sheet);
