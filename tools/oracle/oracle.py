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


EMIT_FAULTS = ['ecqv-1609-sec4-truncate', 'ecqv-1609-whole-cert', 'bf-swap-prefix', 'time32-no-leap', 'kdf2-counter-0',
               'hashedid-high-bytes', 'lv-no-pad', 'ccm-counter-1', 'ecies-dhaes']


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
    # emitted files: the faults that reach the generator must change its output
    from scms_oracle import emit
    out = os.path.join(corpus.CORPUS_DIR, 'oracle')
    if os.path.isdir(out) and not a.only:
        import contextlib
        import io
        for name in EMIT_FAULTS:
            faults.ACTIVE.clear()
            faults.ACTIVE.add(name)
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    rc = emit.main(out, check_only=True)
            except AssertionError:
                rc = 1
            finally:
                faults.ACTIVE.clear()
            print(f'  emit/{name}: {"red" if rc else "SILENT"}')
            if not rc:
                failures.append('emit/' + name)
        with contextlib.redirect_stdout(io.StringIO()):
            if emit.main(out, check_only=True):
                failures.append('emit not green after restore')
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


def cmd_records(a):
    import json
    from scms_oracle import records
    rc = 0
    for d in a.runs:
        R = records.load(d)
        X = records.run_rules(R)
        bad = 0
        for rule, n_ok, fails in X.rows():
            bad += len(fails)
            print(f'  {"ok " if not fails else "RED"} {rule}: {n_ok} ok, {len(fails)} red')
            for f in fails[:a.show]:
                print(f'        {f}')
        for n in sorted(X.na):
            print(f'  n/a {n}')
        if X.info:
            print(f'  info {json.dumps(X.info)}')
        total = sum(n for _, n, _ in X.rows()) + bad
        print(f'records {R.name}: {"PASS" if not bad else "FAIL"} ({total - bad}/{total} checks, {len(R.pdus)} PDUs)')
        if a.json:
            json.dump({'run': R.name, 'rows': [{'rule': r, 'ok': n, 'red': f} for r, n, f in X.rows()],
                       'na': sorted(X.na), 'info': X.info}, open(a.json if len(a.runs) == 1 else f'{a.json}.{R.name}', 'w'),
                      indent=1, default=str)
        if a.expect:
            import yaml
            red = {r for r, _, f in X.rows() if f}
            have = {r for r, _, _ in X.rows()}
            rows = [r for r in yaml.safe_load(open(a.expect)) if r.get('run') == R.name and 'rule' in r]
            miss = []
            for r in rows:
                got = 'red' if r['rule'] in red else ('pass' if r['rule'] in have else 'absent')
                if got != r['expect']:
                    miss.append(f'{r["rule"]}: expected {r["expect"]}, got {got}')
            for m in miss:
                print(f'  EXPECT {m}')
            print(f'expect {R.name}: {"PASS" if not miss else "FAIL"} ({len(rows) - len(miss)}/{len(rows)} rows)')
            rc = rc or (1 if miss else 0)
        elif bad:
            rc = 1
        if a.mutate:
            res = records.mutate(R, a.only)
            for name, rule, st in res:
                print(f'  fault {name} -> {rule}: {st}')
            silent = [n for n, _, st in res if st == 'SILENT']
            print(f'records-mutate {R.name}: {"PASS" if not silent else "FAIL"} '
                  f'({sum(st == "red" for _, _, st in res)} red, {len(silent)} silent, '
                  f'{sum(st == "n/a" for _, _, st in res)} n/a)')
            if silent:
                rc = 1
    return rc


def cmd_ctl_files(a):
    import json
    from scms_oracle import ctlfiles
    gov = json.load(open(a.gov))
    X = ctlfiles.check(gov)
    bad = 0
    for rule, n_ok, fails in X.rows():
        bad += len(fails)
        print(f'  {"ok " if not fails else "RED"} {rule}: {n_ok} ok, {len(fails)} red')
        for f in fails[:3]:
            print(f'        {f}')
    print(f'ctl-files: {"PASS" if not bad else "FAIL"} ({sum(len(g["ctls"]) for g in gov)} CTL copies, '
          f'{len(gov)} providers)')
    rc = 1 if bad else 0
    if a.mutate:
        res = ctlfiles.mutate(gov)
        for name, rule, st in res:
            print(f'  fault {name} -> {rule}: {st}')
        silent = [n for n, _, st in res if st == 'SILENT']
        print(f'ctl-files-mutate: {"PASS" if not silent else "FAIL"} ({sum(st == "red" for _, _, st in res)} red, '
              f'{len(silent)} silent)')
        rc = rc or (1 if silent else 0)
    return rc


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
    rr = sub.add_parser('records')
    rr.add_argument('runs', nargs='+', help='run directories (recording.mcap + scenario.resolved.yaml)')
    rr.add_argument('--mutate', action='store_true', help='also inject every record fault and require red')
    rr.add_argument('--only', nargs='*')
    rr.add_argument('--show', type=int, default=3, help='failures printed per rule')
    rr.add_argument('--json')
    rr.add_argument('--expect', help='expectation YAML: rows {run, rule, expect: pass|red}')
    rr.set_defaults(fn=cmd_records)
    cf = sub.add_parser('ctl-files')
    cf.add_argument('gov', help="an F1 capture's gov.json")
    cf.add_argument('--mutate', action='store_true')
    cf.set_defaults(fn=cmd_ctl_files)
    a = ap.parse_args()
    sys.exit(a.fn(a))


if __name__ == '__main__':
    main()
