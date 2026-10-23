# 06 — How close the engine's SCMS is to the real US deployment: validation summary

First written 2026-10-18 at `8245ef81`. Rewritten 2026-10-22 at `0f7eda5c`. **Rewritten again
2026-10-22 at `bc3bca1b`** (main), after:
- fix wave 2 (TRUST, REPORTS and DETECT, merged at `88e24b09` with merge fix `3471f2fc`);
- the final test wave (ORACLE-FINAL and BENCH, merged at `bc3bca1b`).

This page states the final state of the realism round: the verdict on every real-world rule,
the test totals across every configuration that ran, what is confirmed working, and what
still deviates or cannot be checked, with its impact.

It is for the owner and for a reviewer from outside the project. It goes no further than its
sources. The sources are the four validation checklists, the testing reports, the benchmark
and the merge notes in `docs/design/BUILD-STATUS.md`. Every claim names its row or report.

**How to read the row references:**
- **crypto #n**: `validation/crypto-formats.md` §1, row n.
- **proto Xn**: `validation/protocols.md`, item Xn.
- **gov X-n**: `validation/governance-deployment.md` §1, row X-n.
- **veh Xn**: `validation/vehicle-and-roadside.md` §1, row Xn.
- **07 §n**: `07-config-testing-report.md`.
- **matrix**, **stress**, **oracle-final**, **final-bench**, **usfp**, **pq-profile**,
  **knobs**, **effects**, **edge**, **realism**: the reports of the same names in `testing/`.
- **bench §n**: `docs/benchmark/scms-real-benchmark.md`.
- **BS wave 2** and **BS final**: the BUILD-STATUS sections "SCMS realism fix wave 2
  integration" and "SCMS realism final test wave integration" (both 2026-10-22).
- **04 §n** and **RT Rn**: `04-validation-report.md` and `docs/redteam/pki-privacy.md`.

The checklist files keep the verdicts their validators recorded on 2026-10-18. The only
exceptions are proto A16, D18 and I10, which were updated in `898c077a`. **Where a verdict
here differs from its checklist, this page is the later one.** §4 lists every such change
with its evidence.

The test reports `matrix.md` and `stress.md` were written before fix wave 2. Some of their
"open" items are now fixed; §2.2 and §6 give the current state of each.

## 1. What was checked, and how

Four validators each took one part of the SCMS. Each one listed every rule that the real US
system follows in that part, from primary sources:
- the IEEE 1609.2 / 1609.2.1 ASN.1 modules;
- the SCMS Manager documents (1609.2.1 Profile, Provider Requirements, Elector Policy and
  Technical Specification, EAC Requirements, re-enrolment specification, misbehaviour report
  specification);
- CAMP's EE Requirements 1.2.2;
- the USDOT primer and the CV-pilot reports;
- the CTLs and certificate chains that SCMS Manager actually publishes.

Code comments were never used as a source. Each rule's evidence is one of five kinds:
- an official test vector;
- the independent Python oracle, which was written without reading the Rust;
- a byte-for-byte comparison with a published real artefact;
- a Rust test;
- **runtime evidence**: the oracle checking the PDUs that real runs recorded, and
  configuration tables comparing same-seed runs. Each expectation was committed before the
  run it judges.

Verdicts:
- **Matches.** The engine does what the source says, and the evidence is named.
- **Fixed.** The engine did not match. It was corrected in one of the rounds, and the new
  check was shown to fail on the old behaviour (the old code put back, or the old run's
  recording).
- **Documented deviation.** The engine does not match. The row gives the reason and what the
  difference changes.
- **Unverifiable.** No public source fixes the behaviour or value. The row says how the engine
  models it. Each such value is labelled ASSUMPTION in `constants.md`.

A row with two verdicts is counted under its first.

## 2. Final totals

### 2.1 Rules

| Area | Rules | Matches | Fixed | Documented deviation | Unverifiable |
|---|---|---|---|---|---|
| Cryptography and data formats | 80 | 61 | 10 | 4 | 5 |
| Protocols and backend flows | 109 | 87 | 12 | 3 | 7 |
| Governance, deployment and calendar | 70 | 44 | 10 | 10 | 6 |
| On-vehicle and roadside behaviour | 67 | 40 | 11 | 5 | 11 |
| **All** | **326** | **232 (71.2 %)** | **43 (13.2 %)** | **22 (6.7 %)** | **29 (8.9 %)** |

**Final match: 275 of 326 rules (84.4 %) do what the real system does** (matches plus fixed).
It was 264 (81.0 %) on 2026-10-18 and 270 (82.8 %) at `0f7eda5c`.

What moved since `0f7eda5c`: the five rules that runtime testing had reopened are now fixed
(§4.1). They are proto C6, C8 and F6, gov E-1 and veh R1. No rule was reopened by the final
test wave.

Of the 51 rules that do not match:
- 29 cannot be checked against any public source (§7);
- 22 are known differences (§5). 14 of them change nothing a device trusts or does.

The runtime defects that testing found and that have no checklist row are in §6. The
largest one in the earlier version of this page, R-USFP (the US misbehaviour profile
revoking honest vehicles), is fixed. The largest still open are R-CCSIG, which affects only
the post-quantum certificate-cycle study, and the full-fleet false positives and memory
growth found by the benchmark.

### 2.2 Tests across all configurations

**Configuration tables.** In each table the expectations were written before the runs, and
each run was compared with its base on the same seed. The counts are the verdicts recorded
in each table's expectations file.

| Table | Rows | Pass | Fixed (bug, then pass) | Expectation wrong, corrected | Open |
|---|---|---|---|---|---|
| Matrix: 32 runs, one-change variants of the Midtown realism base; 7 re-run after the R-USFP fix (`scms-matrix.yaml`) | 99 | 83 | 5 | 8 | 3 |
| Realism rows re-run on the matrix runs (`runtime_expectations.yaml`; before the R-USFP fix) | 62 | 49 | 0 | 1 | 12 |
| Settings knobs, same-seed pairs (knobs) | 21 | 15 | 3 | 2 | 1 |
| Effects table (effects) | 88 | 77 | 7 | 4 | 0 |
| Edge cases (edge; 3d fixed in 07 §7; 11 measured by matrix and stress) | 12 | 7 | 5 | 0 | 0 |
| Runtime realism, base (realism) | 27 | 22 | 5 | 0 | 0 |
| Runtime realism, changes (realism; R-TRUSTLEAK now fixed) | 8 | 6 | 2 | 0 | 0 |
| Stress: multi-week, post-quantum, first full-fleet seconds (`scms-stress.yaml`) | 35 | 16 | 1 | 2 | 16 |
| Final bench: full fleet, hybrid ML-DSA-44, certificate cycle, determinism (`scms-final-bench.yaml`) | 28 | 15 | 0 | 3 | 10 |
| Oracle-final on HEAD `88e24b09`: expectation rows, conf and mbr runs (oracle-final §8.1) | 60 | 60 | 0 | 0 | 0 |
| Oracle-final: HEAD against `4924a5d2` and base against variant (oracle-final §8.2) | 10 | 9 | 0 | 1 | 0 |
| **All** | **450** | **359** | **28** | **21** | **42** |

**What the 42 open rows are.** Only part of them is an engine problem:
- **15 are superseded or wait for a re-run.**
  - Stress: 10 full-fleet rows (`FS.*`) were never run; the final bench measured them as
    `FB.*`.
  - Stress: `PQ.certcycle` and `PQ.determinism` were measured by the final bench. The first
    found R-CCSIG; the second passes.
  - Stress: 3 rows (`MWC.overdue-onset`, `MWC.recovery`, `MWP.overdue-on-return`) were open
    on R-NOCRL. It is fixed (§6), but the multi-week runs were not re-run.
- **12 are the realism rows**, judged on runs from before the R-USFP fix and not re-run.
  Some of them carry `status: open` by design.
- **10 are final-bench rows.**
  - 4 were not reached, because the full-scale run hit the memory cap (`FB.devices`,
    `FB.topups-at-rollover`, `FB.ff-ticks`, `FB.cost`).
  - 2 are findings: honest false positives and privacy link claims (§6 items 3 and 5).
  - 4 are R-CCSIG (`PQHMC.*` ×3, `PQHFC.ccsig`).
- **Matrix, 3 rows.** `lte-latency-budget` is a real open item (§6 item 11). `att10-crl` and
  `att10-cases` are explained: after the R-USFP fix the decisions are 2 in M0 and 4 in
  att10, which is the "up" the rows expect (usfp §4). The rows themselves were not re-judged.
- **Knobs, 1 row:** the request shuffle, published as partial (knobs).
- **Stress, 1 row:** `PQ.backend-bytes`. The per-certificate factor was not isolated.

**Two notes on the table:**
- **The knobs row** counts its 21 rows one by one. The 07 §2 summary gives 13 pass and 4 fixed
  for the same rows; the difference is only which column a row is counted in.
- **Not in the table:** the policy package's before/after runs (07 §8), and usfp's same-seed
  before/after table (§3). Neither is a row. On `edge/clock-skew-ahead`, honest vehicles
  revoked fell from 19 to 0. On the realism base under the US profile, honest vehicles
  revoked fell from 26 to 0.

**Independent checks**

| Check | Result |
|---|---|
| Oracle self-test (official vectors) | 412/412 |
| Oracle mutation run | 24 faults, all red |
| Oracle `verify-emitted` | 10/10 |
| Oracle `records` on two recorded runs of HEAD `88e24b09` (conf and mbr) | **279,236 of 279,236 checks pass** (conf 67,451, mbr 211,785). On `4924a5d2` it was 272,778 of 281,240, every failure OF-1 or OF-2 |
| Record faults injected into those runs | 69 of 69 applicable faults red (conf 34, 1 not applicable; mbr 35), 0 silent, after oracle fix 2 (oracle-final §8.5) |
| `ctl-files` on the engine's own CTLs (F1 `gov.json`, HEAD) | PASS: CTL-FILE 16, CTL-ORDER 4, CTL-QUORUM 6, CTL-SIG 140; 6/6 faults red after oracle fix 1 |
| `ctl-files` on the six CTLs SCMS Manager publishes | 30/30 elector signatures valid; 5/5 faults red |
| `check_run.py` on HEAD's F1 conformance capture | 18,910/18,910, 0 disagreements; `--mutate` 18/18 faults red |
| `oracle_final.rs` reproductions (OF-1, OF-2) and `conformance.rs` on HEAD | 2/2 and 2/2 |
| `check_run_crl.py` on a newly generated CRL corpus (`2004bb87`) | 19/19 |
| Published artefacts; `real_artefacts`; `profiles.py`; PDU checkers; conformance run (`8245ef81`, not re-run since) | 164/164; 7/7; 24/24; 24/24, 25/25, 18/18, 4/4; 19,183/19,183 |

**Rust, UI and build checks**

| Suite | Result |
|---|---|
| HEAD `bc3bca1b`: v2xw-sec, msg, proto, node | 1,014 pass, 0 fail, 1 ignored |
| HEAD `bc3bca1b`: engine, 24 SCMS, phase2 and sensitivity test binaries | 167 pass, **6 fail**, 7 ignored. The 6 are `sensitivity_radio` ×5 and `sensitivity_security::the_revocations_move_the_crl_size`. None of their inputs changed in the final merge, so they were on `88e24b09` too; not root-caused. The wave 2 run reported `sensitivity_*` as passing, so when they began failing is not established |
| `88e24b09`: sec, msg, proto, node, mobility | 1,322 pass, 5 fail. All 5 are known mobility traffic tests (calibration ×2, `pedestrian_invariants` ×2, kerbside), and none uses GNSS. Proto after merge fix `3471f2fc`: 316/316 |
| `88e24b09`: engine, 26 SCMS test binaries | 137 pass, 0 fail, 5 ignored |
| `88e24b09`: `scms_knobs_runs` | 17/18, then the shuffle test 1/1 after merge fix `3471f2fc`. The full suite was not re-run after the fix |
| `fragmentation::a_padded_message_…` (M-RX1) | Fails. It also fails on the detect branch's tip, so it predates fix wave 2 |
| `2004bb87`: sec, msg, proto, node, threat, metrics; engine lib plus 18 SCMS binaries | 1,501 pass, 0 fail; 195 pass, 0 fail |
| v2xw-server and v2xw-cli (`2004bb87`) | Pass except `longrun_budget`: 24.9 MB against a 24 MB budget. It already failed before (24.8 MB) |
| `timeline::a_param_change_of_the_arrival_rate_takes_effect_at_its_instant` | Fails (5.29×, band 2.5–5.0). It is traffic-only, has no SCMS, and predates this round |
| UI (`88e24b09`) | protocol and viewer `tsc -b` pass; Studio typecheck passes; Studio 326/326 unit and 32/32 e2e. The Studio PKI fixtures were re-recorded (`06fe6a72`) |
| Release build and boot (`bc3bca1b`) | Release build OK (7 m 18 s); the simulator boots on port 8758, and `/agent/status` and `/rpc/schema` both answer 200 |

## 3. What is confirmed working

Each item below was confirmed by an independent check or by every configuration that tested it.

**1. Formats and cryptography.** Wherever a public test vector or real artefact exists, the
engine reproduces it byte for byte:
- the COER encoding, re-encoding every published file (crypto #1);
- HashedIds and Time32 with leap seconds (#5–8);
- ECDSA P-256/P-384, ECIES, AES-CCM, KDF2 and MAC1 (#52–61);
- butterfly expansion and linkage values (#63–70);
- the six published CTLs (#78).

On HEAD the oracle re-encoded all 54,446 recorded PDUs canonically (DECODE: 16,131 on conf,
38,315 on mbr) and verified every signature with its own ECDSA.

**2. Certificate profiles per provider.**
- provider-a takes ISS's published form: a 70-year root, the root as CRACA, the CRL signer
  root-issued on series 256, and the region on sub-CAs.
- provider-b takes SAESOL's production form.
- The oracle's CRACA-CERT and CRL-SSP rules pass on both forms.
- A governance CRL can revoke a governance sub-CA.

**3. Enrolment (fixed in wave 2, OF-1).** Each device has one enrolment certificate. Its own
ECA exchange issues it, with a back-dated start and the full 6-year lifetime; the DCM
registers it; and it signs every Download-Req. On HEAD: DL-SIGN 2,892 and 10,898 checks
pass, ENR-LIFE 58 and 99 pass, and 0 devices hold a foreign certificate (oracle-final §8.1,
§8.2). The F1 capture now has 62 EeRaCertRequests, against 118 when each device had two.

**4. SCMS architecture, in all 32 matrix runs.**
- No separation-of-duties violation, including under the CAMP topology with its LOP.
- Sealed messages are opened only by their recipient.
- A device talks only to its RA and the DCM.
- One provisioning request per OBU over its life (0.54 requests/s scaled to 17 M vehicles,
  against CAMP's 0.5). At full fleet: 40 certificates for each device's request, about 10
  requests/s during the fill (final-bench §2).
- 20 certificates per device at the weekly top-off, and exactly ×3 under the NYC 60-a-week
  profile.
- I-S1: real and modelled cryptography give identical records.

**5. Message sizes.** provider-b matches CAMP Table 3 exactly: response 454/470, ack 292/294,
ECA response 958. CRL sizes are 328 B with one entry and about 33 B for each further entry.
provider-a sizes are under §6 item 9.

**6. Trust list (fixed in wave 2, R-TRUSTLEAK).** The two kinds of frame the engine sizes
instead of encoding (the RSU's CRL broadcast and a report relayed over the air) now carry
their signer's certificate. Every receiver holds them to its installed CTL roots. With
provider-b off the trust list, provider-a receivers verify 0 of its messages (280 before).
At full fleet, 2,019,081 SPDUs were checked with 0 refused (final-bench FB.trust). The engine's
own CTL bytes pass the oracle's `ctl-files`, with a 3-of-5 quorum (oracle-final §8.4).

**7. Revocation path on the real calendar.** Every case went through the steps the deployment
prescribes:
- the shuffle (at most one day);
- the 8 h review;
- the next business-day 10:00 ET slot, with federal holidays skipped (the Thanksgiving week
  passes);
- the first fetch after publication.

The oracle confirms the CRL rules independently on HEAD: CRL-SEQ, CRL-KEEP, CRL-SLA, CRL-PUB,
CRL-MATCH and RA-BLOCK. Every device the oracle finds on a CRL is an attacker in the ground
truth: 4 of 4 decided on conf, 12 of 12 on mbr.

**8. CRLs from t0, full lists only (fixed in wave 2, R-NOCRL and O-1).**
- Every CRACA signs an empty full CRL at t0 and renews it by `nextCrl`, as a deployment
  running since 2015 does. On conf: `F614/0` at t0, then `F615/0` with the revocations.
- Full CRLs only, with no gap in `indexWithinI`: on mbr 614/0, 615/0, 615/1, 615/2, 616/0,
  each `nextCrl` exactly 7 days on.
- At full fleet, 2,000 receiver installs of the t0 CRLs (2 per device) and 60 RSU installs
  (FB.crl). See OF-3 (§6 item 2) for what the records do not show.

**9. CRL freshness over five weeks** (stress multi-week, on `2004bb87`).
- Five i-period rollovers, each signing under the right period.
- 10 CRL versions in 33 days, reissued by `nextCrl`.
- 12,592 conditional polls answered 412, and 0 refusals.
- The 2-week overdue rule acts in a run. With the CRL Generator down after its first CRL, every
  message under that CRACA was refused once the list became overdue (87,823 refusals), and
  traffic recovered after the Generator came back.
- Re-enrolment: 1 of 75 devices, the expected 1.1.

**10. Downloads and report uploads (OF-2 fixed in wave 2).**
- Every download rule passes the oracle on both HEAD runs: DL-ACK, DL-ENC, DL-SIGN, DL-TIME,
  DL-NAME, DL-AFTER and DL-WAIT. Conditional GET with 412 is in force, and so is `Range`
  resume.
- Every cellular report upload carries its SPDU. MBR-UP passes 3,930 and 18,618 checks; the
  1,695 uploads with no bytes are gone. A detection that cannot be built as an SM-MBR v1.1
  upload is not filed (`reports_not_filed`).

**11. Retries and outages.**
- The EE retries after 10 s, at most 10 times an hour. With an RA outage across the top-ups,
  12 of 12 complete.
- A message between components waits for a component that is down, and the component's timers
  run when it restarts.
- A device that is switched off reaches no part of the SCMS (F-OFF).

**12. Receiver rules.**
- 1609.2 receiver validation rule by rule (veh R2–R9, R12–R18), and the CTL chain (R1).
- The device checks the CTL's elector quorum.
- A composite CRL is kept when one of its CRLs is unverifiable.
- Uncompressed explicit keys are accepted.
- The `generationTime` relevance window is enforced.
- One car with a 5,000 s clock error no longer gets anyone honest revoked (19 → 0).

**13. The US misbehaviour profile (fixed in wave 2, R-USFP).** The cause was the GNSS input,
not the detector: the raw 10 Hz fixes carried fresh 1.6 m white noise each, so consecutive
honest BSMs disagreed by 2.65 m at the median. The US presets' receivers now output a
smoothed navigation solution (τ 100 s) with a 0.3 m/s static hold. On the realism base, same
seed (usfp §3):
- honest vehicles revoked 26 → 0; attackers revoked 1 → 2 of 3; reports at the MA
  7,488 → 235;
- att10: 0 honest, 4 revoked. sybil: 0 honest, 2 revoked;
- every vehicle attacker heard by at least 3 reporters is revoked;
- `gnss-raw` reproduces the old run to the report, so the receiver output accounts for all
  of R-USFP. Four tests were proved red with their fault injected.

**14. Metrics on the realism base.** All are inside their sourced bands:
- PDR 0.364;
- BSM latency p95 14.6 ms (budget 100 ms);
- CBR p95 1.4 %;
- AoI 82 ms;
- security at 46 % of BSM bytes;
- full-certificate share 17 % (re-measures veh S6);
- certificate pool mean 25.7;
- no signature, parse or backend failures, and no starvation.

**15. Full fleet, first 155 s** (bench §0, final-bench §2). Within 100 m the PDR is 0.84,
the same as the quarter fleet. The other numbers:
- BSM latency mean 12.39 ms, p99 14.94 ms;
- CBR mean 0.074–0.078 and p95 0.14–0.16 after the fill, below J2945/1's 0.5;
- MBR airtime 0.21 % (it was 34 % at quarter fleet under R-USFP);
- 20 valid pseudonyms at every census, and 0 starved.

**16. Determinism.** Record digests are identical at 1 and 4 threads. That holds on the 33-day
run, on the first 5 s of the full fleet, and on the hybrid ML-DSA-44 run, where
`metrics.json` and `run-report.json` are byte-identical too. The check can fail: three runs
that differ in one setting each give three digests.

**17. Post-quantum at density**, quarter fleet. These are modelled sizes and costs, since no
standard defines PQ encodings (pq-profile):
- ML-DSA-44: 2.30 frames per BSM and BSM airtime ×12.6; verified BSMs −33 %.
- Hybrid ML-DSA-44: 2.30 frames per BSM, airtime ×13.0, CBR 0.125, verified −33.8 %.
- Hybrid Falcon-512: airtime ×4.4.
- R-PQMBR is fixed. An over-MTU frame is refused and counted as refused, not as uploaded.
  Under `generic-sdu` it is fragmented: 6 relayed reports were delivered this way, and 0
  were miscounted.
- Hybrid top-ups complete after the RA's request shuffle. Nobody starves.

**18. Speed.** One check redid a P-256 multiplication for every pseudonym at every backend
step. With a memo, the realism base went from 2,074 s to 164 s with identical results.
`pq-base` went from 1,175 to 125 CPU-s.

## 4. Verdicts changed in this round

### 4.1 Fixed (16 rules since 2026-10-18)

**Fixed in fix wave 2.** These are the five rules reopened at `0f7eda5c`.

| Row | Before | Now | Evidence |
|---|---|---|---|
| proto C6 (the RA serves only registered enrolment certificates) | OF-1: the RA answered Download-Reqs signed by an enrolment certificate it never registered (112 on conf) | One certificate: the ECA exchange's own, registered by the DCM, signs every Download-Req | `d7b64fae`, `89f54fe4`, `3471f2fc`; oracle DL-SIGN on HEAD 2,892 / 10,898 pass (was 1,320 / 5,245 red); `oracle_final.rs::one_enrolment_certificate_…` passes on HEAD, and its logic is red on the `4924a5d2` recording |
| proto C8 and gov E-1 (enrolment certificate valid 6 years) | OF-1: a second certificate lasting 0.9–5.5 years; age modelled as a short lifetime | Back-dated start, full 6-year duration, clamped to the ECA's start. A late joiner's pre-run exchange runs at that start (merge fix `3471f2fc`) | ENR-LIFE 58 / 99 pass on HEAD (was 56 / 97 red); `enrolment::an_aged_enrolment_is_the_one_certificate_its_eca_exchange_issued` |
| proto F6 (each report is POSTed through the RA with its Download-Req) | OF-2: 1,695 of 2,991 cellular uploads on conf carried no SPDU | A detection that cannot be built as an SM-MBR v1.1 upload is not filed. Every upload carries its SPDU | `e8475281`; MBR-UP 3,930 / 18,618 pass (was 1,695 / 49 red); `oracle_final.rs::every_cellular_report_upload_carries_its_spdus`, red in Python on the `4924a5d2` recording |
| veh R1 (the signer chains to a root on the installed CTL) | R-TRUSTLEAK: 280 provider-b messages verified with provider-b off the CTL | Sized frames carry the signer certificate, and OBU, RSU and VRU receivers validate it to the installed roots | `89f54fe4`; off-ctl 280 → 0 (BS wave 2); `d18a1e80` proved red at the engine (45 unchecked) and at the RSU. No fault was injected into the OBU or VRU fallback |

**Fixed at `2004bb87`** (the PKI, flows and policy wave):

| Row | Before | Now | Evidence |
|---|---|---|---|
| crypto #4 (CF-UNCOMP) | Explicit certificate with an uncompressed key refused on the node path | Accepted after an on-curve check, then stored in compressed form | `22f7cf13` |
| crypto #36 (CF-CRACA; also #24's issuer) | ICA as CRACA for every provider; the governance CRL could not revoke a sub-CA | Per-provider profile: ISS uses the root as CRACA and a root-issued CRL signer on series 256; S2X uses the ICA. Governance follows the profile | `pki_profile::per_provider`; oracle CRL-SSP and CRACA-CERT on both forms |
| crypto #29 (was unverifiable) | Enrolment `cracaId` 000000: a series-4 CRL could never match | The ECA names the provider's CRACA, so a series-4 hash CRL revokes the certificate ([SMP] Table 1) | `22f7cf13`; oracle CRACA-CERT series 4 |
| proto B6 | No EE retry | 10 s, at most 10 an hour (SCMS-522/523); the RA re-acknowledges a byte-identical retry | `5d44674b`; edge 3c; RA outage 12/12 |
| proto D18 | No `Range` resume | A cut batch download resumes; the retry is answered 206 with the rest | `ee_retry.rs::an_interrupted_batch_download_resumes_with_range_for_the_rest` (proved red) |
| proto I7 | 40 B per CRL entry; CTL left out | The encoded `CompositeCrlSpdu`, home CTL included | `5d44674b`; edge: about 33 B an entry, 336 kB at 10,000 entries |
| proto I10 | No `If-Modified-Since` | Every due poll is a conditional GET, answered 412 (92 B, ASSUMPTION) when the copy is current | `report_signing.rs::every_due_crl_poll_asks_and_a_current_copy_is_answered_412` |
| gov L-1 | 30-year root for every provider | ISS form: a 70-year root `v2xrootca.ghsiss.com`. The 20-year key cryptoperiod is still not enforced; no run reaches it | `22f7cf13` |
| veh M2 | Detections with no SM-MBR v1.1 subtype filed | `detect/us-sm-mbr` files v1.1 subtypes only, and it is the default of every US real-world preset | `c5d5c5b7`, `7aa31661` |
| veh M3 | Research thresholds (12 m/s²; no MaxSpeed) | US profile: MaxSpeed 90 m/s, MaxAcceleration 10 m/s², RandomPosition 1 m. With the smoothed receiver (R-USFP), it revokes no honest vehicle on the realism base | `c5d5c5b7`; usfp §3 |
| veh M6 | Reports about messages that failed verification | The US profile judges only BSMs this receiver verified (SM-MBR §2.3.1) | `us_mbr.rs::only_verified_bsms_are_judged_or_remembered` |

Narrowed but not fixed: **proto A16.** The poll is on the air; the `infoStatus` SPDU body is
still not encoded.

### 4.2 Fixes that change runtime behaviour but no checklist row

Each was fixed and proved red, as its report describes.

**In fix wave 2** (BS wave 2):
- **R-USFP**: the smoothed GNSS receiver in every US preset (§3 item 13).
- **R-NOCRL**: every CRACA publishes at t0, and devices join holding those lists.
- **O-1**: full CRLs only; the deltas were signed and never sent.
- **R-PQMBR**: over-MTU application frames are refused and counted, or fragmented under
  `generic-sdu`.
- **The late joiner's pre-run exchange** (merge fix `3471f2fc`). Before it, the RA refused
  34 of 125 joiners' aged certificates as not yet valid.

**At `0f7eda5c` and before:**
- The MA ran one case at a time. Its cases now run side by side (R-SERIAL, gap row 27).
- The CRL Generator reissues by `nextCrl` when no entry has been added.
- Devices install a newer CTL.
- The node clock reaches security.
- No signing with an expired pseudonym across the rollover (edge 1b/1c).
- F-OFF: a switched-off device reaches no part of the SCMS.
- The AoI fast-forward charge (matrix fix 2).
- The pseudonym usability memo (matrix fix 1).
- Skewed dead-reckoning (07 §8.1).
- Fast-forward stops: 255,658 → 4,290 (R-FF).
- The PKI-record defects R-LOP, R-SHARE, R-CASE, R-CORR and R-OFFCTL, and the two merge
  fixes in 07 §5.

**On the oracle side** (`1637acb3`). Two oracle defects were fixed. The `ctl-files` faults
could never fail on the engine's capture, and the CRL faults could not reach the t0 CRLs
(oracle-final §8.5).

## 5. Every documented deviation and what it changes (22)

### Cryptography and data formats (4)

| Row | Difference from the real system | Impact |
|---|---|---|
| crypto #10 (CF-IPERIOD); also gov K-5 | i-periods start on the UTC 09:00 grid, not on a uniform Time32 grid from the epoch | 2 s on every pseudonym `validityPeriod.start`. The two validators read the sources in opposite directions. **Owner decision** |
| crypto #30 (CF-ENR-REQ) | The enrolment request carries `crlSeries` 4, which breaks the ASN.1 `(0)` | Follows [SMP] §6.3.1, which overrides the ASN.1. Same bytes on the wire. No trust effect |
| crypto #35 (CF-RSU-SSP); also proto E7 | RSU application certificates carry no SSP | Probably not a deviation: gov E-8 finds no SPaT, MAP or TIM SSP in CAMP Table 1 (§8 item 1) |
| crypto #38; also veh R11 | The published RA permission group breaks the strict chain-length rule. The engine accepts it, as deployed devices do, and counts each acceptance | A real-world quirk the engine reproduces on purpose |

### Protocols and backend flows (3)

| Row | Difference | Impact |
|---|---|---|
| proto A16 (narrowed) | The `infoStatus` SPDU body (crl/ctl/caCcf/ma/ra dates) and its RA signature are not encoded. The device polls the composite CRL conditionally instead | A change costs one exchange instead of two. Poll bytes are close to real |
| proto E7 | Same as crypto #35 | §8 item 1 |
| proto H10 | `indexWithinI` saturates at 255; a real CRL Generator would have to stop issuing | Only reached with more than 127 publications in one week |

### Governance, deployment and calendar (10)

| Row | Difference | Impact |
|---|---|---|
| gov G-2 | The board's review and investigation delays before a vote are not modelled | None on CTL rules, which start at the vote |
| gov G-10 | No scenario key schedules the yearly Test Ballot | A run longer than a year has no test-ballot record |
| gov G-11 | Post-quantum elector key replacement is not modelled | None today. No method has been chosen |
| gov C-9 | CTL `effectiveDate` is the publication deadline, not the ballot date | 0–28 days late. No trust effect: devices do not check it |
| gov C-10 | A run's first CTL is sequence 0; the live production CTL is sequence 1 | Sequence numbers run one lower. No trust effect |
| gov C-11 | A CTL that drops a root does not also carry the dropped root's certificate | Bytes only |
| gov C-14 | The couriered flash drive and the video-call key confirmation are not modelled | Not on the network |
| gov L-8 | Root termination (365 days' notice) is not modelled; removal is | Only a run of a year or more |
| gov L-11 | The LA SSP is version 2 with `laId` 1 and 2; ISS's LAs carry version 1 with ids 4 and 5 | `laId` is the provider's own choice |
| gov K-5 | Same as crypto #10 | 2 s on validity starts |

### On-vehicle and roadside behaviour (5)

| Row | Difference | Impact |
|---|---|---|
| veh R10 | Country regions are not checked against a location; they are only counted | None for US pseudonyms, which carry no region (`region_unchecked` = 0) |
| veh R11 | Same as crypto #38 | Counted |
| veh C6 | "2 km since the last change" is measured as distance travelled, not straight-line distance | On a winding route a change can come slightly early |
| veh M11 | Relayed report uploads go on safety channel 172, not on a service channel advertised by WSA | Report airtime on 172 is an upper bound (0.21 % at full fleet). The radio has no per-channel MAC queues and no channel switch (07 §7 item 4) |
| veh U5 | No WSA encoder | The SCMS service cannot be advertised, so M11 cannot be fixed yet |

Fourteen of these change nothing a device trusts or does: crypto #30, #35 and proto E7, crypto
#38 and veh R11, crypto #10 and gov K-5 (2 s), and gov G-2, G-11, C-9, C-10, C-11, C-14 and
L-11. The other eight are proto A16 and H10, gov G-10 and L-8, and veh R10, C6, M11 and U5.
Each of them matters only to long runs, to bytes on a poll, or to where report airtime is
counted.

## 6. Runtime defects and findings with no checklist row

In order of impact. Each has its evidence in the named report. Items fixed in wave 2 are in
§4.2.

1. **R-CCSIG: the post-quantum certificate cycle signs every message with the hybrid
   scheme** (final-bench §3). Open, engine defect.
   - The design the fragmenter cites (Twardokus et al., NDSS 2024, Partially-Hybrid) keeps
     ECDSA on every BSM.
   - Under ML-DSA-44 every BSM and PSM is over the MTU, so the run transmits nothing (50,180
     refused).
   - Under Falcon-512, BSMs are 1,045 B, where the design gives about 380 B.
   - The validator accepts both configurations.
   - **Impact:** every certificate-cycle number so far is wrong. The real US P-256
     deployment is not affected. The fix reverses `45f0942f` and moves the cert-cycle
     digests (owner decision, §9).
2. **OF-3: no record shows a device holding the t0 CRL** (oracle-final §8.6, PLAUSIBLE).
   - The t0 CRLs are in `pki.state`, but no `pki.flow` message carries them.
   - `node.security` reads `crl_version 0` until the first revocation CRL.
   - Against that, the run report counts 2,000 receiver installs of the t0 CRLs at full fleet
     (FB.crl). So a recording gap is the likelier cause, but the oracle cannot tell.
   - **Impact:** the 2-week overdue rule from t0 cannot be verified independently. Owed:
     record the pre-run CRL fetch as a flow, and report the held CRL's (craca, series,
     version).
3. **Honest false positives at full density** (bench §0.1 item 3; FB.honest-revoked).
   - In 40 s at full fleet, the US profile filed 229 reports about 32 honest devices. Most
     verdicts were `constantPositionFrozen` (165 of 236), with a median speed of 0.18 m/s in
     queues.
   - The two-day consequence (cases, decisions, revocations) is unmeasured, because the run
     did not finish.
   - On the realism base the residue is 5 reports about one honest vehicle, with none
     revoked (usfp §6).
   - The 0.3 m/s hold and the 100 s smoothing constant are ASSUMPTIONS on cited mechanisms.
4. **Memory grows by about 1.4–2 MB per node on the map, plus transient peaks** (bench §0.1
   item 1).
   - The full-scale benchmark hit the 2.5 GB cap at about 155 of 172,800 simulated seconds.
   - Finishing it needs 3–4 GB and 4–5 CPU-hours.
   - **Impact:** there is no full-fleet two-day result: no rollover window and no
     fast-forward.
5. **The privacy observer claims links with no pseudonym change** (bench §0.1 item 2).
   2,873 link claims, 0 correct, 0 pseudonym changes in 40 s. Any linkability number from the
   observer is unreliable until it is explained.
6. **The US profile cannot report a handset** (usfp §3). SM-MBR v1.0 defines reports for
   the BSM (PSID 0x20) only, so a PSM-sending attacker goes unreported. This is a gap of
   the real profile, which the engine reproduces faithfully. The research suite catches the
   attacker through its stale/replay check.
7. **R-UNSEC and O-3: an RSU's own reports have no bytes** (07 §7 item 2; oracle-final §8.6).
   On conf 583 and on mbr 1,114 `rsu-backhaul` reports are sized only. MBR-UP exempts this
   path, so no checker verifies them. Whether an RSU may sign an SM-MBR with its application
   certificate is an owner question.
8. **Post-quantum path, modelled only** (pq-profile §3; stress §7). Under PQ:
   - RSUs install no CRL (60 → 0), and have no application-certificate cycle;
   - receivers check CRL entries against the Generator's list, not a signed CRL;
   - every top-up is a fresh request (there is no PQ butterfly).
   No standard defines PQ encodings, so no PQ number is a real-world number.
9. **provider-a sizes** (matrix Open 5).
   - The ISS form gives response 501/517, ack 304/306 and ECA response 1,064. CAMP Table 3
     gives 454/470, 292/294 and 958.
   - Most likely its published component certificates are longer. No oracle check of the
     ISS profile exists yet.
10. **Unexplained, not yet shown to be defects:**
    - **The 43,125 s-old evidence.** 10 reports quoted a certificate's BSM from before the
      fast-forward. They are gone with the R-USFP fix (0 in every run after it), and the MA
      had set them aside correctly. The receiver path that produced them is not root-caused
      (usfp §5).
    - **Fast-forward ticks.** 865,653 in the edge peer-root-removed run.
    - **Case-board review time.** The board shows 28,799.2 s, against the expected
      ≥ 28,800 s (matrix L5).
    - **O-2.** Four Saturday decisions on mbr are late under the strict reading of [SM-PROV]
      3.4.10, which does not say which reading applies.
11. **Outside the SCMS:**
    - LTE-V2X BSM p95 latency is 112 ms, over the 100 ms budget, because the BSM is not aligned
      with its SPS reservation.
    - Rebroadcasting a 336 kB standing CRL every 5 s on channel 172 costs about 0.54 Mbit/s per
      RSU (07 §7 item 5). This is a configured behaviour, not a defect.
12. **Red-team finding.** The `generationTime` window makes a replay older than 2 s harmless,
    but the replayer is never reported or punished: 11 of 11 replayers were revoked before the
    window, and 0 after it (07 §8.1).

## 7. Every unverifiable item and how it is modelled (29)

### Cryptography and data formats (5)

| Row | Unknown | How the engine models it |
|---|---|---|
| crypto #26 | The DCM certificate (none published) | The RA's lifetime and the 1609.2.1 `DcmSsp` |
| crypto #32 | Whether pseudonyms carry a region | None. Gov F-2 decided this from [SMP] §6.3.3 (§8 item 2) |
| crypto #59 | The CCM nonce rule (the body text is paywalled) | A fresh random 12 bytes each time, never reused under one key |
| crypto #71 | The group linkage `jValue` | Group linkage is off (US practice). CAMP's `glv` vectors pass |
| crypto #73 | The LCI, PLV-request and LA key formats (NULL interfaces) | Modelled formats. The LCI is a 16 B ASSUMPTION |

### Protocols and backend flows (7)

| Row | Unknown | How the engine models it |
|---|---|---|
| proto B5 | The RA's 1 s TCP ACK as a bound | An M/M/c queue whose delay is measured |
| proto B9 | 1609.2.1 URL paths and HTTP headers | CAMP REST paths and ports. Request lines are not counted |
| proto C13 | The pilots' manual re-enrolment | SCMS Manager's online process through the RA |
| proto D11 | `download-maxAge`, `ra-maxAge`, `eca-*` (paywalled tables) | Not enforced. The engine never breaks them on the maximum-age reading |
| proto D17 | The first-batch delay | 60 s after the shuffle releases |
| proto F9 | J3287 / ETSI TS 103 759 report content | The deployed SM-MBR format |
| proto H8 | `nextCrl − issueDate` | 7 days |

Each of these MATCHES rows also contains one unpublished number:
- proto D4: the LA–ACA key rotation, 7 d.
- proto E6: "1 month" read as 30 d.
- proto G7: the MA case cap, 1,000.
- proto G8: the manual review, 8 h.
- proto H7: the CRL slot, 10:00 ET.

This round added more labelled ASSUMPTIONs:
- the 412 answer's size, 92 B;
- the retry between components, 10 s;
- the `generationTime` window. Its values are C2C-CC's (2 s, 600 s, 40 ms), because the US
  J2945/1 values are not public;
- the GNSS receiver's smoothing constant (100 s, after DO-229's carrier smoothing) and static
  hold (0.3 m/s, after u-blox).

### Governance, deployment and calendar (6)

| Row | Unknown | How the engine models it |
|---|---|---|
| gov G-1 | EAC votes and membership | A decision is an instant the scenario gives |
| gov C-4 | What an elector removal looks like | `electorRemove`, the standard's form |
| gov D-9 | Data-centre regions | Assumed public-cloud metros |
| gov D-10 | Backend latency | Distance over fibre at about 2/3 c, plus an overhead per link |
| gov D-11 | The fleet split between providers | Seeded, stable shares |
| gov E-2 | How "6 years" of enrolment is encoded | `hours(52560)` |

### On-vehicle and roadside behaviour (11)

| Row | Unknown | How the engine models it |
|---|---|---|
| veh S1 | Deployed OBUs' verification policy | Verify-all, on-demand and prioritised. On-demand with detection now warns (07 §8.2): reports fall from 897 to 79 |
| veh S6 | The full-certificate share | Re-measured in steady state at 17 % (matrix) and 16.1 % at full fleet, inside the sourced band of 5–30 % |
| veh P7 | P2PCD concurrency limits | 8 responses, 8 inline entries |
| veh P8 | Which P2PCD flavour US devices run | Out-of-band |
| veh C10 | Whether J2945/1 clears the path history at a change | The code clears it (§8 item 4) |
| veh C11 | The pseudonym-change rule for J2945/9 handsets | The scenario's rule |
| veh M9 | The device report store quota | 64 KiB |
| veh M10 | The report rate limit | At most one per second per (reporter, accused) |
| veh M13 | Whether RSUs run misbehaviour detection | They do, as reporters. Their reports are sized only (§6 item 7) |
| veh V3 | The PSM rate | 1 Hz with a 100 ms floor |
| veh U3 | RSU verification policy | Verify-all |

## 8. Disagreements between the checklists

The totals count each row as its own checklist records it, with the changes in §4 applied.

1. **RSU SSPs: open.** crypto #35 and proto E7 record a deviation. Gov E-8 and F-3 read CAMP
   Table 1 in full and found no SPaT, MAP or TIM SSP in it. On that reading both rows match,
   and the deviations would be 20.
2. **Pseudonym region: open.** crypto #32 says unverifiable. Gov F-2 says "no region", from
   [SMP] §6.3.3.
3. **i-period time base: owner decision** (crypto #10 against gov K-5).
4. **Path history at a change: the checklist text is out of date.** veh C10 says the engine
   does not clear it. `vehicle.rs` `set_identity` does clear it.
5. **MA concurrency: settled.** The MA ran one case at a time; it now runs cases side by side
   (R-SERIAL, 07 §5 bug 1). The "27 decisions in every variant" were the R-USFP false
   positives, and are gone (usfp §4).

## 9. What remains

**Fix first (engine defects, in order of impact)**
1. **R-CCSIG**: under the certificate cycle, size and price each SPDU as ECDSA P-256, and
   verify the PQ signature once per new certificate. Have the validator refuse a
   configuration whose every SDU exceeds the MTU. The test is PQHMC (`frames_transmitted > 0`).
2. **Memory per node**: take a heap profile, then cut the per-node state so that the
   full-scale file fits.
3. **OF-3**: record the pre-run CRL fetch and the held CRL version, then add an oracle rule.
4. **Full-density false positives**: check the `constantPositionFrozen` reports against the
   recording. The 2-day consequence comes with the full-scale run.
5. **The privacy observer's link claims** with no pseudonym change.
6. **The 6 failing sensitivity tests**: root-cause them, and establish when they began.

**Owner decisions**
- CF-IPERIOD (2 s).
- R-CCSIG reverses the deliberate earlier decision `45f0942f`, and it changes the
  certificate-cycle digests.
- Whether RSUs may sign reports with their application certificate (R-UNSEC).
- The US J2945/1 window values.
- Whether SCMS Manager's artefacts may be vendored.
- The US profile stays the default of the US presets. With R-USFP fixed it revokes no
  honest vehicle on the realism base. It misses handset attackers, as the real profile does
  (§6 item 6).
- Delete `target/agents`. A permission check blocked its removal, and the disk is at 94 %.

**Engine work for the remaining deviations**
- The `infoStatus` body (A16).
- Per-channel MAC queues, a service channel and WSA bytes (M11, U5).
- Country polygons (R10).
- Governance scenario keys (G-10, C-9, C-10).
- Root termination (L-8).
- Stop issuing at 255 CRLs a week (H10).
- The post-quantum RSU certificate cycle and an RSU CRL fetch under PQ.
- SPS-aligned BSM generation for LTE-V2X.

**Testing still owed**
- **The full-scale benchmark to the end.** It needs 3–4 GB and 4–5 CPU-hours (bench §0).
- **Re-runs after fix wave 2.** These have not been re-run:
  - the full `scms_knobs_runs` after merge fix `3471f2fc`;
  - the three threat simulation binaries in release;
  - the 26 matrix runs not re-run for R-USFP (M0, gnss-rtk, att10, sybil, off-ctl and
    research-suite were re-run, and gnss-raw is new);
  - `runtime_expectations.yaml` on the smoothed-GNSS presets;
  - the stress multi-week rows that were open on R-NOCRL;
  - the conformance run of `8245ef81`.
- **A fault proof for the OBU and VRU receiver fallback** (R-TRUSTLEAK). Only the engine and
  RSU paths were proved red.
- **An independent check of the ISS profile** (§6 item 9).
- **F6/F7 fault proofs** (`inject2.py`), one test at a time.
- **Realism row V8.** Re-base it on `research-suite`.
- **The test reports.** Bring `matrix.md` and `stress.md` up to date: they still list fixed
  defects (R-USFP, R-TRUSTLEAK, R-NOCRL, R-PQMBR) as open.
- **Failing tests that predate this round:** `longrun_budget`, `timeline`, fragmentation
  M-RX1, and the 5 mobility traffic tests.

**Paywalled standards: buying them would turn unverifiable rows into checked ones.** These
are the IEEE 1609.2.1 body text (crypto #10, proto D11, B9), IEEE 1609.2 §5.3.8 (crypto
#59), SAE J2945/1 (veh S2, C10, the window values), SAE J2945/9 (V3, C11, V1) and SAE J3287
(proto F9).

**Values that no purchase can supply.** None of these is published anywhere, so the engine
will always model them, each labelled as a setting:
- data-centre placement and fleet shares;
- EAC vote records;
- the deployed OBU and RSU verification policies;
- live CRL sizes.

"Identical" can therefore only ever mean identical wherever the real system is observable. On
that measure, 275 of 326 rules (84.4 %) match. Every rule the runtime oracle checks passes on
HEAD: 279,236 of 279,236 checks.
- **What is left on the checklists:** 22 known deviations, 14 of which have no trust effect,
  and 29 rules no public source can settle.
- **What is left at runtime:** R-CCSIG (post-quantum study only), the full-fleet memory,
  false-positive and observer findings, and OF-3.
