#!/usr/bin/env python3

# Tagfile generator and toolkit for wiki-like VimOutliner files.

from collections import namedtuple
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from types import SimpleNamespace

# JavaScript logic for realtime page formatting.
VOTL_JS = r"""
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
    if (match = /`.*?`/.exec(input)) {
        // Escape formatting.
        return splice(match, '<code>$0</code>');
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
                ret.push(isPreformattedBlock(currentBlockPrefix) ? '</pre></li>' : '</p></li>');
            }
            if (linePrefix) {
                ret.push(isPreformattedBlock(linePrefix) ? '<li><pre>' : '<li><p>');
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

        if (!linePrefix) {
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
sheet.innerHTML = ".undefined-word {color: Red;}";
document.body.appendChild(sheet);
"""

class Tag(namedtuple('Tag', 'name path line')):
    def ctag_line(self):
        # ctags format: http://ctags.sourceforge.net/FORMAT
        if self.line == 0:
            # Line 0 means the tag refers to the entire file,
            # just point ctags to the start of the file
            ex = '0'
        else:
            ex = r'/^\t\*%s$/' % self.name
        return '%s\t%s\t%s' % (self.name, self.path, ex)

def otl_files(path='.'):
    for root, dir, files in os.walk(path):
        for f in files:
            if f.endswith('.otl.html') or f.endswith('.otl'):
                ret = os.path.join(root, f)
                # Remove the explicit local dir prefix so we get clean filenames
                if ret.startswith('./'):
                    ret = ret[2:]
                yield ret

def file_tags(path):
    basename = path.split('.')[0]
    # File name is WikiWord, file itself is a tag destination for that word.
    if re.match(r'^(([A-Z][a-z0-9]+){2,})$', basename):
        yield Tag(basename, path, 0)

    with open(path) as f:
        for (i, line) in enumerate(f):
            i += 1
            # WikiWord as the only content on a whole line, tag destination.
            name = re.match(r'^\t*(([A-Z][a-z0-9]+){2,})$', line)
            if name:
                name = name.group(1)
                yield Tag(name, path, line)

def build_tags():
    for f in otl_files():
        for t in file_tags(f):
            yield t

def write_tags():
    tags = list(build_tags())

    # Generate ctags
    ctags = sorted(list({t.ctag_line() for t in tags}))
    with open('tags', 'w') as f:
        f.write('\n'.join(ctags))

    # Generate tags json for votl.js
    tag_json = {}
    for t in tags:
        if not t.name in tag_json or t.line == 0:
            # There might be multiple matches and we only set one.
            # Files take precedence over in-file tags.
            if t.line != 0:
                path = '%s#%s' % (t.path, t.name)
            else:
                path = t.path
            tag_json[t.name] = path
    with open('votl.js', 'w') as f:
        f.write('// *** GENERATED FILE, DO NOT EDIT ***\n')
        f.write('TAGS = ')
        f.write(json.dumps(tag_json, indent=2))
        f.write('\n')
        f.write(VOTL_JS)

def split_user_blocks(input):
    def line_depth(line):
        ret = 0
        while line and line[0] == '\t':
            ret += 1
            line = line[1:]
        return ret

    # Start with a dummy value so the first line can look at ret[-1]
    ret = [None]

    # Turn input into sequences of regular strings (other text) and
    # (block_name, block_depth, block_text)
    for line in input:
        line = line.rstrip(' \n\t')
        depth = line_depth(line)
        line_text = re.sub(r'^\t*', '', line)
        if line_text.startswith('< ') or line_text == '<':
            # Start a new user block if we're not already in one with the
            # correct indentation.
            if not isinstance(ret[-1], SimpleNamespace) or ret[-1].depth != depth:
                ret.append(SimpleNamespace(name='', depth=depth, text=[]))

            ret[-1].text.append(line_text[2:])
        elif line_text.startswith('<'):
            # No trailing space, so this will always start a new block.
            ret.append(SimpleNamespace(name=line_text[1:], depth=depth, text=[]))
        else:
            # Other text
            if isinstance(ret[-1], list):
                ret[-1].append(line)
            else:
                ret.append([line])

    # Drop the dummy value in index 0 when returning
    return ret[1:]

def join_user_blocks(seq):
    ret = []
    for i in seq:
        if isinstance(i, list):
            ret.extend(i)
        else:
            assert(isinstance(i, SimpleNamespace))
            indent = '\t' * i.depth
            if i.name:
                ret.append('%s<%s' % (indent, i.name))
            for line in i.text:
                ret.append(('%s< %s' % (indent, line)).rstrip(' \n\t'))
    return '\n'.join(ret)

def format_j_code(lines):
    # Makes sure user input is indented three columns, wipes out generated
    # output.
    ret = []
    for line in lines:
        # Generated lines are marked with trailing NBSP. Skip them.
        if line.endswith('\u00A0'):
            continue
        # Insufficiently indented lines get extra three spaces of indentation,
        # J session convention is to have user input indented 3 columns.
        if not line.startswith('   '):
            line = '   ' + line
        ret.append(line)
    return ret

def hash_j_code(lines):
    code = []
    for line in lines:
        line = re.sub(r'NB\..*$', '', line).strip()
        if not line:
            continue
        line = re.sub(r'\s+', ' ', line)
        code.append(line)
    return hashlib.md5(bytes('\n'.join(code), 'utf-8')).hexdigest()

def eval_j_code(seq):
    def exec(code):
        """Execute J to get output lines for given program input"""
        p = None
        for name in ('ijconsole', 'jconsole'):
            if shutil.which(name):
                p = subprocess.Popen(
                        ('/usr/bin/env', name),
                        stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE)
                break
        if not p:
            print(
                "Couldn't find ijconsole or jconsole, please install a J programming language interpreter",
                file=sys.stderr)
            sys.exit(1)
        output = str(p.communicate(bytes(code, 'utf-8'))[0], 'utf-8')
        ret = []
        junk_output = True
        for line in output.splitlines():
            if junk_output:
                if not '\u241E' in line:
                    continue
                else:
                    line = line.split('\u241E')[-1]
                    junk_output = False
            if not line.strip():
                continue

            # The first output line will have the three input lines for the
            # interactive prompt printed to it, these need to be removed.
            if not len(ret) and line.startswith('   '):
                line = line[3:]
            # Tag the line with the output marker so we can remove it on the
            # next pass.
            ret.append(line.rstrip() + '\u00A0')
        return ret

    trail = []
    # Only support J language for now.
    for i in seq:
        if isinstance(i, SimpleNamespace):
            # Optional # padding to prevent HTML tag like formations
            if re.match(r'^#?j-lib\b', i.name):
                # J-library code, append to trail for the code sectors.
                trail.extend(i.text)
            elif re.match(r'^#?j\b', i.name):
                # Executable code!

                # Look for a hash of the previously evaluated code. If there
                # is one and it's an exact match for our current code, we can
                # skip the entire (possibly expensive) evaluation.
                cached_digest = re.search(r'md5:(.+?)\b', i.name)
                if cached_digest:
                    cached_digest = cached_digest[1]

                formatted = format_j_code(i.text)

                code_lines = trail + formatted
                digest = hash_j_code(code_lines)

                # XXX Kludge around whitespace junk generated by jconsole. Add
                # the echo command for the separator marker so we can eat all
                # the junk output up until that.
                if formatted and formatted[-1].strip() != ')':
                    code_lines.insert(-1, "echo '\u241E'")

                if digest == cached_digest:
                    # Looks like we already evaluated this exact code here.
                    # Do nothing this time around.
                    continue

                # Otherwise we need to actually evaluate the code
                output = exec('\n'.join(code_lines))
                i.text = formatted + output
                # Update the hash in the name.
                i.name = ' '.join(x for x in i.name.split() if not x.startswith('md5:'))
                i.name += ' md5:%s' % digest

if __name__ == '__main__':
    cmd = len(sys.argv) > 1 and sys.argv[1] or 'tags'
    if cmd == 'eval':
        blocks = split_user_blocks(sys.stdin)
        eval_j_code(blocks)
        print(join_user_blocks(blocks))
        sys.exit(0)
    elif cmd == 'tags':
        write_tags()
    else:
        print('Usage %s (tags|eval)' % sys.argv[0])
        sys.exit(1)
