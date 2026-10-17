"""Transcribe the official and CAMP vectors into the JSON corpus read by the oracle and the Rust tests.

Run once by hand when the corpus is (re)built; nothing here runs at test time.

    python -I tools/oracle/transcribe/transcribe.py \
        --ieee2a-txt <text extraction of IEEE_Std_1609_2a-2017.pdf> --ieee2a-pdf <the pdf> \
        --camp <dir with the CAMP files at commit dd5014e6> --nist <dir with the unzipped CAVP files> \
        --rfc4231 <rfc4231.txt> --out crates/v2xw-sec/tests/vectors

Every vector carries a citation (source, clause or file, line numbers of the transcribed values)
and every file carries the sha256 of the source it was transcribed from.  Values are copied, never
computed: the oracle (tools/oracle) recomputes them independently and must agree.

The IEEE text and the CAMP files have no redistribution licence, so only the values (facts needed
for interoperability testing) are transcribed, with citations; the documents are not vendored.
NIST CAVP files are US-government works (public domain); a selected subset is transcribed.
"""

import argparse
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hexdump_parse as hp  # noqa: E402

TRANSCRIBED = '2026-10-16'
CAMP_REPO = 'https://github.com/conz27/crypto-test-vectors'
CAMP_COMMIT = 'dd5014e6c3136895744f1fe73734fa3592f37bd4'
IEEE2A_URL = 'https://docbox.etsi.org/STF/Archive/STF538_TC_ITS/STFworkarea/libaries/IEEE_Std_1609_2a-2017.pdf'
NIST_BASE = 'https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/'


def sha256_file(p):
    return hashlib.sha256(open(p, 'rb').read()).hexdigest()


def read_lines(p):
    return open(p, encoding='utf-8', errors='replace').read().split('\n')


def lo(h):
    return h.lower()


def write(out, name, doc):
    p = os.path.join(out, name)
    with open(p, 'w') as f:
        json.dump(doc, f, indent=1, sort_keys=False)
        f.write('\n')
    print(f'{name}: {len(doc["vectors"])} vectors')


def take(recs, spec, where):
    """Map records to keys by position, asserting each label starts with the expected prefix."""
    if len(recs) != len(spec):
        raise SystemExit(f'{where}: expected {len(spec)} values, found {len(recs)}: {[r["label"] for r in recs]}')
    out, lines = {}, {}
    for r, (key, prefix) in zip(recs, spec):
        if not r['label'].startswith(prefix):
            raise SystemExit(f'{where}: label {r["label"]!r} does not start with {prefix!r}')
        if r['array_ok'] is False:
            raise SystemExit(f'{where}: {key} hex and C-array repeat disagree')
        out[key] = lo(r['hex'])
        lines[key] = r['line']
    return out, lines


def sections(lines, start, end, header_re):
    """Split [start, end) (0-based) into (title, first_line_index, end_index) at lines matching header_re."""
    idx = [i for i in range(start, end) if re.match(header_re, lines[i].strip())]
    return [(lines[a].strip(), a, b) for a, b in zip(idx, idx[1:] + [end])]


# ----------------------------------------------------------------------------------------------
# IEEE 1609.2a-2017 Annex D.6


def ieee_pages(lines):
    """Printed page number for each line: the number in the first page footer at or after it."""
    page = [None] * len(lines)
    cur = None
    for i in range(len(lines) - 1, -1, -1):
        if lines[i].strip().startswith('Copyright ©') and i + 1 < len(lines):
            m = re.fullmatch(r'(\d{1,3})', lines[i + 1].strip())
            if m:
                cur = int(m.group(1))
        page[i] = cur
    return page


def ieee_d6(args):
    lines = read_lines(args.ieee2a_txt)
    pages = ieee_pages(lines)
    start = next(i for i, l in enumerate(lines) if l.strip() == 'Insert D.6 as follows:')
    heads = {}
    for i in range(start, len(lines)):
        m = re.match(r'^(D\.6\.[1-4]) ', lines[i].strip())
        if m and m.group(1) not in heads:
            heads[m.group(1)] = i
    bounds = {'D.6.1': (heads['D.6.1'], heads['D.6.2']), 'D.6.2': (heads['D.6.2'], heads['D.6.3']),
              'D.6.3': (heads['D.6.3'], heads['D.6.4']), 'D.6.4': (heads['D.6.4'], len(lines))}
    src = {'title': 'IEEE Std 1609.2a-2017, Amendment 1 to IEEE Std 1609.2-2016, Annex D.6 "Cryptographic test vectors"',
           'id': 'S4 (docs/design/scms-real/02-cryptography-and-formats.md section 0)',
           'url': IEEE2A_URL, 'sha256': sha256_file(args.ieee2a_pdf),
           'licence': 'IEEE copyright, no redistribution licence: values transcribed with citations, document not vendored',
           'transcribed': TRANSCRIBED,
           'method': 'tools/oracle/transcribe/transcribe.py from a text extraction of the PDF; every value '
                     'cross-checked against the C-array repeat printed beside it in the standard'}

    def cite(clause, title, recs):
        ls = [r['line'] for r in recs]
        pg = sorted({pages[l - 1] for l in ls if pages[l - 1]})
        return f'IEEE 1609.2a-2017 {clause} {title}, printed page(s) {"-".join(map(str, [pg[0], pg[-1]] if len(pg) > 1 else pg))}'

    # D.6.1 AES-CCM-128
    vec = []
    a, b = bounds['D.6.1']
    for title, s, e in sections(lines, a, b, r'^Test Vector #\d+:'):
        recs = hp.parse(lines, s, e)
        v, _ = take(recs, [('key', 'K'), ('nonce', 'N'), ('plaintext', 'P'), ('ciphertext_tag', 'C_T')], title)
        vec.append({'id': 'D.6.1' + title.split()[-1].rstrip(':'), 'cite': cite('D.6.1', title.rstrip(':'), recs),
                    'in': {k: v[k] for k in ('key', 'nonce', 'plaintext')}, 'out': {'ciphertext_tag': v['ciphertext_tag']}})
    write(args.out, 'ieee1609dot2a_d6_1_aes_ccm.json', {
        'corpus': 'ieee1609dot2a-d6.1-aes-128-ccm', 'source': src,
        'parameters': 'NIST SP 800-38C / RFC 3610, no associated data, tag 16 octets, nonce 12 octets, q = 3; output = C || T',
        'vectors': vec})

    # D.6.2 ECIES (with the P1 print erratum)
    camp_ecies_py = read_lines(os.path.join(args.camp, 'ecies.py'))
    real_p1 = {}
    for i, l in enumerate(camp_ecies_py):
        m = re.match(r'^(p11|p12)\s*=\s*"([0-9A-Fa-f]{64})"', l)
        if m:
            real_p1[m.group(1)] = (lo(m.group(2)), i + 1)
    vec = []
    a, b = bounds['D.6.2']
    for n, (title, s, e) in enumerate(sections(lines, a, b, r'^Test Vector #\d+:'), 1):
        recs = hp.parse(lines, s, e)
        v, _ = take(recs, [('v', 'v'), ('k', 'k'), ('p1_printed', 'P1'), ('r', 'r'), ('rx', 'Rx'), ('ry', 'Ry'),
                           ('vx', 'Vx'), ('vy', 'Vy'), ('c', 'C'), ('t', 'T')], title)
        p1, p1_line = real_p1['p11' if n <= 2 else 'p12']
        vec.append({'id': f'D.6.2#{n}', 'cite': cite('D.6.2', title.rstrip(':'), recs),
                    'in': {'v': v['v'], 'k': v['k'], 'p1': p1, 'r': v['r'], 'rx': v['rx'], 'ry': v['ry']},
                    'out': {'vx': v['vx'], 'vy': v['vy'], 'c': v['c'], 't': v['t']},
                    'p1_printed': v['p1_printed'],
                    'p1_cite': f'CAMP ecies.py line {p1_line} ({CAMP_REPO} @ {CAMP_COMMIT[:12]}), variable {"p11" if n <= 2 else "p12"}'})
    write(args.out, 'ieee1609dot2a_d6_2_ecies.json', {
        'corpus': 'ieee1609dot2a-d6.2-ecies-p256', 'source': src,
        'parameters': 'IEEE 1609.2a section 5.3.5: ECIES over NIST P-256, non-DHAES, KDF2 with SHA-256 '
                      '(K = KDF2(x(v*R), 48, P1)), K1 = K[0:16] XOR-wraps the 16-byte AES key, '
                      'K2 = K[16:48], T = MAC1 = leftmost 16 bytes of HMAC-SHA-256(K2, C), P2 empty',
        'erratum': 'The printed P1 of every D.6.2 vector equals the AES key k (16 bytes): a print bug in the '
                   'generator (CAMP ecies.py prints k on the P1 line). in.p1 is the real 32-byte P1 that the '
                   'generator used (CAMP ecies.py p11/p12). With in.p1, C and T reproduce; with p1_printed they '
                   'must not (docs/design/scms-real/02-cryptography-and-formats.md section 8.2).',
        'vectors': vec})

    # D.6.3 MAC1
    vec = []
    a, b = bounds['D.6.3']
    for title, s, e in sections(lines, a, b, r'^Test Vector #\d+:'):
        recs = hp.parse(lines, s, e)
        v, _ = take(recs, [('key', 'K'), ('message', 'M'), ('tag', 'T')], title)
        vec.append({'id': 'D.6.3' + title.split()[-1].rstrip(':'), 'cite': cite('D.6.3', title.rstrip(':'), recs),
                    'in': {'key': v['key'], 'message': v['message']}, 'out': {'tag': v['tag']}})
    write(args.out, 'ieee1609dot2a_d6_3_mac1.json', {
        'corpus': 'ieee1609dot2a-d6.3-mac1', 'source': src,
        'parameters': 'MAC1 with SHA-256: HMAC-SHA-256 truncated to the leftmost 128 bits (the RFC 4231 inputs)',
        'vectors': vec})

    # D.6.4 KDF2: inputs from D.6.4; the outputs did not survive text extraction of the PDF, and are
    # taken from the NIST CAVP SP 800-135 ANSI X9.63 file, whose [SHA-256] entries carry the same
    # inputs (checked below), and cross-cited to CAMP kdf.py known_key1..4.
    nist = nist_x963_records(args)
    camp_kdf = read_lines(os.path.join(args.camp, 'kdf.py'))
    camp_keys = {}
    for i, l in enumerate(camp_kdf):
        m = re.match(r'^known_key(\d)\s*=\s*"([0-9a-fA-F]+)"', l)
        if m:
            camp_keys[int(m.group(1))] = (lo(m.group(2)), i + 1)
    vec = []
    a, b = bounds['D.6.4']
    for n, (title, s, e) in enumerate(sections(lines, a, b, r'^Test Vector #\d+:'), 1):
        recs = hp.parse(lines, s, e)
        block = '\n'.join(lines[s:e])
        mdl = re.search(r'dl = (\d+) octets', block)
        dl = int(mdl.group(1)) if mdl else None
        if 'kdp = ""' in block:
            v, _ = take(recs, [('ss', 'ss')], title)
            v['kdp'] = ''
        else:
            v, _ = take(recs, [('ss', 'ss'), ('kdp', 'kdp')], title)
        match = [r for r in nist if r['Z'] == v['ss'] and r['SharedInfo'] == v['kdp']
                 and (dl is None or len(r['key_data']) == 2 * dl)]
        dl_note = None
        if dl is None and len(match) == 1:
            dl = len(match[0]['key_data']) // 2
            dl_note = 'the dl line is not recoverable from the PDF text; dl taken from the NIST entry (equal to CAMP kdf.txt)'
        if len(match) != 1:
            raise SystemExit(f'D.6.4 {title}: no unique NIST X9.63 entry with the same inputs')
        key, kl = camp_keys[n]
        if key != match[0]['key_data']:
            raise SystemExit(f'D.6.4 {title}: NIST and CAMP outputs differ')
        vec.append({'id': f'D.6.4#{n}', 'cite': cite('D.6.4', title.rstrip(':'), recs),
                    'in': {'ss': v['ss'], 'kdp': v['kdp'], 'dl': dl}, 'out': {'key': key},
                    'out_cite': f'NIST CAVP ansx963_2001.rsp line {match[0]["line"]} ([SHA-256] COUNT = {match[0]["COUNT"]}); '
                                f'equal to CAMP kdf.py line {kl} known_key{n}',
                    **({'dl_note': dl_note} if dl_note else {})})
    write(args.out, 'ieee1609dot2a_d6_4_kdf2.json', {
        'corpus': 'ieee1609dot2a-d6.4-kdf2', 'source': src,
        'parameters': 'KDF2 (IEEE 1363a section 13.2) with SHA-256 = ANSI X9.63 KDF: '
                      'K = SHA-256(ss || 00000001 || kdp) || SHA-256(ss || 00000002 || kdp) || ..., truncated to dl',
        'note': 'The D.6.4 output lines are not recoverable from the PDF text; out.key comes from the NIST CAVP '
                'X9.63 file entry with identical inputs (see out_cite).',
        'vectors': vec})


# ----------------------------------------------------------------------------------------------
# NIST CAVP


def rsp_records(path):
    """Parse a CAVP .rsp: yields dicts of key = value with 'section' (list of [..] headers) and 'line'."""
    lines = read_lines(path)
    sect, cur, recs, hdr_open = [], None, [], False
    for i, l in enumerate(lines, 1):
        s = l.strip()
        if s.startswith('#'):
            continue
        if s.startswith('['):
            if not hdr_open:
                sect = []
                hdr_open = True
            sect.append(s.strip('[]'))
            cur = None
            continue
        hdr_open = False
        if not s:
            cur = None
            continue
        m = re.match(r'^([A-Za-z_0-9]+)\s*=\s*(.*)$', s)
        if not m:
            continue
        k, v = m.group(1), m.group(2).strip()
        if cur is None:
            cur = {'section': list(sect), 'line': i}
            recs.append(cur)
        cur[k] = v
    return recs


def nist_x963_records(args):
    out = []
    for r in rsp_records(os.path.join(args.nist, 'ansx963_2001.rsp')):
        if r['section'] and r['section'][0] == 'SHA-256' and 'Z' in r:
            out.append({'line': r['line'], 'COUNT': r.get('COUNT'), 'Z': lo(r['Z']),
                        'SharedInfo': lo(r.get('SharedInfo', '')), 'key_data': lo(r['key_data']),
                        'section': r['section']})
    return out


def nist_src(name, zipname, path):
    return {'title': f'NIST CAVP {name}', 'url': NIST_BASE + zipname, 'file': os.path.basename(path),
            'sha256': sha256_file(path), 'licence': 'US Government work (public domain); subset transcribed',
            'transcribed': TRANSCRIBED}


def nist(args):
    # ECDSA SigVer: P-256/SHA-256 and P-384/SHA-384
    p = os.path.join(args.nist, '186-4ecdsatestvectors', 'SigVer.rsp')
    vec = []
    for r in rsp_records(p):
        if 'Msg' not in r:
            continue
        sec = r['section'][0] if r['section'] else ''
        if sec not in ('P-256,SHA-256', 'P-384,SHA-384'):
            continue
        res = r['Result']
        vec.append({'id': f'{sec}@{r["line"]}', 'cite': f'SigVer.rsp line {r["line"]} [{sec}]',
                    'curve': sec.split(',')[0], 'hash': sec.split(',')[1],
                    'in': {'msg': lo(r['Msg']), 'qx': lo(r['Qx']), 'qy': lo(r['Qy']), 'r': lo(r['R']), 's': lo(r['S'])},
                    'out': {'valid': res.startswith('P'), 'result': res}})
    write(args.out, 'nist_ecdsa_sigver.json', {
        'corpus': 'nist-cavp-186-4-ecdsa-sigver', 'source': nist_src('FIPS 186-4 ECDSA SigVer', 'dss/186-4ecdsatestvectors.zip', p),
        'parameters': 'msg is hashed with the named hash; (r, s) verified under Q; out.valid per the Result line',
        'vectors': vec})

    # SHA-256 / SHA-384 short messages
    vec = []
    srcs = []
    for alg, fn in (('SHA-256', 'SHA256ShortMsg.rsp'), ('SHA-384', 'SHA384ShortMsg.rsp')):
        p = os.path.join(args.nist, 'shabytetestvectors', fn)
        srcs.append(nist_src(f'SHS byte-oriented {fn}', 'shs/shabytetestvectors.zip', p))
        for r in rsp_records(p):
            if 'Msg' not in r:
                continue
            ln = int(r['Len'])
            vec.append({'id': f'{alg}-len{ln}', 'cite': f'{fn} line {r["line"]}', 'hash': alg,
                        'in': {'msg': lo(r['Msg']) if ln else ''}, 'out': {'md': lo(r['MD'])}})
    write(args.out, 'nist_sha.json', {'corpus': 'nist-cavp-shs-short', 'source': srcs[0], 'sources': srcs, 'vectors': vec})

    # X9.63 KDF with SHA-256
    p = os.path.join(args.nist, 'ansx963_2001.rsp')
    vec = []
    for r in nist_x963_records(args):
        vec.append({'id': f'x963-sha256@{r["line"]}', 'cite': f'ansx963_2001.rsp line {r["line"]} [{"] [".join(r["section"])}] COUNT = {r["COUNT"]}',
                    'in': {'z': r['Z'], 'shared_info': r['SharedInfo'], 'key_data_len': len(r['key_data']) // 2},
                    'out': {'key_data': r['key_data']}})
    write(args.out, 'nist_x963_kdf.json', {
        'corpus': 'nist-cavp-sp800-135-ansx963-sha256',
        'source': nist_src('SP 800-135 ANSI X9.63-2001 KDF', 'components/800-135testvectors/ansx963_2001.zip', p),
        'vectors': vec})

    # AES-128-CCM: DVPT128 sections with Alen = 0 and Tlen = 16, and VNT128 Nlen = 12 (has Adata)
    vec = []
    p = os.path.join(args.nist, 'DVPT128.rsp')
    key = None
    for r in rsp_records(p):
        if 'Key' in r and len(r) == 3:
            key = lo(r['Key'])
            continue
        if 'Nonce' not in r:
            continue
        sec = r['section'][0]
        f = dict(x.split(' = ') for x in sec.split(', '))
        if f['Alen'] != '0' or f['Tlen'] != '16':
            continue
        ok = r['Result'] == 'Pass'
        vec.append({'id': f'DVPT128@{r["line"]}', 'cite': f'DVPT128.rsp line {r["line"]} [{sec}] Count = {r["Count"]}',
                    'kind': 'decrypt-verify',
                    'in': {'key': key, 'nonce': lo(r['Nonce']), 'adata': '', 'ct': lo(r['CT']), 'tlen': 16},
                    'out': {'valid': ok, 'payload': (lo(r['Payload']) if f['Plen'] != '0' else '') if ok else None}})
    p2 = os.path.join(args.nist, 'VNT128.rsp')
    key = None
    for r in rsp_records(p2):
        if 'Key' in r and 'Nonce' not in r:
            key = lo(r['Key'])
            continue
        if 'Nonce' not in r or r['section'] != ['Nlen = 12']:
            continue
        vec.append({'id': f'VNT128@{r["line"]}', 'cite': f'VNT128.rsp line {r["line"]} [Nlen = 12] Count = {r["Count"]}',
                    'kind': 'encrypt',
                    'in': {'key': key, 'nonce': lo(r['Nonce']), 'adata': lo(r['Adata']), 'payload': lo(r['Payload']), 'tlen': 16},
                    'out': {'ct': lo(r['CT'])}})
    write(args.out, 'nist_ccm.json', {
        'corpus': 'nist-cavp-ccm-aes128',
        'source': nist_src('CCM DVPT128 (Alen = 0, Tlen = 16)', 'mac/ccmtestvectors.zip', p),
        'sources': [nist_src('CCM DVPT128', 'mac/ccmtestvectors.zip', p), nist_src('CCM VNT128 (Nlen = 12, Alen = 32)', 'mac/ccmtestvectors.zip', p2)],
        'note': 'DVPT128 has no 12-byte-nonce section; its Alen = 0 / Tlen = 16 cases use 7- and 13-byte nonces. '
                'VNT128 [Nlen = 12] is the 12-byte nonce case but carries 32 bytes of associated data, which 1609.2 never uses.',
        'vectors': vec})


# ----------------------------------------------------------------------------------------------
# RFC 4231


def rfc4231(args):
    lines = read_lines(args.rfc4231)
    vec = []
    starts = [i for i, l in enumerate(lines) if re.match(r'^4\.[2-8]\.  Test Case \d', l)]
    for a, b in zip(starts, starts[1:] + [len(lines)]):
        tc = int(lines[a].split()[-1])
        fields, cur, cl = {}, None, {}
        for i in range(a + 1, b):
            l = lines[i]
            m = re.match(r'^   (Key|Data|HMAC-SHA-\d+)\s*=?\s+([0-9a-f]+)', l)
            if m:
                cur = m.group(1)
                fields[cur] = m.group(2)
                cl[cur] = i + 1
                continue
            m = re.match(r'^\s{10,}([0-9a-f]{2,})(\s|$)', l)
            if cur and m:
                fields[cur] += m.group(1)
                continue
            if l.strip().startswith('Key') or l.strip().startswith('Data') or not l.strip():
                cur = None if not l.strip() else cur
        if 'HMAC-SHA-256' not in fields:
            raise SystemExit(f'RFC 4231 case {tc}: no HMAC-SHA-256')
        vec.append({'id': f'RFC4231-TC{tc}', 'cite': f'RFC 4231 section 4.{tc + 1} Test Case {tc}, lines {cl["Key"]}-{cl["HMAC-SHA-256"]}',
                    'in': {'key': fields['Key'], 'data': fields['Data']},
                    'out': {'hmac_sha256': fields['HMAC-SHA-256']},
                    'truncated_bits': 128 if tc == 5 else None})
    write(args.out, 'rfc4231_hmac_sha256.json', {
        'corpus': 'rfc4231-hmac-sha256',
        'source': {'title': 'RFC 4231, Identifiers and Test Vectors for HMAC-SHA-224, HMAC-SHA-256, HMAC-SHA-384, and HMAC-SHA-512',
                   'url': 'https://www.rfc-editor.org/rfc/rfc4231.txt', 'sha256': sha256_file(args.rfc4231),
                   'licence': 'IETF Trust; test values transcribed with citation', 'transcribed': TRANSCRIBED},
        'note': 'Test Case 5 publishes only the output truncated to 128 bits.',
        'vectors': vec})


# ----------------------------------------------------------------------------------------------
# IEEE 1609.2-2022 ASN.1 comment examples (HashedIdN of the empty string)


def ieee_examples(args):
    p = os.path.join(os.path.dirname(HERE), 'asn1', 'ieee1609.2-2022', 'Ieee1609Dot2BaseTypes.asn')
    lines = open(p, encoding='cp1252').read().split('\n')
    vec = []
    for i, l in enumerate(lines):
        m = re.search(r'Example: Consider the (SHA-\d+) hash of the empty string', l)
        if not m:
            continue
        end = next(k for k in range(i, i + 20) if lines[k].strip() == '*/')
        text = '\n'.join(re.sub(r'^\s*\*\s?', '', x).rstrip() for x in lines[i:end])
        full = re.search(r'SHA-\d+\(""\)\s*=\s*([0-9a-f\n]+)', text)
        hid = re.search(r'(HashedId\d+)\s*=\s*([0-9a-f\n]+)\.', text)
        if not (full and hid):
            raise SystemExit(f'Ieee1609Dot2BaseTypes.asn line {i + 1}: example not parsed')
        vec.append({'id': f'{hid.group(1)}-empty', 'cite': f'tools/oracle/asn1/ieee1609.2-2022/Ieee1609Dot2BaseTypes.asn lines {i + 1}-{end}',
                    'hash': m.group(1), 'in': {'data': ''},
                    'out': {'digest': full.group(1).replace('\n', ''), 'hashed_id': hid.group(2).replace('\n', ''), 'type': hid.group(1)}})
    write(args.out, 'ieee1609dot2_hashedid_examples.json', {
        'corpus': 'ieee1609dot2-2022-hashedid-examples',
        'source': {'title': 'IEEE 1609.2-2022 ASN.1 module Ieee1609Dot2BaseTypes v2.4, comments on HashedId3/8/10/32/48',
                   'url': 'https://forge.etsi.org/rep/ITS/asn1/ieee1609.2/-/raw/2022-published/Ieee1609Dot2BaseTypes.asn',
                   'sha256': sha256_file(p), 'transcribed': TRANSCRIBED},
        'vectors': vec})


# ----------------------------------------------------------------------------------------------
# CAMP


def camp_src(args, name):
    p = os.path.join(args.camp, name)
    return {'title': f'CAMP SCMS crypto test vectors, {name}', 'id': 'S6', 'url': f'{CAMP_REPO}/blob/{CAMP_COMMIT}/{name}',
            'commit': CAMP_COMMIT, 'sha256': sha256_file(p),
            'licence': 'none (repository has no licence file): values transcribed with citations, files not vendored',
            'transcribed': TRANSCRIBED}


def camp_cite(name, recs):
    ls = [r['line'] for r in recs]
    return f'CAMP {name} lines {min(ls)}-{max(ls)}'


def camp_simple(args, name, spec, keys_in, keys_out, out_name, corpus, extra=None):
    lines = read_lines(os.path.join(args.camp, name))
    vec = []
    for title, s, e in sections(lines, 0, len(lines), r'^Test Vector #\d+:'):
        recs = hp.parse(lines, s, e)
        v, _ = take(recs, spec, f'{name} {title}')
        d = {'id': f'{name}{title.split()[-1].rstrip(":")}', 'cite': camp_cite(name, recs),
             'in': {k: v[k] for k in keys_in}, 'out': {k: v[k] for k in keys_out}}
        if extra:
            extra(d, v, lines, s, e)
        vec.append(d)
    doc = {'corpus': corpus, 'source': camp_src(args, name), 'vectors': vec}
    return doc


def camp(args):
    # aesccm, mac1 (same layout as D.6.1 / D.6.3)
    write(args.out, 'camp_aesccm.json', camp_simple(
        args, 'aesccm.txt', [('key', 'K'), ('nonce', 'N'), ('plaintext', 'P'), ('ciphertext_tag', 'C_T')],
        ('key', 'nonce', 'plaintext'), ('ciphertext_tag',), None, 'camp-aesccm'))
    write(args.out, 'camp_mac1.json', camp_simple(
        args, 'mac1.txt', [('key', 'K'), ('message', 'M'), ('tag', 'T')], ('key', 'message'), ('tag',), None, 'camp-mac1'))

    # ecies (P1 misprint, real P1 from ecies.py)
    py = read_lines(os.path.join(args.camp, 'ecies.py'))
    real = {}
    for i, l in enumerate(py):
        m = re.match(r'^(p11|p12)\s*=\s*"([0-9A-Fa-f]{64})"', l)
        if m:
            real[m.group(1)] = (lo(m.group(2)), i + 1)
    doc = camp_simple(args, 'ecies.txt', [('v', 'v'), ('k', 'k'), ('p1_printed', 'P1'), ('r', 'r'), ('rx', 'Rx'), ('ry', 'Ry'),
                                          ('vx', 'Vx'), ('vy', 'Vy'), ('c', 'C'), ('t', 'T')],
                      ('v', 'k', 'r', 'rx', 'ry'), ('vx', 'vy', 'c', 't'), None, 'camp-ecies-p256')
    for n, d in enumerate(doc['vectors'], 1):
        p1, ln = real['p11' if n <= 2 else 'p12']
        d['in']['p1'] = p1
        d['p1_printed'] = None
        d['p1_cite'] = f'CAMP ecies.py line {ln} ({"p11" if n <= 2 else "p12"}); ecies.py line 179 prints k on the P1 line'
    lines = read_lines(os.path.join(args.camp, 'ecies.txt'))
    for n, (title, s, e) in enumerate(sections(lines, 0, len(lines), r'^Test Vector #\d+:'), 1):
        recs = hp.parse(lines, s, e)
        doc['vectors'][n - 1]['p1_printed'] = lo(next(r['hex'] for r in recs if r['label'] == 'P1'))
    doc['erratum'] = 'Printed P1 = k (generator print bug); in.p1 is the real P1 from ecies.py.'
    write(args.out, 'camp_ecies.json', doc)

    # kdf: inputs in kdf.txt, outputs in kdf.py known_key1..4
    py = read_lines(os.path.join(args.camp, 'kdf.py'))
    keys = {}
    for i, l in enumerate(py):
        m = re.match(r'^known_key(\d)\s*=\s*"([0-9a-fA-F]+)"', l)
        if m:
            keys[int(m.group(1))] = (lo(m.group(2)), i + 1)
    lines = read_lines(os.path.join(args.camp, 'kdf.txt'))
    vec = []
    for n, (title, s, e) in enumerate(sections(lines, 0, len(lines), r'^Test Vector #\d+:'), 1):
        recs = hp.parse(lines, s, e)
        block = '\n'.join(lines[s:e])
        dl = int(re.search(r'dl = (\d+) octets', block).group(1))
        if 'kdp = ""' in block:
            v, _ = take(recs, [('ss', 'ss')], title)
            v['kdp'] = ''
        else:
            v, _ = take(recs, [('ss', 'ss'), ('kdp', 'kdp')], title)
        vec.append({'id': f'kdf.txt#{n}', 'cite': f'{camp_cite("kdf.txt", recs)}; output kdf.py line {keys[n][1]} known_key{n}',
                    'in': {'ss': v['ss'], 'kdp': v['kdp'], 'dl': dl}, 'out': {'key': keys[n][0]}})
    write(args.out, 'camp_kdf.json', {'corpus': 'camp-kdf2', 'source': camp_src(args, 'kdf.txt'),
                                      'sources': [camp_src(args, 'kdf.txt'), camp_src(args, 'kdf.py')], 'vectors': vec})

    # lv
    lines = read_lines(os.path.join(args.camp, 'lv.txt'))
    allr = hp.parse(lines)
    la1, la2 = lo(allr[0]['hex']), lo(allr[1]['hex'])
    assert allr[0]['label'].startswith('LA1 ID') and allr[1]['label'].startswith('LA2 ID')
    vec = []
    secs = sections(lines, 0, len(lines), r'^i = \d+$')
    ls_prev = {}
    for title, s, e in secs:
        i = int(title.split()[-1])
        recs = hp.parse(lines, s, e)
        if i == 0:
            v, _ = take(recs, [('j', 'j'), ('ls1', 'ls1(0)'), ('aes_in1', 'AES input'), ('aes_out1', 'AES output'), ('plv1', 'plv1(0,j)'),
                               ('ls2', 'ls2(0)'), ('aes_in2', 'AES input'), ('aes_out2', 'AES output'), ('plv2', 'plv2(0,j)'), ('lv', 'lv(0,j)')], f'lv i={i}')
            ls_prev = {'ls1_0': v['ls1'], 'ls2_0': v['ls2']}
            vin = {'la_id1': la1, 'la_id2': la2, 'ls1_0': v['ls1'], 'ls2_0': v['ls2'], 'i': 0, 'j': int(v['j'], 16)}
            vout = {k: v[k] for k in ('aes_in1', 'aes_out1', 'plv1', 'aes_in2', 'aes_out2', 'plv2', 'lv')}
            vout.update({'ls1_i': v['ls1'], 'ls2_i': v['ls2']})
        else:
            v, _ = take(recs, [('j', 'j'), ('sha_in1', 'SHA-256 input'), ('sha_out1', 'SHA-256 output'), ('ls1', 'ls1(1)'), ('aes_in1', 'AES input'),
                               ('aes_out1', 'AES output'), ('plv1', 'plv1(1,j)'), ('sha_in2', 'SHA-256 input'), ('sha_out2', 'SHA-256 output'),
                               ('ls2', 'ls2(1)'), ('aes_in2', 'AES input'), ('aes_out2', 'AES output'), ('plv2', 'plv2(1,j)'), ('lv', 'lv(1,j)')], f'lv i={i}')
            vin = {'la_id1': la1, 'la_id2': la2, 'ls1_0': ls_prev['ls1_0'], 'ls2_0': ls_prev['ls2_0'], 'i': 1, 'j': int(v['j'], 16)}
            vout = {k: v[k] for k in ('sha_in1', 'sha_out1', 'aes_in1', 'aes_out1', 'plv1', 'sha_in2', 'sha_out2', 'aes_in2', 'aes_out2', 'plv2', 'lv')}
            vout.update({'ls1_i': v['ls1'], 'ls2_i': v['ls2']})
        vec.append({'id': f'lv.txt i={i}', 'cite': f'CAMP lv.txt lines 7-10 (LA ids), {camp_cite("lv.txt", recs)[9:]}', 'in': vin, 'out': vout})
    write(args.out, 'camp_lv.json', {
        'corpus': 'camp-linkage-values', 'source': camp_src(args, 'lv.txt'),
        'parameters': 'ls(i) = first 16 bytes of SHA-256(la_id || ls(i-1) || 0^112); '
                      'plv(i,j) = first 9 bytes of AES_ls(i)(la_id || j || 0^80) XOR (la_id || j || 0^80); lv = plv1 XOR plv2; '
                      'la_id 16 bits, j 32 bits',
        'vectors': vec})

    # glv
    lines = read_lines(os.path.join(args.camp, 'glv.txt'))
    allr = hp.parse(lines)
    head, _ = take(allr[:4], [('la_id1', 'LA1 ID'), ('la_id2', 'LA2 ID'), ('k', 'k:'), ('ik', 'ik:')], 'glv header')
    vec = []
    gs_prev = {}
    for title, s, e in sections(lines, 0, len(lines), r'^i = \d+$'):
        i = int(title.split()[-1])
        recs = hp.parse(lines, s, e)
        if i == 0:
            v, _ = take(recs, [('j', 'j'), ('gs1', 'gs1(0)'), ('aes_in1', 'AES input'), ('aes_out1', 'AES output'), ('gplv1', 'gplv1(0,j)'),
                               ('gs2', 'gs2(0)'), ('aes_in2', 'AES input'), ('aes_out2', 'AES output'), ('gplv2', 'gplv2(0,j)'), ('glv', 'glv(0,j)'),
                               ('ei_in', 'Encrypted indices function AES input'), ('ei_aes_out', 'AES output'), ('ei', 'ei(j,k)')], f'glv i={i}')
            gs_prev = {'gs1_0': v['gs1'], 'gs2_0': v['gs2']}
            vout = {k: v[k] for k in ('aes_in1', 'aes_out1', 'gplv1', 'aes_in2', 'aes_out2', 'gplv2', 'glv', 'ei_in', 'ei_aes_out', 'ei')}
        else:
            v, _ = take(recs, [('j', 'j'), ('sha_in1', 'SHA-256 input'), ('sha_out1', 'SHA-256 output'), ('gs1', 'gs1(1)'), ('aes_in1', 'AES input'),
                               ('aes_out1', 'AES output'), ('gplv1', 'gplv1(1,j)'), ('sha_in2', 'SHA-256 input'), ('sha_out2', 'SHA-256 output'),
                               ('gs2', 'gs2(1)'), ('aes_in2', 'AES input'), ('aes_out2', 'AES output'), ('gplv2', 'gplv2(1,j)'), ('glv', 'glv(1,j)'),
                               ('ei_in', 'Encrypted indices function AES input'), ('ei_aes_out', 'AES output'), ('ei', 'ei(j,k)')], f'glv i={i}')
            vout = {k: v[k] for k in ('sha_in1', 'sha_out1', 'aes_in1', 'aes_out1', 'gplv1', 'sha_in2', 'sha_out2', 'aes_in2', 'aes_out2', 'gplv2',
                                      'glv', 'ei_in', 'ei_aes_out', 'ei')}
        vout.update({'gs1_i': v['gs1'], 'gs2_i': v['gs2']})
        vin = {'la_id1': head['la_id1'], 'la_id2': head['la_id2'], 'gs1_0': gs_prev['gs1_0'], 'gs2_0': gs_prev['gs2_0'],
               'i': i, 'j': int(v['j'], 16), 'k': int(head['k'], 16), 'ik': head['ik']}
        vec.append({'id': f'glv.txt i={i}', 'cite': f'CAMP glv.txt lines 11-20 (LA ids, k, ik), {camp_cite("glv.txt", recs)[10:]}', 'in': vin, 'out': vout})
    write(args.out, 'camp_glv.json', {
        'corpus': 'camp-group-linkage-values', 'source': camp_src(args, 'glv.txt'),
        'note': 'glv.txt labels the encrypted-indices input "j || k || 0^64", but the printed bytes are k || j || 0^64 '
                '(k = 5A6709EA first). out.ei_in is transcribed as printed; the oracle uses the printed byte order.',
        'vectors': vec})

    # bfkeyexp: signing (i, j, ck, a) and encryption (ek, h); A, H, expanded public keys only as python long text
    lines = read_lines(os.path.join(args.camp, 'bfkeyexp.txt'))
    allr = hp.parse(lines)
    sign_spec = [('i', 'i (32 bits)'), ('j', 'j (in range'), ('ck', 'ck:'), ('a', 'a:'), ('x', 'x_cert'),
                 ('x1', 'x+1'), ('aes1', 'AES_k(x+1):'), ('b1', 'AES_k(x+1) XOR'), ('x2', 'x+2'), ('aes2', 'AES_k(x+2):'), ('b2', 'AES_k(x+2) XOR'),
                 ('x3', 'x+3'), ('aes3', 'AES_k(x+3):'), ('b3', 'AES_k(x+3) XOR'), ('f_int', 'f_k^{int}'), ('f', 'f_k(x)'), ('a_exp', 'a + f_k')]
    enc_spec = [('ek', 'ek:'), ('h', 'h:'), ('x', 'x_enc'),
                ('x1', 'x+1'), ('aes1', 'AES_k(x+1):'), ('b1', 'AES_k(x+1) XOR'), ('x2', 'x+2'), ('aes2', 'AES_k(x+2):'), ('b2', 'AES_k(x+2) XOR'),
                ('x3', 'x+3'), ('aes3', 'AES_k(x+3):'), ('b3', 'AES_k(x+3) XOR'), ('f_int', 'f_k^{int}'), ('f', 'f_k(x)'), ('h_exp', 'h + f_k')]
    sv, _ = take(allr[:len(sign_spec)], sign_spec, 'bfkeyexp sign')
    ev, _ = take(allr[len(sign_spec):], enc_spec, 'bfkeyexp enc')
    # public points printed as "[0x..L], [0x..L]" python longs after a label line
    pts = {}
    for n, l in enumerate(lines):
        m = re.match(r'^\[0x([0-9a-f]+)L\], \[0x([0-9a-f]+)L\]$', l.strip())
        if m:
            label = lines[n - 1].strip()
            pts[label.split(':')[0].split(' mod')[0].strip()] = (m.group(1).rjust(64, '0'), m.group(2).rjust(64, '0'), n + 1)
    need = {'A': 'A', 'A + f_k(x_cert)*G_P256': 'A_exp', 'H': 'H', 'H + f_k(x_enc)*G_P256': 'H_exp'}
    P = {}
    for k, name in need.items():
        if k not in pts:
            raise SystemExit(f'bfkeyexp: point {k} not found; have {list(pts)}')
        P[name] = {'x': pts[k][0], 'y': pts[k][1], 'line': pts[k][2]}
    rs = allr[:len(sign_spec)]
    re_ = allr[len(sign_spec):]
    vec = [
        {'id': 'bfkeyexp sign', 'cite': f'{camp_cite("bfkeyexp.txt", rs)}; A at line {P["A"]["line"]}, A_exp at line {P["A_exp"]["line"]}',
         'kind': 'signing', 'in': {'i': int(sv['i'], 16), 'j': int(sv['j'], 16), 'key': sv['ck'], 'seed_private': sv['a'],
                                   'seed_public_x': P['A']['x'], 'seed_public_y': P['A']['y']},
         'out': {'x': sv['x'], 'x1': sv['x1'], 'x2': sv['x2'], 'x3': sv['x3'], 'aes1': sv['aes1'], 'aes2': sv['aes2'], 'aes3': sv['aes3'],
                 'f_int': sv['f_int'], 'f': sv['f'], 'expanded_private': sv['a_exp'],
                 'expanded_public_x': P['A_exp']['x'], 'expanded_public_y': P['A_exp']['y']}},
        {'id': 'bfkeyexp enc', 'cite': f'{camp_cite("bfkeyexp.txt", re_)}; H at line {P["H"]["line"]}, H_exp at line {P["H_exp"]["line"]}; i, j from lines 10-13',
         'kind': 'encryption', 'in': {'i': int(sv['i'], 16), 'j': int(sv['j'], 16), 'key': ev['ek'], 'seed_private': ev['h'],
                                      'seed_public_x': P['H']['x'], 'seed_public_y': P['H']['y']},
         'out': {'x': ev['x'], 'x1': ev['x1'], 'x2': ev['x2'], 'x3': ev['x3'], 'aes1': ev['aes1'], 'aes2': ev['aes2'], 'aes3': ev['aes3'],
                 'f_int': ev['f_int'], 'f': ev['f'], 'expanded_private': ev['h_exp'],
                 'expanded_public_x': P['H_exp']['x'], 'expanded_public_y': P['H_exp']['y']}},
    ]
    write(args.out, 'camp_bfkeyexp.json', {
        'corpus': 'camp-butterfly-expansion', 'source': camp_src(args, 'bfkeyexp.txt'),
        'parameters': 'x_sign = 00000000 || i || j || 00000000, x_enc = FFFFFFFF || i || j || 00000000 (32-bit big-endian fields); '
                      'f_int = (AES_k(x+1) XOR (x+1)) || (AES_k(x+2) XOR (x+2)) || (AES_k(x+3) XOR (x+3)); f = f_int mod n (P-256); '
                      'expanded private = seed + f mod n, expanded public = seed_public + f*G',
        'note': 'bfkeyexp.txt labels h as "Signing seed private key" in the encryption half; it is the encryption seed private key.',
        'vectors': vec})

    # implicit (SEC 4 form, NOT the 1609.2 profile)
    doc = camp_simple(args, 'implicit.txt',
                      [('tbs_cert', 'tbsCert'), ('ku', 'kU'), ('rux', 'RUx'), ('ruy', 'RUy'), ('dca', 'dCA'), ('qcax', 'QCAx'), ('qcay', 'QCAy'),
                       ('k', 'k'), ('pux', 'PUx'), ('puy', 'PUy'), ('cert_u', 'CertU'), ('r', 'r'), ('du', 'dU'), ('qux', 'QUx'), ('quy', 'QUy')],
                      ('tbs_cert', 'ku', 'dca', 'k'), ('rux', 'ruy', 'qcax', 'qcay', 'pux', 'puy', 'cert_u', 'r', 'du', 'qux', 'quy'),
                      None, 'camp-implicit-sec4')
    doc['profile'] = ('SEC 4 algebra with the CAMP illustration encoding: CertU = tbsCert || compressed(PU), '
                      'e = floor(SHA-256(CertU) / 2) (SEC 4 leftmost 255 bits). This is NOT the IEEE 1609.2 profile, which uses '
                      'the full 256-bit e = SHA-256(SHA-256(tbsCert) || SHA-256(issuer cert)); see 02 section 6.')
    write(args.out, 'camp_implicit.json', doc)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ieee2a-txt', required=True)
    ap.add_argument('--ieee2a-pdf', required=True)
    ap.add_argument('--camp', required=True)
    ap.add_argument('--nist', required=True)
    ap.add_argument('--rfc4231', required=True)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    ieee_d6(args)
    nist(args)
    rfc4231(args)
    ieee_examples(args)
    camp(args)


if __name__ == '__main__':
    main()
