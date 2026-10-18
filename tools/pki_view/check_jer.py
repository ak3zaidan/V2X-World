#!/usr/bin/env python3
"""WP-E1: the PKI view's PDU decoder against an independent one.

`inspect.pdu` shows each SPDU as an ASN.1 value tree: the engine decodes the COER bytes
with its rasn bindings and writes every layer in JER (ITU-T X.697).  This script takes
the corpus `crates/v2xw-proto/tests/pki_view.rs` writes (`V2XW_PKI_OUT=<dir>`), decodes
every layer's bytes again with asn1tools' OER codec over the unchanged 1609.2-2022 and
1609.2.1-2022 modules (the WP-A0 oracle's `scms_oracle.coer`, which does not share a
line of code with the Rust), encodes that value with asn1tools' own JER codec, and
compares the two JSON trees value for value.

Normalisation is limited to what X.697 leaves open and no more: hex digits compared
case-insensitively, and an open type (an extension's content, which the oracle compiles
as OCTET STRING, see coer.py) compared as the hex of its encoding.  Anything else that
differs is a disagreement and fails the run.

Usage: check_jer.py <corpus-dir>      prints one verdict line, exit 0 on agreement.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), 'oracle'))

import asn1tools  # noqa: E402
from scms_oracle import coer  # noqa: E402

TYPES = {'Ieee1609Dot2Data', 'ScmsPdu', 'SignedCertificateRequest', 'CrlContents',
         'EncryptedIndividualPLV'}


def jer_spec():
    parsed = asn1tools.parse_files(coer.module_files(), encoding='cp1252')
    coer._fix_open_types(parsed)
    return asn1tools.compile_dict(parsed, 'jer')


def to_jer(v):
    """asn1tools' decoded value in X.697 form: a CHOICE as {alternative: value}, an
    OCTET STRING as hex, a BIT STRING as the hex of its bits, NULL as null.

    Used when asn1tools' own JER encoder refuses a value it decoded (it treats an absent
    extension addition of a 1609.2-2022 ToBeSignedCertificate as missing); where its
    encoder does work, the two are checked equal first (`main`)."""
    if isinstance(v, tuple) and len(v) == 2 and isinstance(v[0], str):
        return {v[0]: to_jer(v[1])}
    if isinstance(v, tuple) and len(v) == 2 and isinstance(v[0], (bytes, bytearray)):
        return v[0].hex()
    if isinstance(v, (bytes, bytearray)):
        return v.hex()
    if isinstance(v, dict):
        return {k: to_jer(x) for k, x in v.items()}
    if isinstance(v, list):
        return [to_jer(x) for x in v]
    return v


def norm(v):
    if isinstance(v, str):
        return v.lower()
    if isinstance(v, list):
        return [norm(x) for x in v]
    if isinstance(v, dict):
        # X.697 §24: a BIT STRING of variable size is {"value": hex, "length": n}; one of
        # fixed size is the hex alone. Compared as the hex of the bits either way.
        if set(v) == {'value', 'length'} and isinstance(v['value'], str):
            return v['value'].lower()
        return {k: norm(x) for k, x in v.items()}
    return v


def module_defaults():
    """Every component the modules declare with a DEFAULT, as `(name, default)`.

    COER omits a component equal to its DEFAULT (X.696 §8.3.3); asn1tools materialises
    the default on decode and rasn's JER leaves it out, which X.697 allows. Such a key is
    accepted as absent on the rasn side only when it is one of these and its value is the
    default. The set is pinned: a module change that adds a DEFAULT fails here until the
    mapping below is extended."""
    parsed = asn1tools.parse_files(coer.module_files(), encoding='cp1252')
    seen = set()

    def walk(n):
        if isinstance(n, dict):
            if 'default' in n and 'name' in n:
                seen.add((n['name'], repr(n['default'])))
            for v in n.values():
                walk(v)
        elif isinstance(n, list):
            for v in n:
                walk(v)

    walk(parsed)
    expected = {('chainLengthRange', '0'), ('eeType', "['app']"), ('minChainLength', '1')}
    if seen != expected:
        print(f'FAIL the modules declare DEFAULTs {sorted(seen)}, not {sorted(expected)}')
        sys.exit(1)
    # EndEntityType ::= BIT STRING {app (0), enrol (1)} (SIZE (8)): {app} is 0x80.
    return {'chainLengthRange': 0, 'minChainLength': 1, 'eeType': '80'}


DEFAULTS = {}


def diff(a, b, path='$', out=None):
    out = [] if out is None else out
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a and k in DEFAULTS and b[k] == DEFAULTS[k]:
                continue
            if k not in a or k not in b:
                out.append(f'{path}.{k}: only in {"rasn" if k in a else "asn1tools"}')
            else:
                diff(a[k], b[k], f'{path}.{k}', out)
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for i, (x, y) in enumerate(zip(a, b)):
            diff(x, y, f'{path}[{i}]', out)
    elif a != b:
        out.append(f'{path}: rasn {json.dumps(a)[:80]} != asn1tools {json.dumps(b)[:80]}')
    return out


def main():
    rows = json.load(open(os.path.join(sys.argv[1], 'pdus.json')))
    jer = jer_spec()
    DEFAULTS.update(module_defaults())
    layers = agree = native_used = 0
    bad = []
    kinds = set()
    for row in rows:
        for layer in row['tree']['layers']:
            t = layer['type']
            if t not in TYPES:
                continue
            layers += 1
            value = coer.decode(t, bytes.fromhex(layer['hex']))
            theirs = to_jer(value)
            try:
                native = json.loads(jer.encode(t, value).decode())
            except asn1tools.EncodeError:
                native = None
            if native is not None:
                native_used += 1
                d = diff(norm(native), norm(theirs))
                if d:
                    bad.append((row['id'], t + ' (converter vs asn1tools JER)', d[:3]))
                    continue
            d = diff(norm(layer['jer']), norm(theirs))
            if d:
                bad.append((row['id'], t, d[:3]))
            else:
                agree += 1
                kinds.add((row['type'], t))
    if bad:
        for rid, t, d in bad[:5]:
            print(f'DISAGREE {rid} {t}: {d}')
        print(f'FAIL {agree}/{layers} layers agree')
        sys.exit(1)
    print(f'PASS {agree}/{layers} layers agree across {len(kinds)} (PDU, layer) kinds; '
          f'{native_used} also matched asn1tools\' own JER encoder')


if __name__ == '__main__':
    main()
