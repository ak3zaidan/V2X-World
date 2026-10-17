"""Emit tests/vectors/p2pcd_response.json: Ieee1609dot2Peer2PeerPDU encodings made by asn1tools.

WP-C3. The encoder here is asn1tools compiling the published IEEE 1609.2-2022 ASN.1
(tools/oracle/asn1/ieee1609.2-2022/Ieee1609Dot2Peer2Peer.asn and its imports), through the
oracle's COER loader (tools/oracle/scms_oracle/coer.py, used read-only). Nothing in this
script reads or mirrors the Rust encoder it checks (crates/v2xw-sec/src/p2pcd.rs).

The certificates are the oracle's own (tests/vectors/oracle/chain_explicit.json): its ACA
certificate (issued by its root, so a one-element caCerts is a complete response) and one
explicit end-entity certificate, to exercise a two-element SEQUENCE OF.

Run:  python -I crates/v2xw-sec/tests/data/gen_p2pcd_response_vectors.py <repo root>
"""
import hashlib
import json
import os
import sys

root = os.path.abspath(sys.argv[1])
sys.path.insert(0, os.path.join(root, 'tools', 'oracle'))
from scms_oracle import coer  # noqa: E402

src_rel = 'crates/v2xw-sec/tests/vectors/oracle/chain_explicit.json'
src = json.load(open(os.path.join(root, src_rel)))
aca = bytes.fromhex(src['aca_coer'])
ee = bytes.fromhex(src['vectors'][0]['cert_coer'])


def pdu(certs):
    value = {'version': 1, 'content': ('caCerts', [coer.decode('Certificate', c) for c in certs])}
    out = coer.encode('Ieee1609dot2Peer2PeerPDU', value)
    # Round trip through the same compiled module: the bytes are canonical for it.
    assert coer.roundtrip('Ieee1609dot2Peer2PeerPDU', out)
    return out


vectors = []
for name, certs in [('aca-only', [aca]), ('ee-then-aca', [ee, aca])]:
    vectors.append({
        'name': name,
        'certs_coer': [c.hex() for c in certs],
        'pdu_coer': pdu(certs).hex(),
    })

doc = {
    'generator': 'crates/v2xw-sec/tests/data/gen_p2pcd_response_vectors.py (WP-C3, asn1tools)',
    'edition': '1609.2-2022',
    'module': 'Ieee1609Dot2Peer2Peer {... peer-to-peer(1) major-version-2(2) minor-version-3(3)}',
    'source': src_rel,
    'source_sha256': hashlib.sha256(open(os.path.join(root, src_rel), 'rb').read()).hexdigest(),
    'vectors': vectors,
}
out = os.path.join(root, 'crates/v2xw-sec/tests/vectors/p2pcd_response.json')
with open(out, 'w') as f:
    json.dump(doc, f, indent=1)
    f.write('\n')
print('wrote', len(vectors), 'vectors; first pdu prefix', vectors[0]['pdu_coer'][:12])
