"""Random certificate chains built and judged by the independent oracle, for the Rust chain validator.

WP-A3 acceptance (docs/design/scms-real/00 section 4.2): "dual implementation against the oracle on 500
random chains". This script is glue only: it builds every certificate, signature and ECQV issuance with
the oracle package (tools/oracle/scms_oracle: coer, certs, ecqv, prims, chain) and records the oracle's
own verdict (``chain.validate``). It does not change or extend the oracle. The Rust test
``crates/v2xw-sec/tests/chain_xcheck.rs`` reads the file, runs ``v2xw_sec::chain::validate`` and the
SPDU check on every chain, re-issues every certificate from the recorded inputs, and requires the
same verdicts and byte-identical certificates.

Commands (project venv, ``python -I tools/chain-xcheck/chain_xcheck.py ...``):

  gen OUT [--n 500] [--seed 20261016]   write the corpus (JSON) and print a one-line summary
  verify OUT                            regenerate in memory and require byte equality

Where the harness, not the oracle, decides something (stated so a green run is not over-read):
  * ``spdu.valid``: the oracle's ``chain.validate`` checks implicit certificates only through their
    issuer links, so the EE key is checked by signing a digest with the EE private key the issuance
    produced and verifying it with the key reconstructed by ``certs.verification_key`` (one level) and
    ``ecqv.e_1609`` + ``ecqv.public_key`` (deeper implicit levels, because ``certs.verification_key``
    cannot resolve an implicit issuer).
  * Chains are generated only in the space where the oracle's rules are complete: explicit
    certificates only under explicit issuers, one permissions group per CA, no bitmap SSPs, opaque
    SSP ranges only on the EE's issuer, identifiedRegion countryOnly regions only, and a region only
    where the issuer has one or no ancestor has one. Rules outside that space (chain length > 8,
    bitmap SSPs, sub-region containment, certRequestPermissions, duplicate PSIDs) are covered by the
    Rust negative tests instead.
"""

import argparse
import hashlib
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), 'oracle'))

from scms_oracle import certs, chain, coer, ec, ecqv, prims  # noqa: E402

C = ec.P256
START = 679737605  # 2025-07-16 in Time32 (tools/oracle time1609.KNOWN)
PSIDS = [32, 35, 38, 0x80, 0x20409F, 0x8007]
COUNTRIES = [124, 484, 840]
MUTATIONS = ['none'] * 8 + ['sig-tamper', 'tbs-tamper', 'implicit-tamper', 'issuer-id', 'validity',
                            'region', 'psid', 'eetype', 'ssp', 'chain-length', 'min-zero', 'untrusted-root',
                            'root-sig']


def i2h(x, n=32):
    return '%0*x' % (2 * n, x)


def scalar(rnd):
    return rnd.randrange(1, C.n)


def compose_key(cert_chain, n, keys):
    """Key of chain[n] given keys of chain[n+1..]; deeper implicit levels use the oracle primitives."""
    c = cert_chain[n]
    if c['toBeSigned']['verifyKeyIndicator'][0] == 'verificationKey':
        return certs.verification_key(c)[1]
    iss = cert_chain[n + 1]
    e = ecqv.e_1609(certs.tbs_bytes(c), certs.encode_canonical(iss))
    PU = certs.point_from(C, c['toBeSigned']['verifyKeyIndicator'][1])
    return ecqv.public_key(e, PU, keys[n + 1])


def build_chain(rnd, idx):
    mutation = MUTATIONS[idx % len(MUTATIONS)] if idx >= 40 else 'none'
    below = rnd.randrange(1, 5)  # certificates below the root: 1..4
    total = below + 1
    # Heights: root = below, EE = 0. Chain-length groups that satisfy the inequalities.
    mins, maxs = {0: 0}, {0: 0}
    for h in range(1, below + 1):
        mins[h] = rnd.randrange(1, mins[h - 1] + 2)
        maxs[h] = maxs[h - 1] + 1 + rnd.randrange(0, 2)
    root_unbounded = rnd.random() < 0.5
    first_implicit = rnd.randrange(1, below + 2)  # heights below this are implicit (>= below+1: none)
    # Coverage: root covers all or an explicit list; each CA narrows or keeps.
    region_from = rnd.choice([None, below, rnd.randrange(0, below + 1)])
    certs_by_h, priv, issuance = {}, {}, {}
    cover = None  # list of psids, or 'all'
    ee_type = {below: rnd.choice([(b'\x80', 8), (b'\xc0', 8)])}
    regions = {}
    t0, t1 = START, START + 31556952 * rnd.randrange(10, 40)
    windows = {below: (t0, t1)}
    for h in range(below, -1, -1):
        d = scalar(rnd)
        is_root = h == below
        if is_root:
            s0, s1 = windows[h]
        else:
            p0, p1 = windows[h + 1]
            span = (p1 - p0) // 3600
            a = rnd.randrange(0, max(1, span // 4))
            hours = rnd.randrange(1, max(2, span - a))
            if hours > 65535:  # Uint16: whole years instead
                dur = ('years', hours * 3600 // 31556952)
                s0, s1 = p0 + 3600 * a, p0 + 3600 * a + dur[1] * 31556952
            else:
                dur = ('hours', hours)
                s0, s1 = p0 + 3600 * a, p0 + 3600 * (a + hours)
            windows[h] = (s0, s1)
        if is_root:
            dur = ('years', (s1 - s0) // 31556952)
        t = {'id': ('name', f'ca{h}.chain{idx}.example') if h else
             rnd.choice([('none', None), ('linkageData', {'iCert': rnd.randrange(0, 60000), 'linkage-value': rnd.randbytes(9)}),
                         ('name', f'ee.chain{idx}.example')]),
             'cracaId': rnd.randbytes(3), 'crlSeries': rnd.choice([1, 2, 3, 4, 256]),
             'validityPeriod': {'start': s0, 'duration': dur}}
        # region
        if region_from is not None and h <= region_from:
            if h == region_from:
                regions[h] = sorted(rnd.sample(COUNTRIES, rnd.randrange(1, 4)))
            else:
                regions[h] = sorted(rnd.sample(regions[h + 1], rnd.randrange(1, len(regions[h + 1]) + 1)))
            if mutation == 'region' and h == 0 and 1 in regions:
                outside = [c for c in COUNTRIES + [36] if c not in regions[1]]
                regions[h] = sorted(set(regions[h]) | {outside[0]})
            t['region'] = ('identifiedRegion', [('countryOnly', c) for c in regions[h]])
        if h > 0:
            # CA: one certIssuePermissions group.
            if cover is None:
                sub = 'all' if rnd.random() < 0.5 else sorted(rnd.sample(PSIDS, rnd.randrange(2, 5)))
            elif cover == 'all':
                sub = 'all' if rnd.random() < 0.6 else sorted(rnd.sample(PSIDS, rnd.randrange(2, 5)))
            else:
                sub = sorted(rnd.sample(cover, rnd.randrange(1, len(cover) + 1)))
            et = ee_type[h] if is_root else (ee_type[h + 1] if rnd.random() < 0.7 else (b'\x80', 8))
            ee_type[h] = et
            mn, rg = mins[h], maxs[h] - mins[h]
            if is_root and root_unbounded:
                rg = -1
            if h == 1 and mutation == 'chain-length':
                mn = 2
            if h == 1 and mutation == 'min-zero':
                mn = 0
            if h == 1 and mutation == 'eetype':
                et = (b'\x40', 8)
            if sub == 'all':
                sp = ('all', None)
            else:
                ranges = []
                for p in sub:
                    pr = {'psid': p}
                    if h == 1 and rnd.random() < 0.5:
                        pr['sspRange'] = ('opaque', [b'\x01', b'\x02\x03', b''][:rnd.randrange(1, 4)])
                    elif rnd.random() < 0.3:
                        pr['sspRange'] = ('all', None)
                    ranges.append(pr)
                sp = ('explicit', ranges)
            t['certIssuePermissions'] = [{'subjectPermissions': sp, 'minChainLength': mn, 'chainLengthRange': rg, 'eeType': et}]
            cover = 'all' if sub == 'all' else sub
            ca_ranges = None if sub == 'all' else {r['psid']: r.get('sspRange') for r in sp[1]}
            if rnd.random() < 0.3:
                t['appPermissions'] = [{'psid': 35}] if cover == 'all' or 35 in cover else [{'psid': cover[0]}]
        else:
            # EE: appPermissions within the issuer's coverage, SSP per the issuer range.
            pool = PSIDS if cover == 'all' else cover
            app = []
            for p in sorted(rnd.sample(pool, rnd.randrange(1, min(4, len(pool)) + 1))):
                ps = {'psid': p}
                rng = ca_ranges.get(p) if ca_ranges else None
                if rng is not None and rng[0] == 'opaque':
                    choice = rnd.choice(rng[1])
                    if choice:
                        ps['ssp'] = ('opaque', choice)
                elif rnd.random() < 0.3:
                    ps['ssp'] = ('opaque', rnd.randbytes(rnd.randrange(0, 4)))
                app.append(ps)
            if mutation == 'psid':
                missing = [p for p in [0x7F, 0x2040FF] if cover != 'all' and p not in cover]
                if missing:
                    app.append({'psid': missing[0]})
            if mutation == 'ssp' and ca_ranges:
                opq = [p for p, r in ca_ranges.items() if r is not None and r[0] == 'opaque']
                if opq:
                    app = [a for a in app if a['psid'] != opq[0]] + [{'psid': opq[0], 'ssp': ('opaque', b'\xee\xee')}]
                    app.sort(key=lambda a: a['psid'])
            t['appPermissions'] = app
        if mutation == 'validity' and h == 0:
            v = t['validityPeriod']
            over = (windows[1][1] - v['start']) // 3600 + 5
            t['validityPeriod'] = {'start': v['start'], 'duration': ('hours', over) if over <= 65535 else ('years', over * 3600 // 31556952 + 1)}
        implicit = (not is_root) and h < first_implicit
        priv[h] = d
        if is_root:
            t['verifyKeyIndicator'] = ('verificationKey', ('ecdsaNistP256', certs.point_value(C, ec.base_mul(C, d))))
            k = scalar(rnd)
            digest = prims.digest_1609('SHA-256', coer.encode('ToBeSignedCertificate', t), b'')
            r, s, _ = prims.ecdsa_sign(C, d, digest, k)
            cert = {'version': 3, 'type': 'explicit', 'issuer': ('self', 'sha256'), 'toBeSigned': t,
                    'signature': ('ecdsaNistP256Signature', {'rSig': ('x-only', ec.i2os(r, 32)), 'sSig': ec.i2os(s, 32)})}
            issuance[h] = {'kind': 'self', 'd_issuer': i2h(d), 'k': i2h(k)}
        else:
            iss = certs_by_h[h + 1]
            d_iss = priv[h + 1]
            if not implicit:
                t['verifyKeyIndicator'] = ('verificationKey', ('ecdsaNistP256', certs.point_value(C, ec.base_mul(C, d))))
                k = scalar(rnd)
                digest = prims.digest_1609('SHA-256', coer.encode('ToBeSignedCertificate', t), certs.encode_canonical(iss))
                r, s, _ = prims.ecdsa_sign(C, d_iss, digest, k)
                cert = {'version': 3, 'type': 'explicit', 'issuer': ('sha256AndDigest', certs.hashed_id8(iss)), 'toBeSigned': t,
                        'signature': ('ecdsaNistP256Signature', {'rSig': ('x-only', ec.i2os(r, 32)), 'sSig': ec.i2os(s, 32)})}
                issuance[h] = {'kind': 'explicit', 'd_issuer': i2h(d_iss), 'k': i2h(k)}
            else:
                ku = d  # requester key; the certificate key becomes e*ku + r
                box = {}

                def e_of(PU, t=t, iss=iss, box=box):
                    tt = dict(t, verifyKeyIndicator=('reconstructionValue', certs.point_value(C, PU)))
                    box['tbs'] = tt
                    return ecqv.e_1609(coer.encode('ToBeSignedCertificate', tt), certs.encode_canonical(iss))

                k = scalar(rnd)
                PU, r, e = ecqv.issue(ec.base_mul(C, ku), d_iss, k, e_of)
                t = box['tbs']
                cert = {'version': 3, 'type': 'implicit', 'issuer': ('sha256AndDigest', certs.hashed_id8(iss)), 'toBeSigned': t}
                priv[h] = ecqv.private_key(e, ku, r)
                issuance[h] = {'kind': 'implicit', 'd_issuer': i2h(d_iss), 'k_u': i2h(ku), 'k': i2h(k), 'r': i2h(r)}
        certs_by_h[h] = cert
    chain_list = [certs_by_h[h] for h in range(0, below + 1)]
    root = chain_list[-1]
    anchors = [prims.sha256(certs.encode_canonical(root))]
    # Post-issuance tampering.
    applied = mutation
    if mutation == 'sig-tamper' or mutation == 'tbs-tamper':
        exp = [n for n in range(below) if chain_list[n]['type'] == 'explicit']
        if exp:
            n = rnd.choice(exp)
            if mutation == 'sig-tamper':
                a, sg = chain_list[n]['signature']
                ss = bytearray(sg['sSig'])
                ss[rnd.randrange(32)] ^= 1 << rnd.randrange(8)
                chain_list[n]['signature'] = (a, dict(sg, sSig=bytes(ss)))
            else:
                chain_list[n]['toBeSigned']['crlSeries'] ^= 1
        else:
            applied = 'none'
    elif mutation == 'implicit-tamper':
        imp = [n for n in range(below) if chain_list[n]['type'] == 'implicit']
        if imp:
            chain_list[imp[0]]['toBeSigned']['crlSeries'] ^= 1
        else:
            applied = 'none'
    elif mutation == 'issuer-id':
        n = rnd.randrange(below)
        kind, hid = chain_list[n]['issuer']
        hb = bytearray(hid)
        hb[rnd.randrange(8)] ^= 0x80
        chain_list[n]['issuer'] = (kind, bytes(hb))
    elif mutation == 'untrusted-root':
        anchors = [hashlib.sha256(b'another root').digest()]
    elif mutation == 'root-sig':
        a, sg = root['signature']
        ss = bytearray(sg['sSig'])
        ss[0] ^= 0x10
        root['signature'] = (a, dict(sg, sSig=bytes(ss)))
    elif mutation == 'region' and not (1 in regions and 0 in regions):
        applied = 'none'
    elif mutation == 'psid' and cover == 'all':
        applied = 'none'
    elif mutation == 'ssp' and not (ca_ranges and any(r is not None and r[0] == 'opaque' for r in ca_ranges.values())):
        applied = 'none'

    # Oracle verdict on the chain.
    errors = chain.validate(chain_list, anchors)
    # SPDU signed with the EE private key, verified with the (composed) oracle key.
    keys = {}
    for n in range(below, -1, -1):
        keys[n] = compose_key(chain_list, n, keys) if n < below else certs.verification_key(chain_list[n])[1]
    digest = rnd.randbytes(32)
    ks = scalar(rnd)
    r, s, _ = prims.ecdsa_sign(C, priv[0], digest, ks)
    spdu_ok = keys[0] is not None and prims.ecdsa_verify(C, keys[0], digest, r, s)
    return {
        'id': idx, 'mutation': applied,
        'certs': [certs.encode(c).hex() for c in chain_list],
        'anchors': [a.hex() for a in anchors],
        'oracle_valid': not errors, 'oracle_errors': errors,
        'spdu': {'digest': digest.hex(), 'r': i2h(r), 's': i2h(s), 'valid': bool(spdu_ok)},
        'issuance': [issuance[h] for h in range(0, below + 1)],
    }


def generate(n, seed):
    rnd = random.Random(seed)
    out = [build_chain(rnd, i) for i in range(n)]
    return {'generator': 'tools/chain-xcheck/chain_xcheck.py (oracle: tools/oracle/scms_oracle)',
            'edition': '1609.2-2022', 'seed': seed, 'count': n,
            'rule': 'certs[0] is the EE, each next its issuer, the last a self-signed root; anchors are '
                    'HashedId32 (SHA-256 of the canonical root COER); oracle_valid is chain.validate == []',
            'chains': out}


def render(doc):
    return (json.dumps(doc, indent=0, separators=(',', ':')) + '\n').encode()


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='cmd', required=True)
    g = sub.add_parser('gen')
    g.add_argument('out')
    g.add_argument('--n', type=int, default=500)
    g.add_argument('--seed', type=int, default=20261016)
    v = sub.add_parser('verify')
    v.add_argument('path')
    a = ap.parse_args()
    if a.cmd == 'gen':
        doc = generate(a.n, a.seed)
        b = render(doc)
        with open(a.out, 'wb') as f:
            f.write(b)
        ch = doc['chains']
        by = {}
        for c in ch:
            by.setdefault(c['mutation'], [0, 0])[0 if c['oracle_valid'] else 1] += 1
        print(f"gen: {len(ch)} chains, {sum(c['oracle_valid'] for c in ch)} valid, "
              f"{sum(c['spdu']['valid'] for c in ch)} SPDU-valid; sha256 {hashlib.sha256(b).hexdigest()[:16]}; "
              + ', '.join(f'{k}={v[0]}/{v[1]}' for k, v in sorted(by.items())))
        return 0
    old = open(a.path, 'rb').read()
    doc = json.loads(old)
    b = render(generate(doc['count'], doc['seed']))
    ok = b == old
    print(f"verify: {'PASS' if ok else 'FAIL'} ({doc['count']} chains regenerated {'identically' if ok else 'with differences'})")
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main())
