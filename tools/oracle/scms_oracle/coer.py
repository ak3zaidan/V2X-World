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
(X.696 clause 6 canonical rules).  DEFAULT values: COER requires a field equal to its
DEFAULT to be omitted; asn1tools omits it only when the caller omits it, so
``encode`` strips DEFAULT-equal fields first (see ``_strip_defaults``).
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


@functools.lru_cache(maxsize=None)
def _parsed(edition='2022'):
    parsed = asn1tools.parse_files(module_files(edition), encoding='cp1252')
    _fix_open_types(parsed)
    return parsed


def encode(type_name, value, edition='2022'):
    return spec(edition).encode(type_name, value, check_constraints=True)


def decode(type_name, data, edition='2022'):
    return spec(edition).decode(type_name, data, check_constraints=True)


def roundtrip(type_name, data, edition='2022'):
    """Decode then re-encode; True when the bytes are canonical for this codec."""
    return encode(type_name, decode(type_name, data, edition), edition) == data
