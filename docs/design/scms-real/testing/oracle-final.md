# Final independent oracle check of the SCMS records (oracle-final, 2026-10-22)

Part 3 of "test everything": rules that had no independent check are now checked by the
Python oracle against the engine's own recorded PDUs, on two real runs of the release binary.

> **HEAD verdicts (88e24b09, re-checked 2026-10-22): see §8.** Sections 1–7 describe the first
> check, on main `4924a5d2`. On HEAD every rule passes on both runs, OF-1, OF-2 and O-1 are
> fixed, and the reproduction tests and the F1 capture have run. One new disagreement (OF-3)
> and two oracle defects (fixed) are in §8.

**Independence.** The agent did not read the engine's Rust. Every rule was written from one
of two sources: the 1609.2-2022 and 1609.2.1-2022 text, which is the comments of the ASN.1
modules committed under `tools/oracle/asn1`, or the cited profiles in the research notes
02/03. The agent read four other things:

* the public outputs: the recordings, `run-report.json` and the scenario files;
* the record layout used by `tools/realworld/runtime_realism.py` and `tools/conformance/check_run.py` (Python);
* the public test harness calls in `tests/p2pcd_air.rs` (`Scenario::load`, `Engine::build`, `MemoryRecorder`), to write the reproduction test.

## 1. What runs and how

| Run | Scenario | What differs |
|---|---|---|
| conf (base) | `scenarios/scms-conformance.yaml` | The two-provider US deployment on the real calendar: manual MA review (8 h), 10 % constant-position liars, radio 0–660 s, 196,200–196,500 s and 432,900–433,200 s |
| mbr (variant) | `scenarios/scms-oracle-mbr.yaml` | Same seed and deployment. Automatic MA at both providers, 20 % liars, radio windows at 0, 90,000, 260,000 and 1,300,000 s, 15 days in total. Both i-period rollovers (615→616, 616→617) fall inside the run |

Both ran on the release binary of main `4924a5d2` with `--record-level events`, one run per
`heavy.sh` call under the run lock. The check is:

```
<venv>/bin/python -I tools/oracle/oracle.py records RUN_DIR... \
    --expect crates/v2xw-engine/tests/expectations/scms/scms-oracle-final.yaml [--mutate]
```

The new modules are `tools/oracle/scms_oracle/records.py` and `ctlfiles.py`.

`records` decodes every PDU captured in `pki.flow`. That is 16,107 PDUs on conf and 38,315 on
mbr, from EeEcaCertRequest to SecuredCrl. It verifies every signature with the A0 oracle's
own ECDSA, using ECQV-1609 for implicit signers. It then checks the rules in §2 against those
PDUs, the `pki.state` CRL and CTL rows, `node.credentials` and the revocation stages.

`ctl-files` checks an F1 capture's `gov.json`. A capture needs `tests/conformance.rs`, not the
CLI; see §6.

Before reading any result, every row of the expectations file was written as the standard
predicts it. Section 4 lists the rows that changed afterwards, each with its evidence.

## 2. Results: rule by rule

| Rule | What it checks (source) | conf | mbr | Verdict |
|---|---|---|---|---|
| DECODE | Every PDU re-encodes byte for byte (canonical COER) | 16,107 / 16,107 | 38,315 / 38,315 | pass |
| TIME | `pki.state` time32 = Time32(t0 + t), TAI with 5 leap seconds | 9,297 | 26,267 | pass |
| CERT-ID | Recorded certificate keys are the oracle's HashedId8 | 2,556 | 4,717 | pass |
| CRACA-CERT | cracaId = HashedId3 of a CRACA whose CRL signer lists the series. Series 1 pseudonym, 3 application, 4 enrolment ([SMP] Table 1) | 5,188 | 9,592 | pass |
| CRL-ENV | Signed CrlContents: psid 256, headerInfo psid only, version 1, signature | 10 | 70 | pass |
| CRL-SSP | crlSeries is in CrlSsp.crls. crlCraca is the signer (isCraca) or its issuer (issuerIsCraca). This holds for both the ISS form (root as CRACA) and the SAESOL form (ICA) | 6 | 42 | pass |
| CRL-NEXT / CRL-TIME | nextCrl is strictly after issueDate. issueDate is not after the first send | 2 / 2 | 14 / 14 | pass |
| CRL-SEQ | Per (crlCraca, crlSeries): issueDate never decreases, iRev never decreases, indexWithinI is 0 on a new iRev and +1 per full or delta CRL | — (one CRL each) | 36 | pass |
| CRL-IREV | iRev ≤ the i-period at issueDate | 2 | 14 | pass |
| CRL-KEEP | A later full CRL keeps every unexpired entry. Seeds are advanced to the later iRev and compared ([SM-PROV] 3.4.9) | — | 6 | pass |
| CRL-SLA | The next CRL comes by nextCrl ([SM-PROV] 3.4.10) | — | 6 | pass |
| CRL-STATE | `pki.state` CRL rows match the decoded bytes | 6 | 42 | pass |
| CRL-MATCH | Each linkage entry, followed through every full CRL, revokes exactly one recorded device, and that device is one the MA decided on | 9 | 36 | pass |
| CRL-PUB | A decided device is on a CRL by the end of the next business day (ET), and on none before the decision | 6 (3 judged) | 24 (12 judged) | pass |
| RA-BLOCK | No batch reaches a device after its RA blacklisted it (SCMS-1204/509) | 4 | 12 | pass. 2 requests after blacklisting were refused on mbr (`provisioning-refused`) |
| ENR-MAP | EcaEeCertResponse.requestHash names the device's request. ECA signature | 232 | 396 | pass |
| DL-ACK | RaEeCertAck: Signed psid 35 by an RA, requestHash = HashedId8(request as sent), firstI for butterfly | 490 | 846 | pass |
| DL-ENC | Download-Req = Encrypted{Signed}: one certRecipInfo to an RA, psid 35, SignerSingleCert | 10,066 | 38,143 | pass |
| DL-SIGN | Signed by the device's own enrolment certificate. Signature verifies | **1,320 red** / 2,876 | **5,245 red** / 10,898 | **OF-1** |
| DL-TIME | generationTime within 5 s of the send ([CAMP122] 5.1.7) | 1,438 | 5,449 | pass |
| DL-NAME | `<requestHash HEX>_<i HEX>.zip` with i ≥ firstI, `<requestHash>.info`, `misbehavior-report` | 1,704 | 5,957 | pass |
| DL-AFTER | Not before the ack's nextDlTime | 26 | 80 | pass |
| DL-WAIT | Same file not re-requested within 300 s. At most 100 requests ([SMP] Table 3) | 142 | 278 | pass |
| ENR-LIFE | Enrolment certificate valid 6 years ([CAMP122] Table 3) | **56 red** / 114 | **97 red** / 196 | **OF-1** |
| MBR-ENC | Encrypted to one MA certificate whose MaSsp lists 0x20 | 2,592 | 10,342 | pass |
| MBR-SIG | psid 0x26, headerInfo {psid, generationTime}, full pseudonym signer permitting 0x26, inside its validity and not after the upload, signature | 10,368 | 41,368 | pass |
| MBR-UP | Each cellular upload carries the report and a Download-Req. The RA forwards each report once, byte for byte | **1,695 red** / 5,593 | **49 red** / 18,667 | **OF-2** |
| CTL-SUM / CTL-DEV | CTL sequence +1, ≥ quorum signers, in force from effectiveDate. No device runs ahead of or back from the published CTL | 5 / 210 | 5 / 357 | pass |

Totals: on conf, 65,980 of 69,051 checks pass. On mbr, 206,798 of 212,189 pass. Every red
check is one of the two findings below. Both runs match their expectations file rows (23/23
and 24/24).

Independent metric checks:

* **Honest revocations: 0.** Every device the oracle finds on a CRL is an attacker in the
  ground truth (`gt.attack.action` actor, linked to the device through `gt.subject`). That is
  3 of 3 on conf and 12 of 12 on mbr. This agrees with the engine's `revoked_honest = 0`.
* **Decisions all reached the CRLs.** The engine's MA revoke decisions (4 and 12) all reached
  the CRLs. The exception is conf's last decision, taken Friday 19:20 ET. Its deadline is
  Monday, after the run ends.

## 3. Disagreements with the engine (evidence, reproductions)

**OF-1: each vehicle has two enrolment certificates.**

A device has one enrolment certificate. The ECA issues it (1609.2.1 6.3.4.2) and the DCM
registers it with the RA. It signs every Download-Req ([CAMP122] 6.3, [SM-MBR] 4).

On both runs, each of the 56 vehicles holds two:

1. **(a)** The certificate its recorded ECA exchange issued: EeEcaCertRequest → EcaEeCertResponse, 6 years, starting at t0. It signs only the pre-drive Download-Reqs, 112 on conf. It is not in its RA's enrolment registry (`pki.knowledge`): 2 of 58 ECA-exchange certificates are there, and those 2 are the RSUs.
2. **(b)** A second certificate that the ECA signed but no recorded exchange issued. Its validity also starts at t0, but lasts 0.9–5.5 years (for example 19,914 h), not 6 years. The RA registered this one, `node.credentials` reports it, and it signs every later Download-Req: top-ups, `.info` and all 1,296 report uploads.

Both certificates carry the same name (`ee-000f4244.provider-a` for `ee/1000004`), and both
signatures verify. Two things follow. First, the RA accepts pre-drive requests signed by a
certificate it never registered. Second, "device age" is modelled as a shortened lifetime,
not as a back-dated validity start; the real bits would be a back-dated start with a 6-year
duration. Any defence that keys on the enrolment certificate is affected, such as RA
blacklisting or linking a report to a device.

Reproduction: `tests/oracle_final.rs::one_enrolment_certificate_is_issued_registered_and_signs_every_download_request`
(ignored until fixed). It runs `scenarios/scms-oracle-repro.yaml`, which is the first 640 s
of conf. A Python dry run of the same logic on conf's recording gives "56 devices' enrolment
certificate is not the one their ECA exchange issued; 112 Download-Reqs not signed by it".

**OF-2: most report uploads in one window carry no bytes.**

[SM-MBR] 3–4: each upload is a SecuredMisbehaviorReport POSTed with a `misbehavior-report`
Download-Req. On conf, 1,695 of 2,991 cellular uploads (57 %) are 1,119-byte `report`
messages with no PDU. They still go through the RA's shuffle to the MA (`report_received`,
`ma.report`). The oracle cannot check them, and neither can anything else, because no bytes
exist.

1,536 of the 1,695 fall in the window after the CRL that revoked the liars (196,200–196,500
s), while those liars are still driving. In mbr's windows before any CRL, only 13 of 2,599 do.

Likely cause, inferred from outputs only: the engine still uploads a sized-only report about a
sender it has started refusing. Under [SM-MBR], with verified evidence only, that report would
not exist at all. Reproduction: `tests/oracle_final.rs::every_cellular_report_upload_carries_its_spdus`.

Both reproductions are compiled on this branch. On HEAD both run and pass, and OF-1 and OF-2
are fixed (§8.3).

**Observations (not disagreements)**

* **O-1: delta CRLs are never sent.** (Fixed on HEAD: full CRLs only, §8.2.) On mbr, each full CRL from the second onwards is issued
  together with a delta CRL at the same issueDate: 615/1 full with 615/2 delta, 615/3 with
  615/4, 616/0 with 616/1. The 6 deltas are signed and recorded in `pki.state`, but never sent
  on any link. indexWithinI counts them, which is correct per the CrlBaseTypes text, so a
  device that sees only fulls finds gaps (1 → 3). 1609.2 allows this. The already-open
  "device delta-CRL use" item covers the deltas.
* **O-2: what "within 1 business day" means for a weekend decision.** Four automatic decisions
  were taken on Saturday 05:03 ET and published Monday 10:00 ET. Read as "end of the next
  business day", the rule passes. Read strictly as "the same wall time one business day
  later" (Monday 05:03), it fails. The rule uses the first reading; [SM-PROV] 3.4.10 does not
  say which applies (UNCONFIRMED).

## 4. Rules corrected after the first results (wrong expectation, with evidence)

| Rule | First form | Evidence | Now |
|---|---|---|---|
| DL-ACK | firstI always present | The RaEeCertAck text says firstI "shall be present … if the corresponding EeRaCertRequest uses the butterfly key mechanism". The 6 failing acks were for the RSUs' non-butterfly application-certificate requests | Required for butterfly requests only |
| CRL-SEQ | issueDate strictly increases, over sent CRLs only | A full and a delta CRL issued at the same instant share issueDate. The never-sent deltas carry indexWithinI values | Non-decreasing, over every issued CRL (`pki.state` hex included) |
| CRL-MATCH | Newest CRL only | A device recorded only before the newest iRev cannot match its advanced seeds, so 1 of 12 entries looked unmatched | Entries are followed through every full CRL |
| CRL-PUB | Same wall time one business day later | O-2 | End of the next business day |
| DECODE | Every PDU as Ieee1609Dot2Data | EncryptedIndividualPLV is its own type (version 2) | Decoded as its type |

The oracle's own fault catalogue had two faults that missed their rule: DL-AFTER mutated an
unused acknowledgement, and CRL-PUB only applied to decisions after day 5. Both were fixed and
are now red.

## 5. Base vs variant (same seed): expected and measured

| Measure | Expected (written before) | conf | mbr | Verdict |
|---|---|---|---|---|
| Distinct CRLs per provider | conf 1, mbr ≥ 3 | 1 full (615/0) | 4 full and 3 delta (615/0, 615/1, 615/2Δ, 615/3, 615/4Δ, 616/0, 616/1Δ) | pass |
| iRev / indexWithinI | 0, 1, … within an iRev, back to 0 on a new i | — | 615: 0–4, 616: 0–1. No CRL after 1,210,200 s, so 617 never appears (the next reissue is due by nextCrl, after the run) | pass |
| Revoked devices | mbr about 2× conf, honest 0 | 4 decided (a 1, b 3), 3 on CRLs | 12 (a 5, b 7), 12 on CRLs. 0 honest | pass |
| MA decision delay | mbr ≪ conf's 8 h | 28,800 s | 0.001–0.24 s | pass |
| Report uploads (cellular) | about 3× (1.7× radio time, 2× liars) | 2,991 | 5,220 (1.75×) | **expectation too high**. Revoked liars are refused on the air from day 1, so later windows detect fewer of them. No engine fault |
| Cost | wall time 1.7–2.5×, memory < 2×, records 2–3.5× | 310 s CPU, 97.6 MB, 794,838 records | 694 s CPU (2.2×), 107.2 MB, 1,455,350 records (1.83×) | pass. Records are just under the band, for the same reason as the report uploads |
| Message types and entity pairs | the same set | 38 types, 24 pairs | 39 types (adds `provisioning-refused`), 24 pairs | pass. The new type is the RA refusing blacklisted devices (RA-BLOCK), which only mbr exercises |

Wall times are not comparable between the runs: `time` included the wait for the run lock,
which other agents held (mbr: 2,279 s real, 694 s CPU).

## 6. Injected faults (every rule must be able to go red)

`--mutate` injects 35 record faults, one at a time, into the loaded records. A fault counts
as red only if its target rule has more failures than on the unmutated records.

* **conf:** 32 red, 0 silent, 3 not applicable. The 3 are crl-index, crl-backdated and crl-drop-entry, which need two CRLs of one series.
* **mbr:** 35 red, 0 silent.

The faults by rule:

* **TIME:** time32 off by 5 s.
* **CERT-ID:** wrong HashedId8.
* **CRL-ENV:** CRL signature bit.
* **CRL-SSP:** crlCraca, crlSeries 77.
* **CRL-NEXT:** nextCrl = issueDate.
* **CRL-IREV:** iRev in the future.
* **CRL-SEQ:** indexWithinI jump, backdated CRL.
* **CRL-KEEP:** dropped entry.
* **CRL-SLA:** run past nextCrl.
* **CRL-STATE:** CRL row.
* **CRL-PUB:** decision 5 days earlier.
* **CRACA-CERT:** pseudonym cracaId, crlSeries 4.
* **DL-ACK:** requestHash.
* **DL-SIGN:** signature bit, another device's request.
* **DL-TIME:** +10 s.
* **DL-NAME:** lower-case hex, wrong request hash.
* **DL-AFTER:** nextDlTime +30 d.
* **DL-WAIT:** re-poll after 60 s.
* **ENR-LIFE:** 7-year enrolment.
* **RA-BLOCK:** a batch to a revoked device.
* **DECODE:** trailing byte.
* **MBR-ENC:** recipient.
* **MBR-SIG:** psid 0x20, generationTime after expiry, signature bit.
* **MBR-UP:** a forwarded report altered.
* **CTL-SUM:** sequence gap, one signature, in force early.
* **CTL-DEV:** a device ahead.

**CTL files.** `ctl-files` was checked against the six CTLs SCMS Manager publishes. They sit
in the ignored cache `third_party/scms-artefacts/files`; `gov.json` is built by reading their
sequence numbers. The six form two chains: production 0 → 1, and test 0 → 1 → 2 → 3.

* PASS: all 30 elector signatures are valid.
* Each signer is approved by the previous CTL, or by the CTL itself for a genesis CTL.
* The sequence goes up by 1 each time, and effectiveDate never decreases.
* Each ToBeSignedCtlSignature names its CTL, and tbsCtlHash = SHA-384(tbsCtl).

The published genesis CTLs (sequence 0) carry no `quorum`. For the CTL after a genesis, the
checker therefore requires at least one valid approved signature and reports the count (5 of
5). The text says an absent quorum carries over, and a genesis has none to carry.

`--mutate` on those files turns all 5 faults red: one signature, the same elector five times,
a signature bit, an edited root list, a skipped sequence.

The engine's own CTL bytes are only in the F1 capture (`tests/conformance.rs` at HEAD,
`gov.json`). That capture was queued behind other agents' runs and had not got the run lock
when this report was written. The same applies to the run of the two reproduction tests,
which are built.

## 7. What was fixed, and what is open

**Fixed (oracle side):**

* the rules in §4;
* two faults that missed their rule;
* `oracle.py records`, `oracle.py ctl-files` and the expectations file `crates/v2xw-engine/tests/expectations/scms/scms-oracle-final.yaml`.

**Not fixed in the engine**, by design: this package does not read the engine. OF-1 and OF-2
are engine defects with reproductions (both fixed by the fix wave 2 merges; verified on HEAD in §8):

* **OF-1:** issue the device's own ECA-exchange certificate, register it, and use it for every Download-Req, with a back-dated start for older devices.
* **OF-2:** produce no upload for a report that has no SPDU, or encode it.

**Open:**

* ~~The rules hold on 4924a5d2, not yet on HEAD.~~ Done: `records` and `ctl-files` on HEAD, §8.
* `.info` contents cannot be checked: `info-response` is a 4-byte message with no PDU.
* A CTL's bytes are not in the CLI recording, only its summary, so elector verification needs the F1 capture (run on HEAD, §8.4).
* The RSU application-certificate request is not an EeRaCertRequestSpdu on the capture, but its ack and filename are checked.
* The download decryption is not re-done (the CLI records no recipient keys); DL-SIGN proves the opened bytes are the device's own signed request.
* Federal holidays are not modelled in CRL-PUB (none falls in these runs).

## 8. HEAD verdicts (88e24b09, 2026-10-22)

The same independent check, repeated on HEAD after fix wave 2 (TRUST, REPORTS, DETECT). The
agent again did not read the engine's Rust. It read only the recordings, the F1 capture, the
Python checkers and the two test files it runs.

**Build and runs.** One optimised debug build of HEAD (`CARGO_PROFILE_DEV_OPT_LEVEL=2`): the
CLI plus the `oracle_final` and `conformance` test binaries, copied out under the build lock.
Each run was one `heavy.sh` call under the run lock, with `--record-level events`, on the
same seed as §1. The HEAD expectation rows were committed (`43a98a81`) before any HEAD result
was read: 7 conf CRL rows, which now apply because every CRACA publishes at t0; the 6 mbr
rows the first check lacked (ENR-MAP, ENR-LIFE, DL-SIGN, MBR-UP, CTL-SUM, CTL-DEV); and the
HEAD-vs-4924a5d2 compare rows.

### 8.1 Rule verdicts on HEAD

| Check | conf | mbr | Verdict |
|---|---|---|---|
| `oracle.py records` | PASS, 67,451 / 67,451 checks, 16,131 PDUs | PASS, 211,785 / 211,785 checks, 38,315 PDUs | pass |
| Expectation rows | 30 / 30 | 30 / 30 | pass |
| DL-SIGN (was OF-1) | 2,892 ok (was 1,320 red) | 10,898 ok (was 5,245 red) | **fixed** |
| ENR-LIFE (was OF-1) | 58 ok: one certificate per device (was 56 red of 114) | 99 ok (was 97 red of 196) | **fixed** |
| MBR-UP (was OF-2) | 3,930 ok (was 1,695 red) | 18,618 ok (was 49 red) | **fixed** |
| CRL-SEQ / KEEP / SLA on conf | 6 / 2 / 2 ok (no data before) | 24 / 8 / 8 ok | pass |
| Faults (`--mutate`) | 34 red, 0 silent, 1 n/a (crl-drop-entry: the earlier CRL is the empty t0 CRL) | 35 red, 0 silent | pass, after oracle fix 2 (§8.5) |
| `oracle.py selftest` | 412 / 412 vectors | | pass |

### 8.2 Base vs variant and HEAD vs 4924a5d2 (expected before, measured after)

| Change | Expected (written before) | Measured | Verdict |
|---|---|---|---|
| CRLs on conf | An empty full CRL from each CRACA at t0 (R-NOCRL), then one revocation CRL per series. No deltas | Per provider, `F614/0` (0 entries, t0) then `F615/0` (a: 1, b: 2 entries, 191,400 s). No delta | pass |
| CRLs on mbr | Full only. t0 CRL plus one per decision day. indexWithinI with no gap. iRev 615, then 616 | Per provider: 614/0 at t0, then 615/0 at 105,000 s, 615/1 at 191,400 s, 615/2 at 537,000 s, 616/0 at 1,141,800 s. Was 615/1, 615/3 with deltas at 2 and 4. Every nextCrl is +7 d, and 537,000 → 1,141,800 s is exactly 7 d | pass. O-1 fixed |
| Report uploads | No upload without bytes. Uploads with bytes about 1,296 | conf: 1,304 with bytes, 0 without (was 1,296 + 1,695). mbr: 5,171 with bytes, 0 without (was 5,171 + 49) | pass |
| Revocations | conf 4 decided (a 1, b 3), 3 on CRLs. mbr about 12. Honest 0 | conf 4 / 3, mbr 12 / 12. Every decided subject maps (`gt.subject`) to an actor with `gt.attack.action` records: 4 / 4 and 12 / 12. Engine `revoked_honest` 0 | pass. Unchanged from 4924a5d2 |
| Enrolment | One certificate per device: the ECA exchange's own, RA-registered, 6 y. It signs every Download-Req | Dry run of the `oracle_final` logic: 0 devices with a foreign certificate, 0 of 1,440 Download-Reqs signed by another (4924a5d2: 56 devices, 112) | pass |
| F1 capture (conformance) | Fewer EeRaCertRequests, since the second enrolment is gone. Revoked 1000018, 1000027, 1000028 | EeRaCertRequest and RaEeCertAck 62 each (06 §6: 118). Download-Req 1,446 (1,591). Same three revoked. 4 SecuredCrl, 4 CRL heads (the t0 CRLs) | pass |
| Message types / entity pairs | The same set as 4924a5d2 | conf 38 / 24, mbr 39 / 24, identical | pass |
| MA decision delay | conf 8 h manual, mbr automatic | conf 28,800 s. mbr 0.001–0.236 s | pass |
| Cost | Records fall by the empty uploads and their hops. CPU 1–2× the release run (optimised debug build) | conf: 212.9 s CPU (release 4924a5d2: 310.3), 84.9 MB (97.6), 771,012 records (794,838, −3 %), 1,327,951 events. mbr: 447.9 s (693.7), 111.2 MB (107.2), 1,454,724 records (1,455,350) | **CPU expectation wrong**: HEAD at opt-level 2 is 0.65–0.69× the old release binary. Evidence: the same scenario hash and the same counts on lower CPU. This matches the earlier speed fix ("one check redid real cryptography thousands of times"). Records: pass |
| mbr / conf on HEAD | wall 1.7–2.5×, memory < 2×, records 2–3.5× | CPU 2.10×, memory 1.31×, records 1.89× | pass (records just under the band, as in §5) |

### 8.3 Reproduction and capture tests, run on HEAD

| Test | Result |
|---|---|
| `oracle_final.rs::one_enrolment_certificate_is_issued_registered_and_signs_every_download_request` | ok |
| `oracle_final.rs::every_cellular_report_upload_carries_its_spdus` | ok. Both together: 66 s, peak 832 MB (MemoryRecorder) |
| `conformance.rs` (both tests, `V2XW_CONFORMANCE_OUT` capture) | 2 / 2 ok, 203 s, 150 MB |
| `check_run.py` on that capture | `conformance: PASS (18910/18910 checks; 0 disagreements)` |
| `check_run.py --mutate --sample 1` | 18 / 18 faults red |

Can these tests go red? Both reproductions ran their logic in Python (`dryrun`) on the
4924a5d2 recording and came out red: 56 devices, and 1,695 uploads with no bytes. On HEAD the
same logic finds 0 and 0. The engine was not modified to inject the fault, because this
package does not read or change the engine.

### 8.4 CTL files from the engine (F1 `gov.json`)

`ctl-files` on HEAD's capture gives PASS: CTL-FILE 16, CTL-ORDER 4, CTL-QUORUM 6, CTL-SIG 140.
Each provider holds CTL 0 and CTL 1, and holds CTL 1 twice (history and current). Every CTL
carries 5 of 5 valid approved elector signatures, quorum 3. The engine's CTL bytes are now
verified independently. Faults: 6 / 6 red after oracle fix 1. On the published CTLs: 5 / 5 red,
and `ctl-copy-differs` is n/a there (one copy each).

### 8.5 Oracle defects found and fixed (`1637acb3`)

1. **The ctl-files faults could not fail on the engine's capture.** On HEAD's `gov.json`, all 5
   faults were SILENT. `check()` kept one copy per sequence number, the last. The faults
   changed `max(...)`, the first copy. So the changed copy was never checked. This also hid a
   real gap: two different encodings under one sequence number would pass. Fix: every
   distinct copy is checked; copies of one sequence must agree (CTL-FILE, new fault
   `ctl-copy-differs`); and the faults alter every copy.
2. **CRL sequence faults were n/a on conf.** `_mut_crl` looked only at CRLs sent on `pki.flow`.
   Each series has one sent CRL, because the t0 CRLs exist only in `pki.state`. Fix: the
   faults also reach CRLs that were issued but never sent, as `rule_crl` already counts them.
   crl-index and crl-backdated are now red on conf. crl-drop-entry now picks a CRL whose
   predecessor holds an entry: red on mbr, n/a on conf.

### 8.6 Disagreements on HEAD

**OF-3 (PLAUSIBLE): no record shows a device holding a CRL before the first revocation.**
R-NOCRL (BUILD-STATUS 2026-10-22: "every CRACA publishes at t0, and devices join holding
those lists") makes each provider issue an empty full CRL at t0. It is `version 1`,
issueDate 719,571,005, nextCrl +7 d, and it is recorded in `pki.state` and the capture's
`crls.json`. But:

* No `pki.flow` message carries it. The CRLG's `crl-publish` and `crl-broadcast` messages exist only for the 191,400 s CRL. In real operation a device gets a CRL by download from the CRL store, and the device's other pre-drive exchanges (enrolment, Download-Reqs) are recorded.
* The devices' `node.security` rows match 4924a5d2, which had no t0 CRL: `crl_entries 0, crl_version 0` on every row before 196,200 s (6,475 rows on HEAD), then `3 / 3`.

So the records cannot tell whether a device holds the t0 CRL, and therefore its nextCrl. That
is the precondition for the 2-week overdue rule that R-NOCRL was meant to enable. Two causes
are possible. Either the devices do not hold it (an engine defect), or they hold it and
neither the fetch nor `crl_version` records it (an observability defect). Which one cannot be
told without reading the engine. Owed: record the pre-run CRL fetch as a flow, and make
`node.security` report the held CRL's (craca, series, version). Then a rule can check that
every device holds each provider's current CRL from t0.

**Observations (not disagreements)**

* **O-3: RSU-filed backhaul reports have no bytes.** conf 583, mbr 1,114 (`rsu-backhaul`, unchanged from 4924a5d2). MBR-UP exempts this labelled path, so no checker verifies these reports. [SM-MBR] defines only the vehicle upload.
* **O-2 still applies:** 4 Saturday decisions on mbr are late under the strict reading of [SM-PROV] 3.4.10.
* **RA-BLOCK:** 2 requests after blacklisting were refused on mbr, as on 4924a5d2.

**Open:** OF-3. The full-scale benchmark and the PQ runs are outside this package. The
`.info`, RSU-request and download-decryption limits in §7 still apply.
