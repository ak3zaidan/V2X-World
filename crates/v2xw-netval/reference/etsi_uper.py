"""Decodes ETSI CAM, DENM and VAM UPER encodings with asn1tools, an ASN.1 compiler and
codec written independently of the Rust bindings, from the ETSI ASN.1 modules shipped in
third_party/asn1/etsi.

Input on stdin: {"type": "CAM" | "DENM" | "VAM", "asn": [module paths], "items": [hex]}.
Output: {"items": [{"reencoded_equal": bool, "stationId": int, "latitude": int,
"longitude": int} | {"error": str}]}.

Each encoding is decoded and re-encoded: UPER is canonical, so a standard-conformant
encoding re-encodes to the identical octets, and any field written with the wrong
constraint either fails to decode or re-encodes differently. asn1tools must be importable
(`pip install asn1tools`); without it the script exits non-zero and the check is skipped.
"""

import json
import sys

import asn1tools


def find(value, key):
    """The first value under `key`, depth first, in a decoded structure."""
    if isinstance(value, dict):
        if key in value:
            return value[key]
        for v in value.values():
            r = find(v, key)
            if r is not None:
                return r
    elif isinstance(value, (list, tuple)):
        for v in value:
            r = find(v, key)
            if r is not None:
                return r
    return None


def main():
    req = json.load(sys.stdin)
    spec = asn1tools.compile_files(req["asn"], "uper")
    out = []
    for h in req["items"]:
        raw = bytes.fromhex(h)
        try:
            decoded = spec.decode(req["type"], raw)
            again = spec.encode(req["type"], decoded)
            out.append({
                "reencoded_equal": again == raw,
                "stationId": find(decoded, "stationId"),
                "latitude": find(decoded, "latitude"),
                "longitude": find(decoded, "longitude"),
            })
        except Exception as e:  # noqa: BLE001 - any failure is a finding
            out.append({"error": f"{type(e).__name__}: {e}"[:200]})
    json.dump({"items": out}, sys.stdout)


if __name__ == "__main__":
    main()
