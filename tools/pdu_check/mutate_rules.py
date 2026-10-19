"""Fault injection for the CF-RULE checks (docs/design/scms-real/validation/crypto-formats.md §4) added to check_pdus.py / check_appcerts.py.

Each fault changes either the expectation or the decoded data *after* the signature checks
(so only the new rule can see it) and requires the checker's verdict to go red; the clean
run must be green. Prints one line per fault and a verdict.
"""
import contextlib
import copy
import io
import os
import sys

W = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
sys.path.insert(0, os.path.join(W, 'tools', 'pdu_check'))
sys.path.insert(0, os.path.join(W, 'tools', 'oracle'))
import check_pdus as cp  # noqa: E402
import check_appcerts as ca  # noqa: E402

OUT = sys.argv[1]


def run(mod, d):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = mod.main(d)
    return rc, buf.getvalue().strip().splitlines()[-1]


def patched_pdu(alt, fn):
    """Wrap scms_pdu so that a decoded PDU of alternative `alt` is changed by fn."""
    orig = cp.scms_pdu

    def wrapper(b):
        iface, (c, v) = orig(b)
        if c == alt:
            v = copy.deepcopy(v)
            fn(v)
        return iface, (c, v)
    return orig, wrapper


FAULTS = [
    ('hashId expected sha384', lambda: cp.HASH_ID.__setitem__('P-256', 'sha384'),
     lambda: cp.HASH_ID.__setitem__('P-256', 'sha256'), [(cp, 'pdu'), (ca, 'app')]),
    ('version 3 required', None, None, [(cp, 'pdu'), (ca, 'app'), (cp, 'enr')]),
    ('certIssuePermissions required', None, None, [(cp, 'pdu'), (ca, 'app')]),
    ('ecaCertChain reversed', 'ecaEeCertResponse', lambda v: v['ecaCertChain'].reverse(), [(cp, 'pdu')]),
    ('acaEeCertResponse privateKeyInfo dropped', 'acaEeCertResponse', lambda v: v.pop('privateKeyInfo'),
     [(cp, 'pdu'), (ca, 'app')]),
]


def main():
    ok = True
    for name, mods in [('clean', [(cp, 'pdu'), (cp, 'enr'), (ca, 'app')])]:
        for m, d in mods:
            rc, line = run(m, os.path.join(OUT, d))
            ok &= rc == 0
            print(f'clean {d}: {line}')
    for name, a, b, targets in FAULTS:
        saved = {}
        if name == 'version 3 required':
            saved['v2'] = cp.v2
            cp.v2 = lambda pdu, what: cp.need(pdu.get('version') == 3, what)
        elif name == 'certIssuePermissions required':
            saved['t'] = cp.tbs_request_permissions
            cp.tbs_request_permissions = lambda t, what: cp.need('certIssuePermissions' in t, what)
        elif isinstance(a, str):
            orig, wrapper = patched_pdu(a, b)
            saved['pdu'] = orig
            cp.scms_pdu = wrapper
        else:
            a()
        for m, d in targets:
            rc, line = run(m, os.path.join(OUT, d))
            red = rc != 0
            ok &= red
            print(f'{"RED  " if red else "SILENT"} {name} on {d}: {line}')
        if 'v2' in saved:
            cp.v2 = saved['v2']
        if 't' in saved:
            cp.tbs_request_permissions = saved['t']
        if 'pdu' in saved:
            cp.scms_pdu = saved['pdu']
        if callable(b) and not isinstance(a, str):
            b()
    print(f'checker-mutation: {"PASS" if ok else "FAIL"}')


main()
