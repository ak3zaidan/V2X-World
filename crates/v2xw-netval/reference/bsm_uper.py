"""An independent UPER decoder for the SAE J2735 BasicSafetyMessage, Part I.

Written from the ASN.1 of SAE J2735 (BSMcoreData and its element types) and the
unaligned PER rules of ITU-T X.691 (constrained whole numbers in the minimum number of
bits as an offset from the lower bound, MSB first; a fixed-size BIT STRING as its bits;
a non-extensible ENUMERATED as a constrained index; a SEQUENCE preamble of one extension
bit when the type is extensible and one presence bit per OPTIONAL component).

It shares no code with the Rust codec. Input on stdin: a JSON array of hex strings, each
a UPER-encoded BasicSafetyMessage with no Part II. Output: a JSON array of decoded field
dictionaries, or {"error": ...} for an encoding this decoder rejects.
"""

import json
import sys


class Bits:
    def __init__(self, data: bytes):
        self.data = data
        self.pos = 0

    def take(self, n: int) -> int:
        v = 0
        for _ in range(n):
            byte = self.data[self.pos // 8]
            bit = (byte >> (7 - self.pos % 8)) & 1
            v = (v << 1) | bit
            self.pos += 1
        return v

    def constrained(self, lo: int, hi: int) -> int:
        n = (hi - lo).bit_length()
        return lo + self.take(n)


def decode_bsm(data: bytes) -> dict:
    r = Bits(data)
    ext = r.take(1)
    part_ii = r.take(1)
    regional = r.take(1)
    out = {"ext": ext, "partII": part_ii, "regional": regional}
    # BSMcoreData: a SEQUENCE with no extension marker and no OPTIONAL component.
    out["msgCnt"] = r.constrained(0, 127)
    out["id"] = "%08x" % r.take(32)
    out["secMark"] = r.constrained(0, 65535)
    out["lat"] = r.constrained(-900000000, 900000001)
    out["long"] = r.constrained(-1799999999, 1800000001)
    out["elev"] = r.constrained(-4096, 61439)
    out["semiMajor"] = r.constrained(0, 255)
    out["semiMinor"] = r.constrained(0, 255)
    out["orientation"] = r.constrained(0, 65535)
    out["transmission"] = r.constrained(0, 7)  # 8 enumerators, not extensible
    out["speed"] = r.constrained(0, 8191)
    out["heading"] = r.constrained(0, 28800)
    out["angle"] = r.constrained(-126, 127)
    out["accelLong"] = r.constrained(-2000, 2001)
    out["accelLat"] = r.constrained(-2000, 2001)
    out["accelVert"] = r.constrained(-127, 127)
    out["yaw"] = r.constrained(-32767, 32767)
    out["wheelBrakes"] = r.take(5)  # BIT STRING (SIZE(5)), first bit most significant
    out["traction"] = r.constrained(0, 3)
    out["abs"] = r.constrained(0, 3)
    out["scs"] = r.constrained(0, 3)
    out["brakeBoost"] = r.constrained(0, 2)
    out["auxBrakes"] = r.constrained(0, 3)
    out["width"] = r.constrained(0, 1023)
    out["length"] = r.constrained(0, 4095)
    out["bits"] = r.pos
    # The rest of the last octet is padding and must be zero.
    pad = 0
    while r.pos % 8:
        pad |= r.take(1)
    out["padding_zero"] = pad == 0
    out["octets"] = len(data)
    return out


def main() -> None:
    items = json.load(sys.stdin)
    results = []
    for h in items:
        try:
            results.append(decode_bsm(bytes.fromhex(h)))
        except Exception as e:  # noqa: BLE001 - every failure is reported, not raised
            results.append({"error": repr(e)})
    json.dump(results, sys.stdout)


if __name__ == "__main__":
    main()
