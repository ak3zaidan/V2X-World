#!/usr/bin/env python3
"""SCMS oracle command line (WP-A0).  Prints a short verdict; exit status 0 only when everything holds.

    oracle.py selftest            reproduce every vector in crates/v2xw-sec/tests/vectors
    oracle.py mutate              inject each named fault and require its target checks to go red
    oracle.py emit [--out DIR]    write oracle-generated vectors for the Rust tests (default
                                  crates/v2xw-sec/tests/vectors/oracle) and their manifest
    oracle.py verify-emitted      recompute the emitted files and compare byte for byte
    oracle.py coer TYPE HEX       decode HEX as TYPE and re-encode (canonical check)

Run with the oracle venv:  <venv>/bin/python tools/oracle/oracle.py selftest
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from scms_oracle import corpus, faults  # noqa: E402


def summarize(res, verbose=False):
    total = sum(len(v) for v in res.values())
    bad = [(c, vid, d) for c, rows in res.items() for vid, ok, d in rows if not ok]
    if verbose or bad:
        for c, vid, d in bad[:20]:
            print(f'  RED {c}: {vid} {d}')
    return total, bad


def cmd_selftest(a):
    res = corpus.run_all()
    total, bad = summarize(res, a.verbose)
    if a.verbose:
        for c, rows in res.items():
            print(f'  {c}: {len(rows)} ok' if all(r[1] for r in rows) else f'  {c}: {sum(not r[1] for r in rows)} RED')
    print(f'selftest: {"PASS" if not bad else "FAIL"} ({total - len(bad)}/{total} vectors in {len(res)} checks)')
    return 0 if not bad else 1


def cmd_mutate(a):
    clean = corpus.run_all()
    _, bad = summarize(clean)
    if bad:
        print('mutate: FAIL (corpus is not green without faults)')
        return 1
    failures = []
    for name, (desc, targets) in faults.CATALOGUE.items():
        if a.only and name not in a.only:
            continue
        faults.ACTIVE.clear()
        faults.ACTIVE.add(name)
        try:
            res = corpus.run_all(set(targets))
        finally:
            faults.ACTIVE.clear()
        red = {t: sum(not ok for _, ok, _ in res.get(t, [])) for t in targets}
        n = {t: len(res.get(t, [])) for t in targets}
        silent = [t for t in targets if red[t] == 0]
        status = 'red' if not silent else 'SILENT in ' + ','.join(silent)
        print(f'  {name}: {status} ({"; ".join(f"{t} {red[t]}/{n[t]}" for t in targets)})')
        if silent:
            failures.append(name)
    # restored: the corpus is green again
    after = corpus.run_all()
    _, bad_after = summarize(after)
    ok = not failures and not bad_after
    print(f'mutate: {"PASS" if ok else "FAIL"} ({len(faults.CATALOGUE) if not a.only else len(a.only)} faults, '
          f'{len(failures)} silent, corpus green after restore: {not bad_after})')
    return 0 if ok else 1


def cmd_emit(a):
    from scms_oracle import emit
    return emit.main(a.out, check_only=False)


def cmd_verify_emitted(a):
    from scms_oracle import emit
    return emit.main(a.out, check_only=True)


def cmd_coer(a):
    from scms_oracle import coer
    data = bytes.fromhex(a.hex)
    val = coer.decode(a.type, data)
    re = coer.encode(a.type, val)
    print(f'coer: {"CANONICAL" if re == data else "NOT CANONICAL"} ({len(data)} bytes)')
    if a.verbose:
        print(val)
    return 0 if re == data else 1


def cmd_real(a):
    from scms_oracle import real
    total = bad = 0
    for path in a.files:
        try:
            rows = real.check_file(path)
        except Exception as e:
            rows = [('<exception>', False, f'{type(e).__name__}: {e}')]
        for name, ok, d in rows:
            total += 1
            if not ok:
                bad += 1
            if a.verbose or not ok:
                print(f'  {"ok " if ok else "RED"} {os.path.basename(path)}: {name} {d}')
    print(f'real: {"PASS" if not bad else "FAIL"} ({total - bad}/{total} checks over {len(a.files)} files)')
    return 0 if not bad else 1


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='cmd', required=True)
    s = sub.add_parser('selftest')
    s.add_argument('-v', '--verbose', action='store_true')
    s.set_defaults(fn=cmd_selftest)
    m = sub.add_parser('mutate')
    m.add_argument('--only', nargs='*')
    m.set_defaults(fn=cmd_mutate)
    default_out = os.path.join(corpus.CORPUS_DIR, 'oracle')
    e = sub.add_parser('emit')
    e.add_argument('--out', default=default_out)
    e.set_defaults(fn=cmd_emit)
    ve = sub.add_parser('verify-emitted')
    ve.add_argument('--out', default=default_out)
    ve.set_defaults(fn=cmd_verify_emitted)
    c = sub.add_parser('coer')
    c.add_argument('type')
    c.add_argument('hex')
    c.add_argument('-v', '--verbose', action='store_true')
    c.set_defaults(fn=cmd_coer)
    r = sub.add_parser('real')
    r.add_argument('files', nargs='+')
    r.add_argument('-v', '--verbose', action='store_true')
    r.set_defaults(fn=cmd_real)
    a = ap.parse_args()
    sys.exit(a.fn(a))


if __name__ == '__main__':
    main()
