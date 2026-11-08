"""COER codec for the IEEE 1609.2 / 1609.2.1 ASN.1, built on asn1tools' OER codec.

The modules under ``tools/oracle/asn1/`` are compiled unchanged.  asn1tools cannot
resolve information-object-class *type* fields (``CLASS.&Type``), which 1609.2 uses
only for open types (extensions).  An open type is encoded in OER as a length
determinant followed by the encoding of the actual value (ITU-T X.696 clause 30),
which is exactly the OER encoding of an unconstrained OCTET STRING.  So every
open-type field is rewritten to OCTET STRING after parsing and before compiling;
callers put the separately encoded inner value in that field.  Value fields
(``CLASS.&id``) are left to asn1tools, which resolves them.

COER vs OER: asn1tools emits minimal length determinants, minimal-octet INTEGERs for
unconstrained integers, fixed-width for constrained ones, and SEQUENCE preambles with
the extension bit, which is the canonical form for the value set used here
(X.696 clause 6 canonical rules).  DEFAULT values: COER requires a root component equal
to its DEFAULT to be omitted, and asn1tools omits it.  One asn1tools defect in the
extension-addition presence bitmap is patched below (``_encode_additions``).

Evidence that this codec is canonical for real SCMS data: the published SCMS Manager production
CTL and certificate-chain files decode and re-encode byte for byte (see ``real`` in oracle.py).
"""

import functools
import glob
import os

import asn1tools

HERE = os.path.dirname(os.path.abspath(__file__))
ASN1_DIR = os.path.join(os.path.dirname(HERE), 'asn1')

EDITIONS = {
    # 1609.2-2022 + 1609.2.1-2022, the deployed US profile (owner default).
    '2022': ['ieee1609.2-2022', 'ieee1609.2.1-2022'],
}


def _is_open_type(t):
    return isinstance(t, str) and (t == 'OpenType' or ('&' in t and t.split('&')[-1][:1].isupper()))


def _fix_open_types(node):
    if isinstance(node, dict):
        if _is_open_type(node.get('type')):
            node['type'] = 'OCTET STRING'
            node.pop('table', None)
        for v in node.values():
            _fix_open_types(v)
    elif isinstance(node, list):
        for v in node:
            _fix_open_types(v)


def _encode_additions(self, data, encoder):
    """Replacement for asn1tools 0.169 oer.MembersType.encode_additions (two defects, found by
    re-encoding the published SCMS Manager production certificate chain):

    1. A missing non-OPTIONAL extension addition raised EncodeError, which ended the loop before the
       presence bits of the remaining additions were shifted in, so the presence bitmap was written
       shifted (cert 'aca.prod.s2x.io' with only `flags`: 0x20 instead of 0x80).  In X.691/X.696 every
       extension addition may be absent, so absence just clears its presence bit.
    2. The unused-bits octet was 8 instead of 0 when the number of additions is a multiple of 8.
    """
    presence = []
    encoded = []
    for addition in self.additions:
        if addition.name not in data:
            presence.append(0)
            continue
        e = oer.Encoder()
        self.encode_member(addition, data, e, encode_default=True)
        presence.append(1)
        encoded.append(e)
    if not encoded:
        return False
    n = len(self.additions)
    encoder.append_length_determinant((n + 7) // 8 + 1)
    encoder.append_non_negative_binary_integer((8 - n % 8) % 8, 8)
    for bit in presence:
        encoder.append_bit(bit)
    encoder.align()
    for e in encoded:
        encoder.append_length_determinant(e.number_of_bytes())
        encoder += e
    return True


from asn1tools.codecs import compiler as _compiler  # noqa: E402
from asn1tools.codecs import oer  # noqa: E402

oer.MembersType.encode_additions = _encode_additions

_orig_bitstring_default = _compiler.Compiler.pre_process_default_value_bit_string


def _bitstring_default(self, member, resolved_member):
    """asn1tools sizes a named-bit DEFAULT ({app}) by its highest set bit (1 bit), ignoring a fixed
    SIZE constraint; 1609.2 `EndEntityType ::= BIT STRING {app(0), enrol(1)} (SIZE(8))` then
    decodes an absent eeType as a 1-bit value that fails its own constraint.  Pad to the fixed size."""
    _orig_bitstring_default(self, member, resolved_member)
    size = resolved_member.get('size')
    if size and len(size) == 1 and isinstance(size[0], int):
        mask, nbits = member['default']
        n = size[0]
        if nbits < n:
            v = int.from_bytes(mask, 'big') >> (len(mask) * 8 - nbits) if nbits else 0
            v <<= (n - nbits)
            member['default'] = ((v << (-n % 8)).to_bytes((n + 7) // 8, 'big'), n)


_compiler.Compiler.pre_process_default_value_bit_string = _bitstring_default


def module_files(edition='2022'):
    files = []
    for d in EDITIONS[edition]:
        files += sorted(glob.glob(os.path.join(ASN1_DIR, d, '*.asn')))
    return files


@functools.lru_cache(maxsize=None)
def spec(edition='2022'):
    parsed = asn1tools.parse_files(module_files(edition), encoding='cp1252')
    _fix_open_types(parsed)
    return asn1tools.compile_dict(parsed, 'oer')


def encode(type_name, value, edition='2022'):
    return spec(edition).encode(type_name, value, check_constraints=True)


def decode(type_name, data, edition='2022'):
    return spec(edition).decode(type_name, data, check_constraints=True)


def roundtrip(type_name, data, edition='2022'):
    """Decode then re-encode; True when the bytes are canonical for this codec."""
    return encode(type_name, decode(type_name, data, edition), edition) == data
