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
    basename = path.split('/')[-1].split('.')[0]
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
    print("Wrote tagfile %s/tags" % os.getcwd(), file=sys.stderr)

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
        if line_text.startswith('; ') or line_text == ';':
            # Start a new user block if we're not already in one with the
            # correct indentation.
            if not isinstance(ret[-1], SimpleNamespace) or ret[-1].depth != depth:
                ret.append(SimpleNamespace(name='', depth=depth, text=[]))

            ret[-1].text.append(line_text[2:])
        elif line_text.startswith(';'):
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
                ret.append('%s;%s' % (indent, i.name))
            for line in i.text:
                ret.append(('%s; %s' % (indent, line)).rstrip(' \n\t'))
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
            if re.match(r'^\.?j-lib\b', i.name):
                # J-library code, append to trail for the code sectors.
                trail.extend(i.text)
            elif re.match(r'^\.?j\b', i.name):
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
