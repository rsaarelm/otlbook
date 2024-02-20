#!/usr/bin/env python3

import pandas as pd
import sys

df = pd.read_csv(sys.argv[1])

df = df.sort_values(by='created', ascending=True)

for i, r in df.iterrows():
    print(r['title'])
    print("\t:uri", r['url'])
    print("\t:added", r['created'])
    if isinstance(r['tags'], str):
        print("\t:tags", r['tags'].replace(',', ''))
    if isinstance(r['excerpt'], str):
        for line in r['excerpt'].splitlines():
            print("\t%s" % line)
    if isinstance(r['note'], str):
        for line in r['note'].splitlines():
            print("\t%s" % line)

