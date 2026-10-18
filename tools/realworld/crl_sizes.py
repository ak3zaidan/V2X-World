#!/usr/bin/env python3
"""WP-F2: the engine's 10,000-entry linkage CRL, decoded by the A0 oracle, against Brecht's figure.

Input: the file ``V2XW_CRL_OUT=<file> cargo test -p v2xw-sec --test crl_size_brecht`` writes
(the COER ``CrlContents`` of a series-1 full linkage CRL with 10,000 revoked devices).
The oracle (asn1tools over the 1609.2-2022 modules) must decode it, re-encode it byte for byte,
and find exactly 10,000 IndividualRevocation entries, each with two distinct 16-byte seeds.
It then prints the size against Brecht et al. (IEEE T-ITS 2018 section VI-F): about 40 B per
entry and about 400 KB per 10,000 entries.

Usage (project venv): python -I tools/realworld/crl_sizes.py FILE
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))
from scms_oracle import coer  # noqa: E402

BRECHT_PER_ENTRY = 40
BRECHT_10K = 400_000


def main(path):
    data = open(path, 'rb').read()
    bad = []
    c = coer.decode('CrlContents', data)
    if coer.encode('CrlContents', c) != data:
        bad.append('oracle re-encoding differs')
    kind, body = c['typeSpecific']
    if kind != 'fullLinkedCrl':
        bad.append(f'typeSpecific {kind}')
    n = groups = 0
    seeds = set()
    for jg in body.get('individual', []):
        for lg in jg['contents']:
            for ig in lg['contents']:
                groups += 1
                for e in ig['contents']:
                    n += 1
                    for s in (e['linkageSeed1'], e['linkageSeed2']):
                        if len(s) != 16:
                            bad.append('a seed is not 16 bytes')
                        seeds.add(s)
    if n != 10_000:
        bad.append(f'{n} entries, not 10,000')
    if len(seeds) != 2 * n:
        bad.append(f'{len(seeds)} distinct seeds for {n} entries')
    per = len(data) / max(n, 1)
    if not per <= BRECHT_PER_ENTRY:
        bad.append(f'{per:.2f} B per entry exceeds Brecht')
    for b in bad:
        print('RED', b)
    print(f'crl: {"PASS" if not bad else "FAIL"} ({len(data)} B for {n} entries in {groups} iMax groups, '
          f'{per:.2f} B per entry; Brecht about {BRECHT_PER_ENTRY} B and {BRECHT_10K // 1000} KB)')
    return 0 if not bad else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
