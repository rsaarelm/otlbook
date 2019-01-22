// *** GENERATED FILE, DO NOT EDIT ***
TAGS = {
  "JayNotebook": "JayNotebook.otl.html",
  "OtlBookIntro": "OtlBookIntro.otl.html",
  "WikiWord": "OtlBookIntro.otl.html#WikiWord",
  "OtlBoilerplate": "OtlBookIntro.otl.html#OtlBoilerplate",
  "OtlbookTips": "OtlBookIntro.otl.html#OtlbookTips"
}

if (!String.prototype.format) {
    String.prototype.format = function() {
        var args = arguments;
        return this.replace(/\$(\d+)/g, function(match, number) {
            return typeof args[number] != 'undefined' ? args[number] : match;
        });
    };
}

function filename() {
    var name = window.location.pathname;
    name = name.slice(name.lastIndexOf('/')+1);
    name = name.slice(0, name.indexOf('.'));
    return name;
}

function wikiWordSpaces(wikiWord) {
    if (!wikiWord.match(/([A-Z][a-z0-9]+){2,}/)) {
        return wikiWord;
    }
    var bits = wikiWord.split(/([A-Z])/);
    var ret = bits[1] + bits[2];
    for (var i = 3; i < bits.length; i += 2) {
        ret += ' '+bits[i]+bits[i+1];
    }
    return ret;
}

function depth(line) {
    var i = 0;
    for (; line[i] == '\t'; i++) {}
    return [i, line.slice(i)];
}

// Return [prefix or null, remaining line, remaining line is user block type]
function blockPrefix(line) {
    var match;
    line = line.replace(/^\t*/, '');

    var rules = [
        [/^:/, ':', 1, false],              // Wrapped text
        [/^ /, ' ', 1, false],              // Wrapped text (leading space)
        [/^;/, ';', 1, false],              // Preformatted text
        [/^&gt;\S+/, '>', 4, true],         // User-defined wrapped text type
        [/^&gt;( |$)/, '>', 5, false],      // User-defined wrapped text body
        [/^&lt;\S+/, '<', 4, true],         // User preformatted text type
        [/^&lt;( |$)/, '<', 5, false],      // User preformatted text body
        [/^\|/, '|', 0, false],             // Table
    ]
    for (var i = 0; i < rules.length; i++) {
        if (line.match(rules[i][0])) {
            return [rules[i][1], line.slice(rules[i][2]), rules[i][3]];
        }
    }
    return [null, line, false];
}

function isPreformattedBlock(prefix) {
    return prefix == ';' || prefix == '<' || prefix == '|';
}

function isWrappingBlock(prefix) {
    return prefix == ' ' || prefix == '>' || prefix == ':';
}

function formatLineSegment(input) {
    function splice(match, replace) {
        var head = input.slice(0, match.index);
        var tail = input.slice(match.index + match[0].length);
        return formatLineSegment(head) + replace.format(...match) + formatLineSegment(tail)
    }

    var match;
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
        var wikiWord = match[1];
        if (wikiWord in TAGS) {
            return splice(match, '<a href="'+TAGS[wikiWord]+'">$0</a>');
        } else {
            return splice(match, '<span class="undefined-word">$0</span>');
        }
    }
    if (match = /!\[(.*?)\]\((.*?)\)/.exec(input)) {
        // Inline image
        return splice(match, '<img alt="'+match[1]+'" src="'+match[2]+'" />');
    }

    return input;
}

function processLines(lines) {
    var currentDepth = 0;
    var ret = [];
    var currentBlockPrefix = null;

    ret.push('<h1>'+wikiWordSpaces(filename())+'</h1>');
    ret.push('<ul style="list-style-type:none">');
    for (var i = 0; i < lines.length; i++) {
        var [lineDepth, line] = depth(lines[i]);
        var depthChanged = false;
        var doNotFormat = false;

        if (lineDepth != currentDepth && currentBlockPrefix) {
            // Exit block when depth changes.
            ret.push(isPreformattedBlock(currentBlockPrefix) ? '</pre></li>' : '</p></li>');
            currentBlockPrefix = null;
        }

        while (lineDepth > currentDepth) {
            ret.push('<ul style="list-style-type:none">');
            currentDepth += 1;
            depthChanged = true;
        }
        while (lineDepth < currentDepth) {
            ret.push('</ul>');
            currentDepth -= 1;
            depthChanged = true;
        }

        var [linePrefix, lineText, isUserType] = blockPrefix(line);
        if (linePrefix != currentBlockPrefix) {
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
        if (linePrefix != ' ') {
            line = line.replace(/<(.*?)>/g, '&lt;$1&gt;');
        }

        // Prettify votl todo boxes
        line = line.replace(/^(\t*)\[_\] /, '$1☐')
        line = line.replace(/^(\t*)\[X\] /, '$1☑')

        if (isWrappingBlock(linePrefix) && line.match(/^\s*$/)) {
            // Paragraph break on empty line
            ret.push("</p><p>");
            continue;
        }

        if (line.match(/^\t*(([A-Z][a-z0-9]+){2,})$/)) {
            // Wiki caption, add an anchor.
            line = line.replace(/\s*/, '');
            line = '<strong id="'+line+'">'+wikiWordSpaces(line)+'</strong>';
            doNotFormat = true;
        }

        if (!doNotFormat) {
            // Match wikiwords etc. items
            line = formatLineSegment(line);
        }

        if (linePrefix && isPreformattedBlock(linePrefix)) {
            line = '<code>' + line + '</code><br/>';
        } else if (!linePrefix) {
            if (line.match(/ \*$/)) {
                // Important item
                line = '<mark>' + line.replace(/ \*$/, '') + '</mark>';
            }

            line = '<li>' + line + '</li>';
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
var lines = document.getElementsByTagName('body')[0].innerHTML.split(/\r?\n/);
lines = processLines(lines);
document.getElementsByTagName('body')[0].innerHTML = lines.join('\n');
document.title = wikiWordSpaces(filename());

// Replace the initial plaintext style with our own.
if (document.styleSheets.length > 0) {
    document.styleSheets[0].disabled = true;
}
var sheet = document.createElement('style')
sheet.innerHTML = `
body{margin:auto;max-width:50em;
 font-family:"Noto Sans",Verdana,sans-serif;}
code{white-space:pre;}
h1{text-align:center;}
p{font-family: "Times New Roman",Times,serif;
 margin-top:0.2em;margin-bottom:0.2em;color:#333}
ul{padding-left:1em;line-height:1.3;}
.undefined-word {color: Red;}
`;
document.body.appendChild(sheet);
