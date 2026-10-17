"""Reproduce every vector of the transcribed corpus with the oracle's own code.

Each check returns a list of (vector id, ok, detail).  ``run_all`` returns {check name: results}.
"""

import datetime as dt
import hashlib
import json
import os

from . import butterfly, ec, ecqv, faults, linkage, prims, time1609

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
CORPUS_DIR = os.path.join(ROOT, 'crates', 'v2xw-sec', 'tests', 'vectors')

H = bytes.fromhex


def load(name):
    with open(os.path.join(CORPUS_DIR, name)) as f:
        return json.load(f)


def _cmp(fields):
    bad = [k for k, (got, want) in fields.items() if got != want]
    return (not bad), ('mismatch: ' + ', '.join(bad)) if bad else ''


# --------------------------------------------------------------------------------------------

def check_ccm(name):
    out = []
    for v in load(name)['vectors']:
        k, n, p = H(v['in']['key']), H(v['in']['nonce']), H(v['in']['plaintext'])
        ct = prims.ccm_encrypt(k, n, p)
        dec = prims.ccm_decrypt(k, n, H(v['out']['ciphertext_tag']))
        ok, d = _cmp({'ciphertext_tag': (ct.hex(), v['out']['ciphertext_tag']), 'decrypt': (dec, p)})
        out.append((v['id'], ok, d))
    return out


def _ecies_one(v, p1):
    C = ec.P256
    vv, r = int(v['in']['v'], 16), int(v['in']['r'], 16)
    R = (int(v['in']['rx'], 16), int(v['in']['ry'], 16))
    k = H(v['in']['k'])
    V, c, t = prims.ecies_encrypt(R, k, p1, vv)
    dec = prims.ecies_decrypt(r, V, H(v['out']['c']), H(v['out']['t']), p1)
    return _cmp({'R=r*G': (ec.base_mul(C, r), R), 'vx': ('%064x' % V[0], v['out']['vx']), 'vy': ('%064x' % V[1], v['out']['vy']),
                 'c': (c.hex(), v['out']['c']), 't': (t.hex(), v['out']['t']), 'decrypt': (dec, k)})


def check_ecies(name):
    out = []
    for v in load(name)['vectors']:
        p1 = H(v['p1_printed'] if faults.on('ecies-printed-p1') else v['in']['p1'])
        ok, d = _ecies_one(v, p1)
        out.append((v['id'], ok, d))
    return out


def check_ecies_erratum(name):
    """The printed P1 must NOT reproduce C and T (02 section 8.2): this check passes when it fails."""
    out = []
    for v in load(name)['vectors']:
        ok, _ = _ecies_one(v, H(v['p1_printed']))
        out.append((v['id'] + ' printed-P1', not ok, 'printed P1 reproduced C/T' if ok else ''))
    return out


def check_mac1(name):
    out = []
    for v in load(name)['vectors']:
        t = prims.mac1(H(v['in']['key']), H(v['in']['message']))
        ok, d = _cmp({'tag': (t.hex(), v['out']['tag'])})
        out.append((v['id'], ok, d))
    return out


def check_kdf(name):
    out = []
    for v in load(name)['vectors']:
        key = prims.kdf2(H(v['in']['ss']), v['in']['dl'], H(v['in']['kdp']))
        ok, d = _cmp({'key': (key.hex(), v['out']['key'])})
        out.append((v['id'], ok, d))
    return out


def check_x963():
    out = []
    for v in load('nist_x963_kdf.json')['vectors']:
        key = prims.kdf2(H(v['in']['z']), v['in']['key_data_len'], H(v['in']['shared_info']))
        ok, d = _cmp({'key_data': (key.hex(), v['out']['key_data'])})
        out.append((v['id'], ok, d))
    return out


def check_rfc4231():
    out = []
    for v in load('rfc4231_hmac_sha256.json')['vectors']:
        m = prims.hmac_sha256(H(v['in']['key']), H(v['in']['data']))
        want = v['out']['hmac_sha256']
        got = m[:16].hex() if v.get('truncated_bits') == 128 else m.hex()
        ok, d = _cmp({'hmac': (got, want)})
        out.append((v['id'], ok, d))
    return out


def check_hashedid():
    out = []
    for v in load('ieee1609dot2_hashedid_examples.json')['vectors']:
        n = int(v['out']['type'][len('HashedId'):])
        hid = prims.hashed_id(b'', n, v['hash'])
        dig = prims.HASHES[v['hash']](b'')
        ok, d = _cmp({'digest': (dig.hex(), v['out']['digest']), 'hashed_id': (hid.hex(), v['out']['hashed_id'])})
        out.append((v['id'], ok, d))
    return out


def check_sha():
    out = []
    for v in load('nist_sha.json')['vectors']:
        md = prims.HASHES[v['hash']](H(v['in']['msg']))
        ok, d = _cmp({'md': (md.hex(), v['out']['md'])})
        out.append((v['id'], ok, d))
    return out


def check_sigver():
    out = []
    for v in load('nist_ecdsa_sigver.json')['vectors']:
        C = ec.CURVES[v['curve']]
        Q = (int(v['in']['qx'], 16), int(v['in']['qy'], 16))
        digest = prims.HASHES[v['hash']](H(v['in']['msg']))
        got = prims.ecdsa_verify(C, Q, digest, int(v['in']['r'], 16), int(v['in']['s'], 16))
        ok, d = _cmp({'valid': (got, v['out']['valid'])})
        out.append((v['id'], ok, d))
    return out


def check_ecdsa_range():
    """r or s outside [1, n-1] must be rejected even when the algebra would otherwise pass."""
    out = []
    for C in (ec.P256, ec.P384):
        d, k = 0x1234567, 0x7654321
        Q = ec.base_mul(C, d)
        dig = prims.sha256(b'range') if C is ec.P256 else prims.sha384(b'range')
        r, s, _ = prims.ecdsa_sign(C, d, dig, k)
        out.append((f'{C.name} genuine', prims.ecdsa_verify(C, Q, dig, r, s), ''))
        # (r, s + n) is congruent to a valid signature: only the range check rejects it
        out.append((f'{C.name} s+n', not prims.ecdsa_verify(C, Q, dig, r, s + C.n), 'accepted s >= n'))
        out.append((f'{C.name} r+n', not prims.ecdsa_verify(C, Q, dig, r + C.n, s), 'accepted r >= n') if r + C.n < C.p else
                   (f'{C.name} r+n', True, 'r + n >= p, skipped'))
    return out


def check_nist_ccm():
    out = []
    for v in load('nist_ccm.json')['vectors']:
        i = v['in']
        if v['kind'] == 'decrypt-verify':
            p = prims.ccm_decrypt(H(i['key']), H(i['nonce']), H(i['ct']), H(i['adata']), i['tlen'])
            want = v['out']['payload']
            ok, d = _cmp({'result': (None if p is None else p.hex(), want if v['out']['valid'] else None)})
        else:
            ct = prims.ccm_encrypt(H(i['key']), H(i['nonce']), H(i['payload']), H(i['adata']), i['tlen'])
            ok, d = _cmp({'ct': (ct.hex(), v['out']['ct'])})
        out.append((v['id'], ok, d))
    return out


def check_bfkeyexp():
    out = []
    C = ec.P256
    for v in load('camp_bfkeyexp.json')['vectors']:
        i = v['in']
        kind = butterfly.SIGN if v['kind'] == 'signing' else butterfly.ENC
        key = H(i['key'])
        x = butterfly.x_input(kind, i['i'], i['j'])
        fi, trace = butterfly.f_int(key, x)
        f = butterfly.f(key, kind, i['i'], i['j'])
        a = int(i['seed_private'], 16)
        A = (int(i['seed_public_x'], 16), int(i['seed_public_y'], 16))
        Aexp = butterfly.expand_public(A, key, kind, i['i'], i['j'])
        o = v['out']
        ok, d = _cmp({'seed_public=a*G': (ec.base_mul(C, a), A), 'x': (x.hex(), o['x']),
                      'x1': (trace[0][0].hex(), o['x1']), 'x2': (trace[1][0].hex(), o['x2']), 'x3': (trace[2][0].hex(), o['x3']),
                      'aes1': (trace[0][1].hex(), o['aes1']), 'aes2': (trace[1][1].hex(), o['aes2']), 'aes3': (trace[2][1].hex(), o['aes3']),
                      'f_int': (fi.hex(), o['f_int']), 'f': ('%064x' % f, o['f']),
                      'expanded_private': ('%064x' % butterfly.expand_private(a, key, kind, i['i'], i['j']), o['expanded_private']),
                      'expanded_public': (Aexp, (int(o['expanded_public_x'], 16), int(o['expanded_public_y'], 16)))})
        out.append((v['id'], ok, d))
    return out


def check_lv():
    out = []
    for v in load('camp_lv.json')['vectors']:
        i, o = v['in'], v['out']
        la1, la2 = int(i['la_id1'], 16), int(i['la_id2'], 16)
        ls1 = linkage.seed_at(la1, H(i['ls1_0']), i['i'])
        ls2 = linkage.seed_at(la2, H(i['ls2_0']), i['i'])
        f = {'ls1_i': (ls1.hex(), o['ls1_i']), 'ls2_i': (ls2.hex(), o['ls2_i']),
             'aes_in1': (linkage.plv_input(la1, i['j']).hex(), o['aes_in1']), 'aes_in2': (linkage.plv_input(la2, i['j']).hex(), o['aes_in2']),
             'aes_out1': (prims.aes_block(ls1, linkage.plv_input(la1, i['j'])).hex(), o['aes_out1']),
             'aes_out2': (prims.aes_block(ls2, linkage.plv_input(la2, i['j'])).hex(), o['aes_out2']),
             'plv1': (linkage.plv(la1, ls1, i['j']).hex(), o['plv1']), 'plv2': (linkage.plv(la2, ls2, i['j']).hex(), o['plv2']),
             'lv': (linkage.lv(la1, H(i['ls1_0']), la2, H(i['ls2_0']), i['i'], i['j']).hex(), o['lv'])}
        if 'sha_in1' in o:
            f['sha_in1'] = (linkage.seed_input(la1, H(i['ls1_0'])).hex(), o['sha_in1'])
            f['sha_in2'] = (linkage.seed_input(la2, H(i['ls2_0'])).hex(), o['sha_in2'])
            f['sha_out1'] = (prims.sha256(linkage.seed_input(la1, H(i['ls1_0']))).hex(), o['sha_out1'])
            f['sha_out2'] = (prims.sha256(linkage.seed_input(la2, H(i['ls2_0']))).hex(), o['sha_out2'])
        ok, d = _cmp(f)
        out.append((v['id'], ok, d))
    return out


def check_glv():
    out = []
    for v in load('camp_glv.json')['vectors']:
        i, o = v['in'], v['out']
        la1, la2 = int(i['la_id1'], 16), int(i['la_id2'], 16)
        gs1 = linkage.seed_at(la1, H(i['gs1_0']), i['i'])
        gs2 = linkage.seed_at(la2, H(i['gs2_0']), i['i'])
        ei_in = i['k'].to_bytes(4, 'big') + i['j'].to_bytes(4, 'big') + b'\x00' * 8  # printed order k || j (see corpus note)
        f = {'gs1_i': (gs1.hex(), o['gs1_i']), 'gs2_i': (gs2.hex(), o['gs2_i']),
             'aes_in1': (linkage.plv_input(la1, i['j']).hex(), o['aes_in1']), 'aes_in2': (linkage.plv_input(la2, i['j']).hex(), o['aes_in2']),
             'gplv1': (linkage.plv(la1, gs1, i['j']).hex(), o['gplv1']), 'gplv2': (linkage.plv(la2, gs2, i['j']).hex(), o['gplv2']),
             'glv': (linkage.lv(la1, H(i['gs1_0']), la2, H(i['gs2_0']), i['i'], i['j']).hex(), o['glv']),
             'ei_in': (ei_in.hex(), o['ei_in']), 'ei': (linkage.encrypted_index(H(i['ik']), ei_in).hex(), o['ei'])}
        if 'sha_in1' in o:
            f['sha_out1'] = (prims.sha256(linkage.seed_input(la1, H(i['gs1_0']))).hex(), o['sha_out1'])
            f['sha_out2'] = (prims.sha256(linkage.seed_input(la2, H(i['gs2_0']))).hex(), o['sha_out2'])
        ok, d = _cmp(f)
        out.append((v['id'], ok, d))
    return out


def check_implicit_sec4():
    out = []
    C = ec.P256
    for v in load('camp_implicit.json')['vectors']:
        i, o = v['in'], v['out']
        tbs, ku, dca, k = H(i['tbs_cert']), int(i['ku'], 16), int(i['dca'], 16), int(i['k'], 16)
        RU = ec.base_mul(C, ku)
        holder = {}

        def e_of(PU):
            holder['cert_u'] = tbs + ec.compress(C, PU)
            return ecqv.e_sec4(holder['cert_u'])

        PU, r, e = ecqv.issue(RU, dca, k, e_of)
        QCA = ec.base_mul(C, dca)
        du = ecqv.private_key(e, ku, r)
        QU = ecqv.public_key(e, PU, QCA)
        ok, d = _cmp({'RU': (RU, (int(o['rux'], 16), int(o['ruy'], 16))), 'QCA': (QCA, (int(o['qcax'], 16), int(o['qcay'], 16))),
                      'PU': (PU, (int(o['pux'], 16), int(o['puy'], 16))), 'cert_u': (holder['cert_u'].hex(), o['cert_u']),
                      'r': ('%064x' % r, o['r']), 'du': ('%064x' % du, o['du']), 'QU': (QU, (int(o['qux'], 16), int(o['quy'], 16))),
                      'du*G=QU': (ec.base_mul(C, du), QU)})
        out.append((v['id'], ok, d))
    return out


def check_ecqv_1609():
    """No public 1609.2-profile ECQV vector exists (02 section 14): check the e formula against a
    direct hashlib computation, and the key relations, on fixed inputs."""
    out = []
    C = ec.P256
    for n in range(3):
        tbs = hashlib.sha256(b'tbs%d' % n).digest() * 3
        issuer = hashlib.sha256(b'iss%d' % n).digest() * 5
        whole = b'\x80\x03' + tbs + b'\x00'
        want = int.from_bytes(hashlib.sha256(hashlib.sha256(tbs).digest() + hashlib.sha256(issuer).digest()).digest(), 'big')
        got = ecqv.e_1609(tbs, issuer, whole)
        ku, dca, k = 1000 + n, 2000 + n, 3000 + n
        PU, r, e = ecqv.issue(ec.base_mul(C, ku), dca, k, lambda PU: ecqv.e_1609(tbs, issuer, whole))
        du = ecqv.private_key(e, ku, r)
        ok, d = _cmp({'e': (got, want), 'du*G=QU': (ec.base_mul(C, du), ecqv.public_key(e, PU, ec.base_mul(C, dca)))})
        out.append((f'ecqv-1609 #{n}', ok, d))
    return out


def check_time32():
    out = []
    for t, want in time1609.KNOWN:
        got = time1609.time32(t)
        back = time1609.from_time32(want)
        out.append((t.isoformat(), got == want and back == t, f'got {got}, want {want}, back {back}'))
    t = dt.datetime(2020, 2, 29, 12, 0, 0, 123456, tzinfo=dt.timezone.utc)
    out.append(('time64', time1609.time64(t) == time1609.time32(t.replace(microsecond=0)) * 1_000_000 + 123456, ''))
    return out


def check_ec_cross():
    return [('k*G vs cryptography', ec.cross_check(), '')]


def check_lib_cross():
    r = prims.cross_check_libs()
    return [('hmac/kdf2/ccm vs libraries', r is None, r or '')]


CHECKS = {
    'ieee-d6.1': lambda: check_ccm('ieee1609dot2a_d6_1_aes_ccm.json'),
    'ieee-d6.2': lambda: check_ecies('ieee1609dot2a_d6_2_ecies.json'),
    'ieee-d6.2-erratum': lambda: check_ecies_erratum('ieee1609dot2a_d6_2_ecies.json'),
    'ieee-d6.3': lambda: check_mac1('ieee1609dot2a_d6_3_mac1.json'),
    'ieee-d6.4': lambda: check_kdf('ieee1609dot2a_d6_4_kdf2.json'),
    'ieee-hashedid': check_hashedid,
    'camp-aesccm': lambda: check_ccm('camp_aesccm.json'),
    'camp-ecies': lambda: check_ecies('camp_ecies.json'),
    'camp-ecies-erratum': lambda: check_ecies_erratum('camp_ecies.json'),
    'camp-mac1': lambda: check_mac1('camp_mac1.json'),
    'camp-kdf': lambda: check_kdf('camp_kdf.json'),
    'camp-lv': check_lv,
    'camp-glv': check_glv,
    'camp-bfkeyexp': check_bfkeyexp,
    'camp-implicit': check_implicit_sec4,
    'nist-ecdsa': check_sigver,
    'nist-ecdsa-range': check_ecdsa_range,
    'nist-sha': check_sha,
    'nist-x963': check_x963,
    'nist-ccm': check_nist_ccm,
    'rfc4231': check_rfc4231,
    'oracle-ecqv-1609': check_ecqv_1609,
    'time32': check_time32,
    'ec-cross': check_ec_cross,
    'lib-cross': check_lib_cross,
}


def run_all(names=None):
    res = {}
    for name, fn in CHECKS.items():
        if names and name not in names:
            continue
        try:
            res[name] = fn()
        except Exception as e:  # a crash counts as red
            res[name] = [('<exception>', False, f'{type(e).__name__}: {e}')]
    return res
