#!/usr/bin/env python3
"""Writes the hand-made VeReMi fixture that `tests/veremi_fixture.rs` holds the exporter to.

Written from the published schemas, not copied from any VeReMi dataset (its licence has not
been checked): VeReMi 1.0 (Van der Heijden, Lukaseder, Kargl, SecureComm 2018) and the
VeReMi Extension (Kamel et al., ICC 2020). The rows are invented; only the field names,
line types, attack codes and file naming follow the papers. This script is an independent
second encoder of the same lines (Python's own `json`, sorted keys, no whitespace), so a
fixture it writes checks the Rust writer rather than echoing it.

The codes are written here as literals from the two codebooks, which is what makes the
fixture fail when two codes in the Rust tables are swapped:

  veremi-1:   ConstPos 1, ConstPosOffset 2, EventualStop 16, ConstSpeed (none) 1024
  veremi-ext: ConstPos 1, ConstPosOffset 2, EventualStop 9,  ConstSpeed (unmapped) 1024

Run from this directory: python3 make_fixture.py
"""

import hashlib
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))


def pseudonym_int(p):
    h = hashlib.sha256(p.encode()).digest()
    return int.from_bytes(h[:4], "big") & 0x7FFFFFFF


Z = [0.0, 0.0, 0.0]
PA = "1a2b3c4d5e6f7081"  # attacker 3's pseudonym
PB = "99aa88bb77cc66dd"  # sender 4's pseudonym


def kin(pos, pos_noise=Z, spd=Z, acl=Z, hed=Z):
    return {
        "pos": pos,
        "pos_noise": pos_noise,
        "spd": spd,
        "spd_noise": Z,
        "acl": acl,
        "acl_noise": Z,
        "hed": hed,
        "hed_noise": Z,
    }


def own(t, **k):
    return dict(type=2, rcvTime=t, **kin(**k))


def rx(rcv, send, pseudo, msg, rssi, **k):
    return dict(
        type=3,
        rcvTime=rcv,
        sendTime=send,
        sender=pseudonym_int(pseudo),
        senderPseudo=pseudo,
        messageID=msg,
        RSSI=rssi,
        **kin(**k),
    )


# Receiver 7, benign: two own reports and two receptions either side of the hour.
R7 = [
    own(3599.5, pos=[10.0, 20.0, 0.0], spd=[5.0, 0.0, 0.0], acl=[0.5, 0.0, 0.0], hed=[1.0, 0.0, 0.0]),
    rx(3599.9, 3599.898, PA, 101, -71.25, pos=[50.25, -12.5, 0.0], pos_noise=[1.25, 2.5, 0.0],
       spd=[0.0, 13.5, 0.0], acl=[0.0, 0.75, 0.0], hed=[0.0, 1.0, 0.0]),
    rx(3600.2, 3600.198, PB, 102, -88.5, pos=[-3.125, 40.0, 1.5], pos_noise=[0.6, 0.6, 0.0],
       spd=[-8.66, 5.0, 0.0], hed=[-0.866025, 0.5, 0.0]),
    own(3600.5, pos=[15.0, 20.0, 0.0], spd=[5.0, 0.0, 0.0], acl=[0.5, 0.0, 0.0], hed=[1.0, 0.0, 0.0]),
]
# Receiver 9, itself an EventualStop attacker.
R9 = [
    own(12.0, pos=[100.0, 0.0, 0.0], hed=[0.0, -1.0, 0.0]),
    rx(12.5, 12.499, PA, 7, -60.0, pos=[120.5, 3.0, 0.0], pos_noise=[3.0, 1.5, 0.0],
       spd=[12.0, 0.0, 0.0], hed=[1.0, 0.0, 0.0]),
]


def truth(line_type, codes):
    rows = [
        (7, 12.499, 3, PA, codes["ConstPos"], kin([120.0, 3.0, 0.0], spd=[12.0, 0.0, 0.0], hed=[1.0, 0.0, 0.0])),
        (101, 3599.898, 3, PA, codes["ConstPos"], kin([50.0, -12.5, 0.0], spd=[0.0, 13.5, 0.0], hed=[0.0, 1.0, 0.0])),
        (102, 3600.198, 4, PB, codes["ConstSpeed"], kin([-3.125, 40.0, 1.5], spd=[-4.33, 2.5, 0.0], hed=[-0.866025, 0.5, 0.0])),
        (103, 3700.0, 5, "0011223344556677", codes["ConstPosOffset"], kin([1.0, 2.0, 0.0])),
        (104, 10.0, 6, "8899aabbccddeeff", 0, kin([7.0, 7.0, 0.0])),
    ]
    out = []
    for msg, send, sender, pseudo, code, k in rows:
        out.append(dict(type=line_type, sendTime=send, sender=sender, senderPseudo=pseudo,
                        messageID=msg, attackerType=code, **k))
    return out


def dump(path, lines):
    with open(path, "w", newline="\n") as f:
        for v in lines:
            f.write(json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n")


def main():
    v1 = os.path.join(HERE, "veremi-1")
    ext = os.path.join(HERE, "veremi-ext")
    for d in (v1, ext):
        os.makedirs(d, exist_ok=True)
        for n in os.listdir(d):
            os.remove(os.path.join(d, n))

    dump(os.path.join(v1, "traceJSON-7.json"), R7)
    dump(os.path.join(v1, "traceJSON-9.json"), R9)
    dump(os.path.join(v1, "GroundTruthJSONlog.json"),
         truth(3, {"ConstPos": 1, "ConstPosOffset": 2, "ConstSpeed": 1024}))

    # Extension layout: hourly slices; receiver 9's own code (EventualStop = 9) in its name.
    dump(os.path.join(ext, "traceJSON-7-7-A0-3599-0.json"), R7[:2])
    dump(os.path.join(ext, "traceJSON-7-7-A0-3600-1.json"), R7[2:])
    dump(os.path.join(ext, "traceJSON-9-9-A9-12-0.json"), R9)
    t = truth(4, {"ConstPos": 1, "ConstPosOffset": 2, "ConstSpeed": 1024})
    dump(os.path.join(ext, "traceGroundTruthJSON-0.json"), [r for r in t if r["sendTime"] < 3600.0])
    dump(os.path.join(ext, "traceGroundTruthJSON-1.json"), [r for r in t if r["sendTime"] >= 3600.0])


if __name__ == "__main__":
    main()
