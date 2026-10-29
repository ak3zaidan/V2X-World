#!/usr/bin/env python3
"""The asn1tools side of the IEEE 1609.3 WSA oracle (WP-19 P19.1).

Compiles the cached modules (``third_party/asn1/ieee1609.3``: ISO 17419
``CITSapplMgmtIDs``, ``wee.asn``, ``wsa.asn``) with asn1tools' UPER codec, which the
modules name as their encoding rule, and either

* ``emit``: encodes each vector below and writes ``name hex`` lines (the fixture
  ``crates/v2xw-msg/tests/fixtures/wsa_asn1tools.txt``, which ``tests/wsa_oracle.rs``
  holds the Rust codec to, byte for byte and value for value); or
* ``check FILE``: decodes every ``name hex`` line of FILE (for example octets the Rust
  codec wrote) and re-encodes it, failing unless both succeed and agree.

Each vector is built here independently of the Rust side; the two must describe the
same value (see ``tests/wsa_oracle.rs``). Open types (each ``Extension.value``) are
asn1tools ``bytes``: the inner value is encoded on its own first, as X.691 clause 11.2
says.

Usage: ``python3 -I wsa_oracle.py <repo>/third_party/asn1/ieee1609.3 emit|check [FILE]``.
"""

import os
import sys

import asn1tools


def spec(asn_dir):
    files = ['17419_CITSapplMgmtIDs_v1.asn', 'wee.asn', 'wsa.asn']
    return asn1tools.compile_files([os.path.join(asn_dir, f) for f in files], 'uper')


def psid(v):
    if v <= 127:
        return ('content', v)
    if v <= 16511:
        return ('extension', ('content', v))
    if v <= 2113663:
        return ('extension', ('extension', ('content', v)))
    return ('extension', ('extension', ('extension', v)))


def ext(s, ref, type_name, value):
    return {'extensionId': ref, 'value': s.encode(type_name, value)}


def loc(lat, lon):
    return {'latitude': {'fill': (b'\x00', 1), 'lat': lat}, 'longitude': lon}


def doc_addr(last, mid=1):
    return bytes([0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, mid] + [0] * 7 + [last])


def scms(s, lat, lon, elevation, channel, power):
    return {
        'version': {'messageID': 0, 'rsvAdvPrtVersion': 3},
        'body': {
            'changeCount': {'saID': 0, 'contentCount': 0},
            'extensions': [
                ext(s, 17, 'RepeatRate', 50),
                ext(s, 6, 'ThreeDLocation', dict(loc(lat, lon), elevation=elevation)),
            ],
            'serviceInfos': [
                {'serviceID': psid(0x23), 'channelIndex': 1, 'chOptions': {}},
                {'serviceID': psid(0x1020407E), 'channelIndex': 1, 'chOptions': {}},
            ],
            'channelInfos': [{
                'operatingClass': 17, 'channelNumber': channel, 'powerLevel': power,
                'dataRate': {'adaptable': (b'\x00', 1), 'dataRate': 12},
                'extensions': {},
            }],
            'routingAdvertisement': {
                'lifetime': 1800, 'ipPrefix': doc_addr(0), 'ipPrefixLength': 64,
                'defaultGateway': doc_addr(1), 'primaryDns': doc_addr(0x53, mid=0),
                'extensions': [],
            },
        },
    }


def edca(aci, aifsn, ecw_max, ecw_min):
    return {'res': 0, 'aci': aci, 'acm': 0, 'aifsn': aifsn, 'ecwMax': ecw_max,
            'ecwMin': ecw_min, 'txopLimit': 0}


def full(s):
    v = scms(s, 407527000, -739868000, 120, 174, 20)
    b = v['body']
    b['changeCount'] = {'saID': 3, 'contentCount': 9}
    b['extensions'].append(ext(s, 7, 'AdvertiserIdentifier', 'RSU-42'))
    b['serviceInfos'][0]['chOptions'] = {'extensions': [
        ext(s, 8, 'ProviderServiceContext', {'fillBit': (b'\x00', 3), 'psc': b'scms'}),
        ext(s, 9, 'IPv6Address', bytes([0x20] * 16)),
        ext(s, 10, 'ServicePort', 443),
        ext(s, 11, 'ProviderMacAddress', bytes([1, 2, 3, 4, 5, 6])),
        ext(s, 19, 'RcpiThreshold', 110),
        ext(s, 20, 'WsaCountThreshold', 2),
        ext(s, 22, 'WsaCountThresholdInterval', 10),
    ]}
    b['channelInfos'][0]['extensions'] = {'extensions': [
        ext(s, 12, 'EdcaParameterSet', {
            'acbeRecord': edca(0, 6, 10, 4), 'acbkRecord': edca(1, 9, 10, 4),
            'acviRecord': edca(2, 3, 4, 3), 'acvoRecord': edca(3, 2, 3, 2)}),
        ext(s, 21, 'ChannelAccess80211', 1),
    ]}
    b['routingAdvertisement']['extensions'] = [
        ext(s, 13, 'SecondaryDns', bytes([0x20] * 16)),
        ext(s, 14, 'GatewayMacAddress', bytes([6, 5, 4, 3, 2, 1])),
        {'extensionId': 99, 'value': bytes([0xab, 0xcd])},
    ]
    return v


def psids(s):
    return {
        'version': {'messageID': 0, 'rsvAdvPrtVersion': 3},
        'body': {
            'changeCount': {'saID': 15, 'contentCount': 15},
            'serviceInfos': [
                {'serviceID': psid(p), 'channelIndex': i, 'chOptions': {}}
                for i, p in enumerate([0x20, 0x7F, 0x80, 0x4080, 0x8002, 0x1020407E])
            ],
        },
    }


def edges(s):
    return {
        'version': {'messageID': 0, 'rsvAdvPrtVersion': 3},
        'body': {
            'changeCount': {'saID': 1, 'contentCount': 2},
            'extensions': [ext(s, 5, 'TwoDLocation', loc(-900000000, 1800000001))],
            'channelInfos': [{
                'operatingClass': 255, 'channelNumber': 184, 'powerLevel': -128,
                'dataRate': {'adaptable': (b'\x80', 1), 'dataRate': 127},
                'extensions': {},
            }],
        },
    }


def vectors(s):
    minimal = {'version': {'messageID': 0, 'rsvAdvPrtVersion': 3},
               'body': {'changeCount': {'saID': 0, 'contentCount': 0}}}
    return [
        ('minimal', minimal),
        ('scms-service', scms(s, 407527000, -739868000, 120, 174, 20)),
        ('full', full(s)),
        ('psids', psids(s)),
        ('edges', edges(s)),
    ]


def main():
    s = spec(sys.argv[1])
    if sys.argv[2] == 'emit':
        for name, value in vectors(s):
            data = s.encode('SrvAdvMsg', value)
            assert s.encode('SrvAdvMsg', s.decode('SrvAdvMsg', data)) == data, name
            print(name, data.hex())
        return 0
    bad = 0
    with open(sys.argv[3]) as f:
        for line in f:
            if not line.strip() or line.startswith('#'):
                continue
            name, hexs = line.split()
            data = bytes.fromhex(hexs)
            try:
                again = s.encode('SrvAdvMsg', s.decode('SrvAdvMsg', data))
                ok = again == data
            except Exception as e:  # noqa: BLE001 - any failure is a red row
                ok, again = False, str(e).encode()
            print(('ok  ' if ok else 'RED ') + name)
            bad += not ok
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
