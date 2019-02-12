#!/usr/bin/env python3

# Tagfile generator and toolkit for wiki-like VimOutliner files.

from collections import namedtuple
from types import SimpleNamespace
from urllib.error import URLError
from urllib.request import urlopen
import argparse
import hashlib
import json
import os
import os.path
import re
import shutil
import subprocess
import sys
import time
import urllib

class OtlNode:
    @staticmethod
    def from_file(path):
        """Parse a file into a VimOutliner tree

        The toplevel node will have the filename without an extension as
        content."""
        toplevel_name = os.path.basename(path).split('.')[0]
        with open(path) as f:
            result = OtlNode(list(f), name=toplevel_name)
        return result

    def __init__(self, lines, line_idx=-1, parent=None, name=None):
        def depth(line):
            depth = 0
            while line[0] == '\t':
                depth += 1
                line = line[1:]
            return depth, line.rstrip()

        self.line_number = line_idx + 1
        self.parent = parent
        self.children = []

        if line_idx >= 0:
            self.depth, self.text = depth(lines[line_idx])
        else:
            # Special case for whole file
            self.text = None
            self.depth = -1
            assert(not self.parent)
            if name is not None:
                self.text = name

        i = line_idx + 1
        while i < len(lines):
            d, _ = depth(lines[i])
            if d > self.depth:
                child = OtlNode(lines, i, self)
                self.children.append(child)
                i += len(child)
            else:
                break

        # Only first children can describe aliases
        # TODO: Allow @tags lines in header block too
        in_header_block = True
        for c in self.children:
            if c.alias_name:
                if not in_header_block:
                    c.alias_name = None
            else:
                in_header_block = False

        # Wikiword if this node describes that.
        self.wiki_name = None
        if self.text and re.match(r'^(([A-Z][a-z0-9]+){2,})$', self.text):
            self.wiki_name = self.text

        # Wiki alias if thes node describes that.
        self.alias_name = None
        match = self.text and re.match(r'\(([^()\s]+)\)$', self.text)
        if match:
            self.alias_name = match.group(1)

    def __len__(self):
        self_len = 0
        if self.depth >= 0: self_len = 1
        return self_len + sum(len(c) for c in self.children)

    def anki_cards(self):
        """If this node describes an Anki card, generate the card value."""
        clozes = self.text and re.split(r'{{(.*?)}}', self.text)
        is_item = self.text and self.text[0] not in (':', ';', '<', '>')

        # Answers end with period, not ellipsis though (not endswith('..'))
        if is_item and self.text and len(self.children) == 1 and self.text.endswith('?') \
                and self.children[0].text.endswith('.') and not self.children[0].text.endswith('..'):
            # Regular question-answer pairs.
            yield {'front': self.text, 'back': self.children[0].text}
        elif is_item and not self.children and clozes and len(clozes) > 1 \
                and self.text.endswith('.') and not self.text.endswith('..'):
            # Clozes.
            assert(len(clozes) % 2 == 1)

            for skip_idx in range(1, len(clozes), 2):
                parts = clozes[:]
                parts[skip_idx] = '...'
                front = ''.join(parts)
                back = ''.join(clozes)
                yield {'front': front, 'back': back}
        else:
            for c in self.children:
                yield from c.anki_cards()

    def ctag_lines(self, path):
        """Yield ctags lines for this entry and children."""
        SEARCH_EX = r'/^\t\*%s$/'

        if self.wiki_name:
            if self.line_number == 0:
                # Line 0 means the tag refers to the entire file,
                # just point ctags to the start of the file
                ex = '0'
            else:
                ex = SEARCH_EX % self.text
            yield '%s\t%s\t%s' % (self.wiki_name,
                    path,
                    SEARCH_EX % self.wiki_name)
        if self.alias_name and self.parent and self.parent.wiki_name:
            # Redirect aliases to parent
            yield '%s\t%s\t%s' % (self.alias_name,
                    path,
                    SEARCH_EX % self.parent.wiki_name)

        for c in self.children:
            yield from c.ctag_lines(path)

def otl_files(path='.'):
    for root, dir, files in os.walk(path):
        for f in files:
            if f.endswith('.otl.html') or f.endswith('.otl'):
                ret = os.path.join(root, f)
                # Remove the explicit local dir prefix so we get clean filenames
                if ret.startswith('./'):
                    ret = ret[2:]
                yield ret

### Tagsfile generator #########################################

def build_tags():
    for path in otl_files():
        for t in OtlNode.from_file(path).ctag_lines(path):
            yield t

def write_tags():
    tags = list(build_tags())

    # Generate ctags
    ctags = sorted(list(tags))
    with open('tags', 'w') as f:
        f.write('\n'.join(ctags))
    print("Wrote tagfile %s/tags" % os.getcwd(), file=sys.stderr)

### Interactive J notebook #####################################

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

def eval_j_code(seq, force=False):
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

                if not force and digest == cached_digest:
                    # Looks like we already evaluated this exact code here.
                    # Do nothing this time around.
                    continue

                # Otherwise we need to actually evaluate the code
                output = exec('\n'.join(code_lines))
                i.text = formatted + output
                # Update the hash in the name.
                i.name = ' '.join(x for x in i.name.split() if not x.startswith('md5:'))
                i.name += ' md5:%s' % digest

### Anki deck builder ##########################################

def generate_cards():
    cards = []
    for path in otl_files():
        with open(path) as f:
            cards.extend(OtlNode(list(f)).anki_cards())
    return cards

def upload_cards(cards):
    if cards:
        print("Updating Anki deck with %s cards" % len(cards), file=sys.stderr)
        with AnkiConnection() as anki:
            anki.update_deck(cards)
            anki.sync()

class AnkiConnection:
    """Anki deck modification tool."""
    class RestError(Exception):
        def __init__(self, msg): super().__init__(msg)

    class AnkiError(Exception):
        def __init__(self, msg): super().__init__(msg)

    def __init__(self, url='http://localhost:8765'):
        self.url = url
        self.p = None

        # Start an anki process if one is needed.
        try:
            urlopen(self.url)
            # Anki is already running, work with that.
        except URLError:
            # Run Anki just for you.
            self.p = subprocess.Popen('anki')
            # Let's check that it actually has the add-on
            try:
                time.sleep(3)
                urlopen('http://localhost:8765')
            except URLError:
                raise AnkiConnection.AnkiError(
                        "Can't connect to AnkiConnect. Do you have the AnkiConnect add-on installed?")

    def __enter__(self):
        return self

    def __exit__(self, type, value, traceback):
        self.p and self.p.kill()

    def _anki_connect_call(self, name, **kwargs):
        req_json = json.dumps({'version': 6, 'action': name, 'params': kwargs})
        ret = json.load(urlopen(self.url, req_json.encode('utf-8')))
        if ret.get('error'):
            raise AnkiConnection.RestError(ret['error'])
        else:
            return ret['result']

    def __getattr__(self, name):
        # Turn unknown methods into REST calls to AnkiConnect
        return lambda **kwargs : self._anki_connect_call(name, **kwargs)

    def cards(self):
        ids = self.findCards(query='deck:current')
        suspends = self.areSuspended(cards=ids)
        infos = self.cardsInfo(cards=ids)
        assert(len(ids) == len(suspends) == len(infos))
        for (id, suspended, info) in zip(ids, suspends, infos):
            assert(info['cardId'] == id)
            tags = set(self.notesInfo(notes=[info['note']])[0]['tags'])
            yield SimpleNamespace(
                    id=id,
                    note_id=info['note'],
                    is_suspended=suspended,
                    tags=tags,
                    front=info['fields']['Front']['value'],
                    back=info['fields']['Back']['value'])

    def update_deck(self, input_cards):
        """Given input [{'front': question, 'back': answer}], make Anki deck consist of these cards.

        Existing cards with fronts not in input will be suspended. Cards in
        deck with the same front but different back will have their back
        updated.
        """

        ids = self.findCards(query='deck:current')
        info = self.cardsInfo(cards=ids)
        fronts = [info[i]['fields']['Front']['value'] for i in range(len(ids))]

        # Suspended existing cards by front text.
        suspended = self.areSuspended(cards=ids)
        suspended = {fronts[i] for i in range(len(fronts)) if suspended[i]}

        # Existing cards indexed by front text.
        deck = dict((info[i]['fields']['Front']['value'],
            SimpleNamespace(
                card_id=ids[i],
                note_id=info[i]['note'],
                back=info[i]['fields']['Back']['value'])) for i in range(len(ids)))

        # Incoming new cards
        suspend_fronts = set(fronts).difference(suspended)
        unsuspend_ids = set()
        for c in input_cards:
            front, back = c['front'], c['back']

            if front in suspended:
                unsuspend_ids.add(deck[front].card_id)
            if not front in deck:
                # Add new note.
                self.addNote(
                        note={
                            'deckName': 'Default',
                            'modelName': 'Basic',
                            'fields': {
                                'Front': front,
                                'Back': back
                            },
                            'options': {'allowDuplicate': False},
                            'tags': []
                        })
            else:
                if front in suspend_fronts:
                    suspend_fronts.remove(front)
                if deck[front].back != back:
                    print("Updating card '%s' to have back '%s'" % (front, back), file=sys.stderr)
                    # Note exists but the answer has changed.
                    self.updateNoteFields(
                            note={
                                'id': deck[front].note_id,
                                'fields': {
                                    'Front': front,
                                    'Back': back
                                }
                            })

        # Suspend cards not in input, make sure cards in input are
        # unsuspended.
        for c in suspend_fronts:
            print("Live card '%s' not found in input, suspending" % c, file=sys.stderr)
        suspend_ids = {deck[front].card_id for front in suspend_fronts}
        self.unsuspend(cards=list(unsuspend_ids))
        self.suspend(cards=list(suspend_ids))

def main():
    parser = argparse.ArgumentParser(description="Otlbook utility kit")
    subparsers = parser.add_subparsers(dest='cmd')

    tags = subparsers.add_parser('tags', help="Generate tags file")

    j_eval = subparsers.add_parser('eval', help="Evaluate interactive J notebook")
    j_eval.add_argument('--force',
            action='store_true',
            help="Ignore cached checksums and re-evaluate everything")

    anki = subparsers.add_parser('anki', help="Import embedded flashcards to Anki")
    anki.add_argument('--dump',
            action='store_true',
            help="Print tab-separated plaintext export instead of uploading to Anki")

    args = parser.parse_args()

    if args.cmd == 'tags':
        write_tags()
    elif args.cmd == 'eval':
        blocks = split_user_blocks(sys.stdin)
        eval_j_code(blocks, force=args.force)
        print(join_user_blocks(blocks))
    elif args.cmd == 'anki':
        cards = generate_cards()
        if args.dump:
            for c in cards:
                front = c['front'].replace('\t', ' ')
                back = c['back'].replace('\t', ' ')
                print("%s\t%s\t" % (front, back))
        else:
            upload_cards(cards)
    else:
        parser.print_help()
        sys.exit(1)

if __name__ == '__main__':
    main()
