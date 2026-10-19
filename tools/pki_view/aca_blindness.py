"""tools/pki_view/aca_blindness.py <run_dir> [--inject] (oracle venv, python -I): ACA blindness on a run's recorded PDUs, with a
positive control that can fail.

The RA must strip the enrolment identity before the ACA (1609.2.1 §4.1.3): an
EeRaCertRequestSpdu is signed by the device's enrolment certificate (SignerSingleCert, the
whole certificate, not its HashedId8), so the enrolment identity on the wire is that
certificate's bytes and its HashedId8.

* control: RA-bound requests (opened plaintext) whose signer, decoded with the oracle's
  COER, is an enrolment certificate the RA registered (HashedId8 in the RA's truth), and
  of those how many contain that certificate's bytes as a byte search finds them.
* leaks: ACA-bound PDUs (wire bytes and any opened plaintext) that contain any such
  enrolment certificate's bytes or any registered enrolment HashedId8.
* aca_signers: who signed each RaAcaCertRequestSpdu (decoded), by HashedId8 class.
--inject appends one enrolment certificate to the first ACA-bound blob: leaks must go >= 1.
Writes <run_dir>/aca-check.json (not under --inject) and prints the verdict as JSON. Used by
the SCMS effects round (docs/design/scms-real/testing/effects.md, rows aca-*).
"""
import json
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "oracle"))
from mcap.reader import make_reader  # noqa: E402
from scms_oracle import certs, coer  # noqa: E402

run = Path(sys.argv[1])
inject = "--inject" in sys.argv
enrol_ids = set()
ra_req = []   # (kind, plaintext bytes)
aca = []      # (kind, [blobs])
with open(run / "recording.mcap", "rb") as f:
    for _, ch, msg in make_reader(f).iter_messages(topics=["record/pki.flow", "record/pki.knowledge"]):
        d = json.loads(msg.data)
        if ch.topic == "record/pki.knowledge":
            for e in ((d.get("truth") or {}).get("enrolments") or []):
                if isinstance(e, dict) and e.get("enrolment"):
                    enrol_ids.add(e["enrolment"].lower())
            continue
        for p in d.get("pdus") or []:
            to = str(p.get("to", ""))
            blobs = [h for h in (p.get("hex"), (p.get("opened") or {}).get("hex")) if h]
            if "/aca/" in to:
                aca.append((p.get("type"), [b.lower() for b in blobs]))
            elif "/ra/" in to and str(p.get("from", "")).startswith("ee/"):
                o = (p.get("opened") or {}).get("hex")
                if o:
                    ra_req.append((p.get("type"), bytes.fromhex(o)))


def signer_of(data):
    """The SignerSingleCert certificate of a signed certificate request, or None."""
    try:
        d = coer.decode("Ieee1609Dot2Data", data)
        if d["content"][0] != "signedCertificateRequest":
            return None
        scr = coer.decode("SignedCertificateRequest", d["content"][1])
        if scr["signer"][0] != "certificate":
            return None
        return scr["signer"][1][0]
    except Exception:
        return None


enrol_certs = set()
truth_ids = set(enrol_ids)
control = control_found = undecoded = truth_confirmed = 0
for kind, b in ra_req:
    c = signer_of(b)
    if c is None:
        undecoded += 1
        continue
    # Every EE-signed request to the RA is signed by the EE's enrolment certificate
    # (1609.2.1 EeRaCertRequestSpdu; check_pdus asserts it): all are needles, and the RA's
    # truth rows (capped in the record) confirm the ones they list.
    ch = certs.encode(c).hex()
    enrol_certs.add(ch)
    enrol_ids.add(certs.hashed_id8(c).hex())
    control_found += ch in b.hex()
    control += 1
    truth_confirmed += certs.hashed_id8(c).hex() in truth_ids
if inject and aca and enrol_certs:
    k, bl = aca[0]
    aca[0] = (k, [bl[0] + next(iter(enrol_certs))] + bl[1:])
needles = list(enrol_certs) + sorted(enrol_ids)
leaks = sum(1 for _, bl in aca if any(n in b for b in bl for n in needles))
signers = Counter()
for kind, bl in aca:
    if kind != "RaAcaCertRequestSpdu":
        continue
    c = signer_of(bytes.fromhex(bl[0]))
    signers["none" if c is None else ("enrolment" if certs.hashed_id8(c).hex() in enrol_ids else "other")] += 1
out = {"enrolment_ids": len(enrol_ids), "ra_requests": len(ra_req), "ra_undecoded": undecoded,
       "control": control, "control_found_by_search": control_found, "truth_confirmed": truth_confirmed, "truth_ids": len(truth_ids),
       "enrolment_certs": len(enrol_certs), "aca_pdus": len(aca), "leaks": leaks,
       "aca_signers": dict(signers), "injected": inject}
if not inject:
    (run / "aca-check.json").write_text(json.dumps(out))
print(json.dumps(out))
