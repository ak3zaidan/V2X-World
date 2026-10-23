# 06 — How close the engine's SCMS is to the real US deployment: validation summary

First written 2026-10-18 at `8245ef81`. Rewritten 2026-10-22 at `0f7eda5c` and at
`bc3bca1b`. **Rewritten again 2026-10-23 at `a4982f9f`** (main), after:
- fix wave 3 (PQMEM, DETPRIV and RERUNS, merged at `0a125a98`, `f812677e` and `4a62ed5b`;
  `cargo fmt` `09e55393`; docs `e4004b4c`);
- final runs part 2 (BENCH, merged at `430f40ee`; docs `a4982f9f`).

No Rust code changed after `09e55393`, so every test result of the wave 3 integration is a
result for HEAD's code.

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
- **matrix**, **stress**, **oracle-final**, **final-bench**, **final-run**, **usfp**,
  **pq-profile**, **knobs**, **effects**, **edge**, **realism**, **ccsig-memory**,
  **measure**, **rerun**: the reports in `testing/` (`rerun` is `rerun-2026-10-22.md`).
- **bench §n**: `docs/benchmark/scms-real-benchmark.md`.
- **BS wave 2**, **BS final**, **BS wave 3** and **BS final 2**: the BUILD-STATUS sections
  "SCMS realism fix wave 2 integration", "SCMS realism final test wave integration" (both
  2026-10-22), "SCMS realism fix wave 3 integration" and "SCMS realism final runs part 2
  integration" (both 2026-10-23).
- **04 §n** and **RT Rn**: `04-validation-report.md` and `docs/redteam/pki-privacy.md`.

The checklist files keep the verdicts their validators recorded on 2026-10-18. The only
exceptions are proto A16, D18 and I10, which were updated in `898c077a`. **Where a verdict
here differs from its checklist, this page is the later one.** §4 lists every such change
with its evidence.

## 1. What was checked, and how

Four validators each took one part of the SCMS. Each one listed every rule that the real US
system follows in that part, from primary sources:
- the IEEE 1609.2 / 1609.2.1 ASN.1 modules;
- the SCMS Manager documents (1609.2.1 Profile, Provider Requirements, Elector Policy and
  Technical Specification, EAC Requirements, re-enrolment specification, misbehaviour report
  specification);
- CAMP's EE Requirements 1.2.2;
- the USDOT primer, the USDOT DSRC RSU Specification v4.1 and the CV-pilot reports;
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
It was 264 (81.0 %) on 2026-10-18, 270 (82.8 %) at `0f7eda5c` and 275 at `bc3bca1b`.

**What moved since `bc3bca1b`: no rule verdict.** Fix wave 3 fixed runtime defects that have
no checklist row (§4.2). No rule was reopened by it or by final runs part 2. One row could
move on an owner reading: veh M13 (do RSUs report misbehaviour?) is still counted
unverifiable, although the engine now follows the reading that the US profile has no roadside
reporter (§8 item 6). If the owner accepts that reading, the match is 276 (84.7 %).

Of the 51 rules that do not match:
- 29 cannot be checked against any public source (§7);
- 22 are known differences (§5). 14 of them change nothing a device trusts or does.

The runtime defects that testing found and that have no checklist row are in §6. R-CCSIG,
the largest open item in the previous version of this page, is fixed. The largest still open
are the two fast-forward defects that the full-scale benchmark found (R-FFHOLD and
R-FFBURST), the honest reports at full density, and the full-scale memory.

### 2.2 Tests across all configurations

**Configuration tables.** In each table the expectations were written before the runs, and
each run was compared with its base on the same seed. The counts are the verdicts recorded
in each table's expectations file.

| Table | Rows | Pass | Fixed (bug, then pass) | Expectation wrong, corrected | Open |
|---|---|---|---|---|---|
| Matrix: 34 runs (32 one-change variants of the Midtown realism base, plus `gnss-raw` and `us-profile`), re-run on `77cffb5b` (`scms-matrix.yaml`) | 99 | 81 | 7 | 10 | 1 |
| Realism rows on the matrix runs (`runtime_expectations.yaml`; re-run on `77cffb5b`) | 62 | 54 | 0 | 6 | 2 |
| Settings knobs, same-seed pairs (knobs) | 21 | 15 | 3 | 2 | 1 |
| Effects table (effects) | 88 | 77 | 7 | 4 | 0 |
| Edge cases (edge; 3d fixed in 07 §7; 11 measured by matrix and stress) | 12 | 7 | 5 | 0 | 0 |
| Runtime realism, base (realism) | 27 | 22 | 5 | 0 | 0 |
| Runtime realism, changes (realism) | 8 | 6 | 2 | 0 | 0 |
| Stress: multi-week, post-quantum, first full-fleet seconds; R-NOCRL rows re-run on `77cffb5b` (`scms-stress.yaml`) | 35 | 17 | 1 | 2 | 15 |
| Final bench: full fleet, hybrid ML-DSA-44, certificate cycle, determinism (`scms-final-bench.yaml`) | 28 | 15 | 0 | 3 | 10 |
| Final run: certificate cycle after R-CCSIG, full-scale benchmark (`scms-final-run.yaml`) | 27 | 16 | 2 | 2 | 7 |
| Oracle-final on `88e24b09`: expectation rows, conf and mbr runs (oracle-final §8.1) | 60 | 60 | 0 | 0 | 0 |
| Oracle-final: `88e24b09` against `4924a5d2` and base against variant (oracle-final §8.2) | 10 | 9 | 0 | 1 | 0 |
| **All** | **477** | **379** | **32** | **30** | **36** |

The realism file records no per-row "fixed" verdict. The rows that the R-USFP fix turned
green are counted as pass, and the 6 rows corrected in the re-run (Z1, Z3, Z4, A3,
V8-us-catches-constpos and V8-us-checks-every-bsm, rerun §6) as corrected. Each corrected
row was shown to fail with a value outside its new band injected.

**What the 36 open rows are.** Most are not engine problems:
- **22 are superseded by a later table, or carried into one.**
  - Stress, 12: the 10 full-fleet rows (`FS.*`) were never run, and the final bench and final
    run measured them as `FB.*` and `FB2.*`. `PQ.certcycle` found R-CCSIG, which is now fixed
    (`PQHFC.sizes` and `PQHMC.sizes`, final-run). `PQ.determinism` passes in the final bench.
  - Final bench, 10. Six are closed by the final run: `FB.devices` and
    `FB.topups-at-rollover` pass as `FB2.*`, and `PQHMC.*` ×3 and `PQHFC.ccsig` are fixed
    (R-CCSIG). Four are carried into final-run rows that are still open: `FB.ff-ticks`,
    `FB.cost`, the honest false positives and the privacy link claims.
- **1 is open by design:** the knobs row for the request shuffle, published as partial,
  because the shuffle has no in-run effect (knobs). Its test passes (`scms_knobs_runs` 18/18).
- **13 are open items.**
  - Final run, 7: the full-scale benchmark did not finish (`FB2.completes`, `FB2.cost`,
    `FB2.ff-ticks`, `FB2.crl`, `FB2.backend-load`), and two of its findings
    (`FB2.honest-reports`, `FB2.privacy`). §6 items 1–5.
  - Stress, 3: `MWC.recovery` (R-JOINCRL, §6 item 7), `MWP.off-no-backend` (reopened, §6
    item 8) and `PQ.backend-bytes` (the per-certificate factor was not isolated; final-run
    §2 shows the bytes are reports, not provisioning).
  - Matrix, 1: `lte-latency-budget`, outside the SCMS (§6 item 16).
  - Realism, 2: L5, the case-board review time (§6 item 15), and T5, the US component set,
    which carries `status: open` in its file.

**Two notes on the table:**
- **The matrix and realism re-runs used the release binary of `77cffb5b`**, before fix wave
  3. Wave 3 changed the GNSS propagation, stopped RSUs filing their own reports and recorded
  the pre-run CRL fetch, so their report counts are owed again (§9).
- **Not in the table:** the policy package's before/after runs (07 §8), and usfp's same-seed
  before/after table (§3). Neither is a row. On `edge/clock-skew-ahead`, honest vehicles
  revoked fell from 19 to 0. On the realism base under the US profile, honest vehicles
  revoked fell from 26 to 0.

**Independent checks**

| Check | Result |
|---|---|
| Oracle self-test (official vectors), HEAD | 412/412 |
| Oracle mutation run, HEAD | 24 faults, all red, 0 silent |
| Oracle `verify-emitted`, HEAD | 10/10 |
| Oracle `records` on two recorded runs of `88e24b09` (conf and mbr) | **279,236 of 279,236 checks pass** (conf 67,451, mbr 211,785). On `4924a5d2` it was 272,778 of 281,240, every failure OF-1 or OF-2. **Not re-run on wave 3 code**, which adds the pre-run CRL PDUs and `node.security.crls` to the records |
| Record faults injected into those runs | 69 of 69 applicable faults red (conf 34, 1 not applicable; mbr 35), 0 silent, after oracle fix 2 (oracle-final §8.5) |
| `component_certs.py`: every component certificate of the M0 run against the published chain of its form (rerun §8) | **10/10 roles** match field for field (ISS pre-production chain for provider-a, SAESOL production chain for provider-b); every SPDU re-encodes byte for byte; `--mutate` 2 of 2 roles red |
| `ctl-files` on the engine's own CTLs (F1 `gov.json`) | PASS: CTL-FILE 16, CTL-ORDER 4, CTL-QUORUM 6, CTL-SIG 140; 6/6 faults red after oracle fix 1 |
| `ctl-files` on the six CTLs SCMS Manager publishes | 30/30 elector signatures valid; 5/5 faults red |
| `check_run.py` on the F1 conformance capture of `bc3bca1b`'s code | 18,910/18,910, 0 disagreements; `--mutate` 18/18 faults red |
| `oracle_final.rs` reproductions (OF-1, OF-2) and `conformance.rs` | 2/2 and 2/2 |
| `check_run_crl.py` on a newly generated CRL corpus (`2004bb87`) | 19/19 |
| Published artefacts; `real_artefacts`; `profiles.py`; PDU checkers; conformance run (`8245ef81`, not re-run since) | 164/164; 7/7; 24/24; 24/24, 25/25, 18/18, 4/4; 19,183/19,183 |
| Fault proofs of fix wave 3 | PQMEM F1–F5 red on main (BS wave 3); DETPRIV 6 of 6 injected faults red (measure §6); RERUNS: 3 sensitivity faults, the M-RX1 filter, the 5 s CRL poll and 4 realism size/count injections red (rerun §1–§3, §6) |

**Rust, UI and build checks**

| Suite | Result |
|---|---|
| HEAD's code (`09e55393`, run at the wave 3 integration): v2xw-sec, msg, proto, node, threat (`calibrate`, `in_the_loop`, `legacy_engine_compare` included), metrics, mobility | 1,830 pass, 5 fail, 3 ignored. The 5 are the known mobility traffic tests (calibration ×2, `pedestrian_invariants` ×2, kerbside `realism`). None uses GNSS, and their values are unchanged |
| HEAD's code: engine lib plus 29 SCMS, phase2 and sensitivity binaries, `scms_knobs_runs` 18/18 included | 284 pass, **2 fail**, 7 ignored. The 2 failures need an owner decision (§6 item 9) |
| HEAD `a4982f9f` (BS final 2): sec, msg, proto, node; engine lib plus 17 SCMS, phase2, PQ and sensitivity binaries | 1,015 pass, 0 fail, 1 ignored; 230 pass, the same 2 fail, 7 ignored |
| The 6 sensitivity failures of `bc3bca1b` | Root-caused and re-stated (rerun §1–§2): `sensitivity_radio` 27/27 (the real 80 B certificate changed frame sizes), `sensitivity_security` 20/20 (a 5 s CRL poll refuses the next pseudonym before it is reported, so nothing is left to link). CLI rows 28/28, 13/13, 3/3, 24/24 |
| `fragmentation::a_padded_message_…` (M-RX1) | Passes. M-RX1 blamed an SDU's fate on a fragment the PHY lost after it; the engine was right (rerun §3) |
| Regression digests | `phase2-path` and `i-s1-phase2-60s` re-blessed for wave 3's record changes; the phase2/scms_scale digests are unchanged after the merge |
| v2xw-server and v2xw-cli (`2004bb87`, not re-run since) | Pass except `longrun_budget`: 24.9 MB against a 24 MB budget. It already failed before (24.8 MB) |
| `timeline::a_param_change_of_the_arrival_rate_takes_effect_at_its_instant` | Fails (5.29×, band 2.5–5.0). It is traffic-only, has no SCMS, and predates this round |
| UI (`88e24b09`) | protocol and viewer `tsc -b` pass; Studio typecheck passes; Studio 326/326 unit and 32/32 e2e. No `ui/` change since, but the Studio fixtures that contain `node.security` or the new report fields may need re-recording |
| Release build and boot (`a4982f9f`) | Release build OK (6 m 57 s); the simulator boots on port 8758, and `/agent/status` and `/rpc/schema` both answer 200 |

## 3. What is confirmed working

Each item below was confirmed by an independent check or by every configuration that tested it.

**1. Formats and cryptography.** Wherever a public test vector or real artefact exists, the
engine reproduces it byte for byte:
- the COER encoding, re-encoding every published file (crypto #1);
- HashedIds and Time32 with leap seconds (#5–8);
- ECDSA P-256/P-384, ECIES, AES-CCM, KDF2 and MAC1 (#52–61);
- butterfly expansion and linkage values (#63–70);
- the six published CTLs (#78).

On `88e24b09` the oracle re-encoded all 54,446 recorded PDUs canonically (DECODE: 16,131 on
conf, 38,315 on mbr) and verified every signature with its own ECDSA.

**2. Certificate profiles per provider, now checked against the published chains.**
- provider-a takes ISS's published form: a 70-year root, the root as CRACA, the CRL signer
  root-issued on series 256, and the region on sub-CAs.
- provider-b takes SAESOL's production form.
- `component_certs.py` decodes every component certificate from a run's provisioning SPDUs
  with the oracle's codec and compares it with the published certificate of its role: 10 of
  10 roles have the same fields and the same encoded size of every field but the name (rerun
  §8).
- The oracle's CRACA-CERT and CRL-SSP rules pass on both forms.
- A governance CRL can revoke a governance sub-CA.

**3. Enrolment (fixed in wave 2, OF-1).** Each device has one enrolment certificate. Its own
ECA exchange issues it, with a back-dated start and the full 6-year lifetime; the DCM
registers it; and it signs every Download-Req. On `88e24b09`: DL-SIGN 2,892 and 10,898 checks
pass, ENR-LIFE 58 and 99 pass, and 0 devices hold a foreign certificate (oracle-final §8.1,
§8.2). The F1 capture has 62 EeRaCertRequests, against 118 when each device had two.

**4. SCMS architecture, in all 34 matrix runs.**
- No separation-of-duties violation, including under the CAMP topology with its LOP.
- Sealed messages are opened only by their recipient.
- A device talks only to its RA and the DCM.
- One provisioning request per OBU over its life (0.54 requests/s scaled to 17 M vehicles,
  against CAMP's 0.5). At full fleet: 40.0 certificates for each device's request, and 1,933
  devices provisioned in 240 s, 48.8 % by the home provider (final-run FB2.devices,
  FB2.provisioning).
- 20 certificates per device at the weekly top-off, and exactly ×3 under the NYC 60-a-week
  profile.
- **The rollover top-up at full fleet:** the pseudonym pool goes from 20 to 39.8 within 10 s
  of the 43,200 s i-period boundary; the home provider issued 12,580 certificates (629
  devices × 20) in those 10 s (FB2.topups-at-rollover).
- I-S1: real and modelled cryptography give identical records.

**5. Message sizes.** provider-b matches CAMP Table 3 exactly: response 454/470, ack 292/294,
ECA response 958. provider-a's sizes (501/517, 304/306, 1,064) are now explained to the byte
by the published ISS certificates: a 12 B `identifiedRegion` on the ICA and every component,
and a 35 B `encryptionKey` on the PCA and the ECA (rerun §8). CRL sizes are 328 B with one
entry and about 33 B for each further entry.

**6. Trust list (fixed in wave 2, R-TRUSTLEAK).** The two kinds of frame the engine sizes
instead of encoding (the RSU's CRL broadcast and a report relayed over the air) carry their
signer's certificate. Every receiver holds them to its installed CTL roots. With provider-b
off the trust list, provider-a receivers verify 0 of its messages (280 before; 0 again in the
`77cffb5b` re-run, realism V6). At full fleet, 2,019,081 SPDUs were checked with 0 refused
(final-bench FB.trust), and the second full-scale run shows no signature, PSID, CTL or CRL
loss cause (FB2.trust). The engine's own CTL bytes pass the oracle's `ctl-files`, with a
3-of-5 quorum (oracle-final §8.4).

**7. Revocation path on the real calendar.** Every case went through the steps the deployment
prescribes:
- the shuffle (at most one day);
- the 8 h review;
- the next business-day 10:00 ET slot, with federal holidays skipped (the Thanksgiving week
  passes);
- the first fetch after publication.

The oracle confirms the CRL rules independently on `88e24b09`: CRL-SEQ, CRL-KEEP, CRL-SLA,
CRL-PUB, CRL-MATCH and RA-BLOCK. Every device the oracle finds on a CRL is an attacker in the
ground truth: 4 of 4 decided on conf, 12 of 12 on mbr.

**8. CRLs from t0, full lists only (fixed in wave 2, R-NOCRL and O-1), and now on the
record (OF-3, fixed in wave 3).**
- Every CRACA signs an empty full CRL at t0 and renews it by `nextCrl`, as a deployment
  running since 2015 does. On conf: `F614/0` at t0, then `F615/0` with the revocations.
- Full CRLs only, with no gap in `indexWithinI`: on mbr 614/0, 615/0, 615/1, 615/2, 616/0,
  each `nextCrl` exactly 7 days on.
- **The records now show what a device holds** (measure §3): each `node.security` row lists
  the signed CRLs the node's receiver verified and holds (craca, series, counter, issue date,
  `nextCrl`), and `pki.flow` carries each device's pre-run CRL download, marked `preload`.
  The engine was right before; `crl_version` counted entries, which is 0 for an empty list.
- **The 2-week overdue rule acts from t0 at runtime.** In the multi-week run with the CRL
  Generator down, the home list issued at t0 goes overdue at exactly t0 + 21 d = 1,814,400 s:
  0 refusals before it, then 45 % of W3's last minute and 48 % of W4 refused, the home
  provider's share (stress `MWC.overdue-onset`, fixed; rerun §7).
- At full fleet, 2,000 receiver installs of the t0 CRLs (2 per device) and 60 RSU installs
  (FB.crl).

**9. CRL freshness over five weeks** (stress multi-week, on `2004bb87`).
- Five i-period rollovers, each signing under the right period.
- 10 CRL versions in 33 days, reissued by `nextCrl`.
- 12,592 conditional polls answered 412, and 0 refusals.
- With the CRL Generator down after its first CRL, every message under that CRACA was
  refused once the list became overdue (87,823 refusals), and traffic recovered after the
  Generator came back. A parked car that returns after four weeks refuses one message at
  power-on and then none (`MWP.overdue-on-return`, pass on `77cffb5b`).
- Re-enrolment: 1 of 75 devices, the expected 1.1.

**10. Downloads and report uploads (OF-2 fixed in wave 2; R-UNSEC fixed in wave 3).**
- Every download rule passes the oracle on both `88e24b09` runs: DL-ACK, DL-ENC, DL-SIGN,
  DL-TIME, DL-NAME, DL-AFTER and DL-WAIT. Conditional GET with 412 is in force, and so is
  `Range` resume.
- Every cellular report upload carries its SPDU. MBR-UP passes 3,930 and 18,618 checks; the
  1,695 uploads with no bytes are gone. A detection that cannot be built as an SM-MBR v1.1
  upload is not filed (`reports_not_filed`).
- **A roadside unit's own detection is not filed under the US profile** (measure §4):
  [SM-MBR] §3 signs a report with the end-entity's pseudonym certificate, its evidence is
  "a BSM from the sender and a BSM from the receiver", and the USDOT RSU Specification v4.1
  gives a unit no detection or reporting function. `reports_from_rsus` is 0 on P-256, and
  the sized-only backhaul reports (583 on conf, 1,114 on mbr) are gone. A unit still relays
  vehicles' reports. `modelled_reports: 1` keeps the old path for research. Under PQ this
  is still wrong (§6 item 10).
- **A report carries the BSMs its reporter judged** ([SM-MBR] §2.3.1.2.1, §2.3.1.3.1): the
  US profile's consecutive pair, matched by generation time. Before, it attached the
  accused's last two BSMs on the air when the report was built.

**11. Retries and outages.**
- The EE retries after 10 s, at most 10 times an hour. With an RA outage across the top-ups,
  12 of 12 complete.
- A message between components waits for a component that is down, and the component's timers
  run when it restarts.
- A device that is switched off reaches no part of the SCMS (F-OFF), with one exception at a
  fast-forward boundary that is not root-caused (§6 item 8).

**12. Receiver rules.**
- 1609.2 receiver validation rule by rule (veh R2–R9, R12–R18), and the CTL chain (R1).
- The device checks the CTL's elector quorum.
- A composite CRL is kept when one of its CRLs is unverifiable.
- Uncompressed explicit keys are accepted.
- The `generationTime` relevance window is enforced, and a handset now applies it when it
  takes a frame, not when the frame arrived (measure §5). That was the cause of the 43,125 s
  old evidence: frames that waited in a handset's inbox across a fast-forward were judged
  12 h later.
- One car with a 5,000 s clock error no longer gets anyone honest revoked (19 → 0).

**13. The US misbehaviour profile at quarter density (R-USFP fixed in wave 2, R-FS40 in
wave 3).**
- **R-USFP.** The raw 10 Hz GNSS fixes carried fresh 1.6 m white noise each. The US presets'
  receivers now output a smoothed navigation solution (τ 100 s) with a 0.3 m/s static hold.
  On the realism base, same seed (usfp §3): honest vehicles revoked 26 → 0; attackers revoked
  1 → 2 of 3; reports at the MA 7,488 → 235. `gnss-raw` reproduces the old run to the report.
- **R-FS40** (measure §1). The receiver carried its solution across an epoch by the later
  velocity alone, while the OBU dead-reckons each BSM to its own phase; near a stop two
  consecutive BSMs could then coincide, which ConstantPosition rightly flags. The solution
  now moves by the mean of the two measured velocities (Hatch 1982's carrier-phase delta,
  `gnss/error/gauss-markov` 1.2.0). Monte Carlo of stop-and-go creep: 509 frozen pairs in
  396,000 before, 4 after. At quarter density `constantPositionFrozen` fell 28 → 0 and
  reports 54 → 15 (final-run §2).
- In all 34 matrix runs on `77cffb5b` no honest device is revoked, except `gnss-raw` (26, the
  fault by construction). att10 revokes 4 of 9 attackers and sybil 2; every vehicle attacker
  heard by at least 3 reporters is revoked.
- The ConstantPosition check is as SM-MBR v1.0 §2.3.1.2.3 specifies, and the static hold
  cannot trigger it (measure §1).

**14. Metrics on the realism base.** All are inside their sourced bands:
- PDR 0.364;
- BSM latency p95 14.6 ms (budget 100 ms);
- CBR p95 1.4 %;
- AoI 82 ms;
- security at 46 % of BSM bytes;
- full-certificate share 17 % (re-measures veh S6);
- certificate pool mean 25.7;
- no signature, parse or backend failures, and no starvation.

**15. Full fleet, both live windows** (bench §00, final-run §4). The 0–240 s window and the
43,170–43,230 s rollover window ran in full:
- PDR within 100 m 0.833–0.843, and 0.846–0.853 in the rollover window; the quarter fleet
  gives 0.838. The 130–150 s windows are identical to the first full-scale run's;
- BSM latency mean 12.39–12.46 ms, p99 14.94–15.07 ms;
- CBR mean 0.073–0.087 and p95 0.13–0.20, below J2945/1's 0.5;
- 240–268 verifications per node per second, verification queue p99 0;
- 20 valid pseudonyms at every census until the boundary, 39.8 after it, and 0 starved.

**16. Determinism.** Record digests are identical at 1 and 4 threads. That holds on the 33-day
run, on the first 5 s of the full fleet, and on the hybrid ML-DSA-44 run, where
`metrics.json` and `run-report.json` are byte-identical too. The check can fail: three runs
that differ in one setting each give three digests, and so do PQ0, PQHFC and PQHMC
(`099bed7a…`, `a73101dd…`, `b33458e9…`, final-run PQ.digests).

**17. Post-quantum at density** (quarter fleet). These are modelled sizes and costs, since no
standard defines PQ encodings (pq-profile).
- **The Partially-Hybrid certificate cycle (R-CCSIG fixed in wave 3).** A node's own
  messages are signed, sized and priced with ECDSA, as Twardokus et al. (NDSS 2024) design
  it. A receiver pays the PQ check once for each certificate it learns
  (`cert_pq_verifications`). The validator refuses the cycle with a purely PQ scheme
  (ccsig-memory §1). Measured on current code (final-run §2):

  | | ECDSA | Falcon-512 cycle | ML-DSA-44 cycle | ML-DSA-44 on every SPDU |
  |---|---|---|---|---|
  | BSM PSDU mean | 207 B | 380 B (was 1,045) | 814 B (was nothing sent) | 1,332 B, 2.30 frames per BSM |
  | BSM airtime, 60 s | 13.6 s | 23.2 s | 47.5 s | 176.2 s |
  | Verified messages | 380,664 | −2.1 % | −5.7 % | −33.8 % |
  | BSM e2e mean | 3.72 ms | 3.96 ms | 4.57 ms | 12.06 ms |

- Hybrid ML-DSA-44 on every SPDU: CBR 0.125. Hybrid Falcon-512 on every SPDU: airtime ×4.4.
- **RSUs learn the CRL under PQ** (R-PQRSUCRL, fixed in wave 3): 60 RSU installs in both
  cycle runs (0 before).
- R-PQMBR is fixed. An over-MTU frame is refused and counted as refused, not as uploaded.
  Under `generic-sdu` it is fragmented.
- Hybrid top-ups complete after the RA's request shuffle. Nobody starves.

**18. Speed.** One check redid a P-256 multiplication for every pseudonym at every backend
step. With a memo, the realism base went from 2,074 s to 164 s with identical results
(31–41 s on `77cffb5b`). `pq-base` went from 1,175 to 125 CPU-s.

## 4. Verdicts changed in this round

### 4.1 Fixed (16 rules since 2026-10-18)

No rule verdict changed in fix wave 3 or final runs part 2.

**Fixed in fix wave 2.** These are the five rules reopened at `0f7eda5c`.

| Row | Before | Now | Evidence |
|---|---|---|---|
| proto C6 (the RA serves only registered enrolment certificates) | OF-1: the RA answered Download-Reqs signed by an enrolment certificate it never registered (112 on conf) | One certificate: the ECA exchange's own, registered by the DCM, signs every Download-Req | `d7b64fae`, `89f54fe4`, `3471f2fc`; oracle DL-SIGN on `88e24b09` 2,892 / 10,898 pass (was 1,320 / 5,245 red); `oracle_final.rs::one_enrolment_certificate_…` passes, and its logic is red on the `4924a5d2` recording |
| proto C8 and gov E-1 (enrolment certificate valid 6 years) | OF-1: a second certificate lasting 0.9–5.5 years; age modelled as a short lifetime | Back-dated start, full 6-year duration, clamped to the ECA's start. A late joiner's pre-run exchange runs at that start (merge fix `3471f2fc`) | ENR-LIFE 58 / 99 pass on `88e24b09` (was 56 / 97 red); `enrolment::an_aged_enrolment_is_the_one_certificate_its_eca_exchange_issued` |
| proto F6 (each report is POSTed through the RA with its Download-Req) | OF-2: 1,695 of 2,991 cellular uploads on conf carried no SPDU | A detection that cannot be built as an SM-MBR v1.1 upload is not filed. Every upload carries its SPDU | `e8475281`; MBR-UP 3,930 / 18,618 pass (was 1,695 / 49 red); `oracle_final.rs::every_cellular_report_upload_carries_its_spdus`, red in Python on the `4924a5d2` recording |
| veh R1 (the signer chains to a root on the installed CTL) | R-TRUSTLEAK: 280 provider-b messages verified with provider-b off the CTL | Sized frames carry the signer certificate, and OBU, RSU and VRU receivers validate it to the installed roots | `89f54fe4`; off-ctl 280 → 0 (BS wave 2; 0 again on `77cffb5b`); `d18a1e80` proved red at the engine (45 unchecked) and at the RSU. No fault was injected into the OBU or VRU fallback |

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

**In fix wave 3** (BS wave 3):
- **R-CCSIG** (ccsig-memory §1): the Partially-Hybrid cycle signs a node's messages with
  ECDSA and checks the PQ signature once per certificate. It reverses `45f0942f` (owner
  decision applied). Faults F1 and F2 red.
- **R-PQRSUCRL** (ccsig-memory §3): RSUs fetch and install the CRL under PQ. Fault F3 red.
- **Per-node memory de-duplication** (ccsig-memory §2): shared ACA and pseudonym certificate
  `Arc`s, `checked_at` not stored under the US profile, and UsSmMbr evicts senders after
  30 s (ASSUMPTION). About 10–15 % of the live heap; results unchanged. Faults F4 and F5 red.
- **R-FS40** (measure §1): the smoothing receiver propagates by the mean velocity (§3
  item 13).
- **The privacy run report** (measure §2): `privacy_links_claimed` counts only claims across
  a pseudonym change; new `privacy_changes_heard` and `privacy_links_spurious`. The
  `linkability` metric was already right.
- **OF-3** (measure §3): held CRLs and the pre-run CRL fetch are recorded (§3 item 8).
- **R-UNSEC** (measure §4): an RSU's own reports are not filed under the US profile (§3
  item 10).
- **The old evidence** (measure §5): a handset applies the relevance window at processing
  time, and a report carries the BSMs its reporter judged.
- **M-RX1** (rerun §3): the metric no longer blames an SDU's fate on a fragment lost after
  it. The engine was right; the check was wrong.
- **Six sensitivity rows re-stated** on their measured causes (rerun §1–§2).

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
| veh M11 | Relayed report uploads go on safety channel 172, not on a service channel advertised by WSA | Report airtime on 172 is an upper bound (0.21 % at full fleet in the first run). The radio has no per-channel MAC queues and no channel switch (07 §7 item 4) |
| veh U5 | No WSA encoder | The SCMS service cannot be advertised, so M11 cannot be fixed yet |

Fourteen of these change nothing a device trusts or does: crypto #30, #35 and proto E7, crypto
#38 and veh R11, crypto #10 and gov K-5 (2 s), and gov G-2, G-11, C-9, C-10, C-11, C-14 and
L-11. The other eight are proto A16 and H10, gov G-10 and L-8, and veh R10, C6, M11 and U5.
Each of them matters only to long runs, to bytes on a poll, or to where report airtime is
counted.

## 6. Runtime defects and findings with no checklist row

In order of impact. Each has its evidence in the named report. Items fixed in this round are
in §4.2. None of the open items below changes a rule verdict, but items 1–3 decide whether
any two-day misbehaviour number is a real-world number.

1. **R-FFHOLD: reports are held in flight across a fast-forward** (final-run §4 item 1).
   Open, engine defect.
   - At full fleet the census `p2.in_flight` stayed at 1,258 from 240 s to 43,170 s, and the
     MAs had 0 reports. They received the first window's reports only at the 43,200 s
     i-period boundary.
   - The fast-forward's next wake apparently does not include in-flight report deliveries.
   - **Impact:** report → case → decision timing is 12 h late for anything reported before a
     fast-forward. Whether the quarter-fleet runs, which also cross a fast-forward, carry
     the same delay has not been checked.
2. **R-FFBURST: an honest-report burst on resuming after a fast-forward** (final-run §4
   item 2). Open, cause unconfirmed.
   - 4,876 reports in the 10 s after the 43,170 s resume, against about 80 per 10 s live.
   - The hypothesis is that the GNSS model or the vehicles' kinematics step across the gap as
     one epoch. The node count also fell from 1,833 to 1,281 at the resume.
   - With item 1 this gave 15,062 reports at the MAs in 20 s and **71 cases about honest
     vehicles** (3.3 % of vehicles). None was decided before the run was stopped.
3. **Honest reports at full density** (FB2.honest-reports). Open.
   - The first live window filed 1,935 reports in 240 s (8.1 a second), all about honest
     vehicles. The first full-scale run filed 229 in 40 s (5.7 a second), mostly
     `constantPositionFrozen`.
   - The R-FS40 fix removes that cause at quarter density (28 → 0) and in Monte Carlo, but
     it is not shown to hold at full density. The detector breakdown is only in the run
     report, which the stopped run did not write. The 71 `positionJump` verdicts of the
     40 s window were not investigated, and density rises into gridlock (median speed 0 from
     160 s).
   - The 0.3 m/s hold, the 100 s smoothing constant and the velocity σ are ASSUMPTIONS on
     cited mechanisms.
   - **Impact:** no full-fleet false-positive rate is a real-world number until items 1–3
     are settled.
4. **The full-scale benchmark has not finished** (bench §00, final-run §4).
   - Both live windows ran in full, under a 3.5 GB cap, with a peak of 3,109 MB.
   - The second fast-forward runs at about 6.7 simulated seconds per wall second, one SCMS
     kernel step per simulated second (bench §9.2). The run was stopped at 47,340 of
     172,800 s, about 5.4 hours short, so it has no end-of-run report.
   - **Memory:** the wave 3 de-duplication removed 10–15 % of the live heap but does not show
     at full scale (2,690 MB at 150 s, as before). Allocator retention after transient peaks
     dominates; the 2.5 GB target is not met (ccsig-memory §2).
   - **Impact:** there is no two-day full-fleet result: no CRL, case or decision outcome
     (FB2.crl, FB2.backend-load), and no fast-forward tick count (FB2.ff-ticks).
5. **The rollover registers no pseudonym change** (FB2.privacy). `pseudonym_change_rate` is 0
   in every window, the 43,200 s boundary included, and linkability is 0 of 0, although the
   pool doubles there; the quarter fleet counted 314 changes at its rollover. The privacy
   observer's 2,873 link claims are explained and fixed (measure §2), but no privacy number
   at full fleet can be read until this is.
6. **Pedestrian wait counts the fast-forward** (final-run §4 item 3). `ped_wait` has a mean
   of 3,465 s in the window after the resume, the same class of defect as the AoI charge
   fixed in the matrix round.
7. **R-JOINCRL: a car that joins mid-run holds only the t0 CRLs** (rerun §7, §9 item 1).
   Late in a long run those lists are overdue, so the joiner refuses its home provider's
   messages until its first download (2–49 s): 755 refusals at 4 joiners in MWC's W5, and
   18,617 at 27 joiners in MWP. It should hold the newest list each CRACA published before
   it joined, as `preload_at` does for certificates.
8. **A switched-off car records CRL downloads** (`MWP.off-no-backend`, reopened; rerun §7).
   Node 20 is off from 645 s to 2,419,510 s, but `proto.revocation` records it downloading,
   processing and enforcing three CRLs at 604,740.2 s, the first live step after a
   fast-forward. Not root-caused.
9. **Two engine tests fail and need an owner decision** (BS wave 3). Neither was weakened.
   - `scms_wiring::detectors_file_real_us_reports_that_the_ma_opens_and_verifies`: "the MA
     opened no case". Its 9 real reports are filed, opened and verified, but once RSUs no
     longer file reports (R-UNSEC), vehicle reports alone do not reach the MA's quorum of 3
     independent reporters within 120 s. The test needs a longer run or a vehicle-only
     quorum.
   - `pq_relay::an_oversized_relayed_report_or_crl_frame_is_fragmented_and_acted_on_whole`:
     "no vehicle report reached the proxy through a unit". The GNSS fix moved which
     relay-only vehicle detects and where, so no forwarding RSU hears the 4 relayed reports.
     It fails on the detpriv tip alone, so the merge did not cause it. It needs a scenario
     change.
10. **R-PQUNSEC: under PQ an RSU's own reports are still filed** (final-run §3). PQHFC files
    7 and PQHMC 9. `Phase2::real_mbr` returns `Off` when `pki` is `None`, before the R-UNSEC
    roadside check. The fix is one line, but it may leave the `pq_relay` R-PQRSUCRL test's MA
    without a case, the same decision as item 9.
11. **Post-quantum path, modelled only** (pq-profile §3; final-run §3).
    - RSUs have no application-certificate cycle under PQ.
    - Receivers check CRL entries against the Generator's list, not a signed CRL.
    - Every top-up is a fresh request (there is no PQ butterfly).
    - VRU handsets always price ECDSA.
    - Both cycle runs show 47 `Invalid` verification states that P-256 does not, with no
      refusal counted; not root-caused (perhaps item 13's label).
    - No standard defines PQ encodings, so no PQ number is a real-world number.
12. **The enrolment certificate name** (rerun §8). Every enrolment certificate is named
    `ee-<device>.provider-a`, including the 46 that provider-b's ECA issues. The name is
    still unique per device, so crypto #28, proto C3 and gov E-3 hold, and the size is
    unchanged. Fixing it moves certificate bytes and the regression digests.
13. **An overdue-CRL refusal is labelled `signature-invalid` on `node.rx`** (rerun §9 item
    3). In MWC's W4 the run report counts 97,679 `crl-overdue` refusals and `node.rx` has
    exactly 97,679 `signature-invalid` fates. An analyst reading `node.rx` would see forgery
    where there is a stale list.
14. **The US profile cannot report a handset** (usfp §3). SM-MBR v1.0 defines reports for
    the BSM (PSID 0x20) only, so a PSM-sending attacker goes unreported. This is a gap of
    the real profile, which the engine reproduces faithfully. The research suite, with
    `modelled_reports: 1`, catches it: 3 of 3 revoked.
15. **Unexplained, not yet shown to be defects:**
    - **Fast-forward ticks.** 865,653 in the edge peer-root-removed run; related to item 4's
      slow second fast-forward.
    - **Case-board review time** (realism L5). The board shows 28,799.95 s on `77cffb5b`,
      against the expected ≥ 28,800 s. The review clock starts at report receipt and the
      case's corroborated stamp comes up to a second later.
    - **O-2.** Four Saturday decisions on mbr are late under the strict reading of [SM-PROV]
      3.4.10, which does not say which reading applies.
    - **The generic reassembler's 64-set buffer** fills in the dense padded fragmentation
      run and refuses new sets (11,607). This is a real effect of the fragmenter's default,
      not a defect (rerun §3).
16. **Outside the SCMS:**
    - LTE-V2X BSM p95 latency is 112 ms, over the 100 ms budget, because the BSM is not
      aligned with its SPS reservation (matrix `lte-latency-budget`).
    - Rebroadcasting a 336 kB standing CRL every 5 s on channel 172 costs about 0.54 Mbit/s per
      RSU (07 §7 item 5). This is a configured behaviour, not a defect.
17. **Red-team finding.** The `generationTime` window makes a replay older than 2 s harmless,
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
- the GNSS receiver's smoothing constant (100 s, after DO-229's carrier smoothing), static
  hold (0.3 m/s, after u-blox) and velocity σ;
- the US profile's sender eviction after 30 s without a message (at least 10× `max_gap_s`).

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
| veh S6 | The full-certificate share | Re-measured in steady state at 17 % (matrix) and 16.1–19.8 % at full fleet, inside the sourced band of 5–30 % |
| veh P7 | P2PCD concurrency limits | 8 responses, 8 inline entries |
| veh P8 | Which P2PCD flavour US devices run | Out-of-band |
| veh C10 | Whether J2945/1 clears the path history at a change | The code clears it (§8 item 4) |
| veh C11 | The pseudonym-change rule for J2945/9 handsets | The scenario's rule |
| veh M9 | The device report store quota | 64 KiB |
| veh M10 | The report rate limit | At most one per second per (reporter, accused) |
| veh M13 | Whether RSUs run misbehaviour detection | Under the US profile a unit's own detection is not filed, on the reading of [SM-MBR] §3 and the USDOT RSU Specification v4.1 (measure §4; §8 item 6). `modelled_reports: 1` files them sized-only for research. Under PQ they are still filed (§6 item 10) |
| veh V3 | The PSM rate | 1 Hz with a 100 ms floor |
| veh U3 | RSU verification policy | Verify-all |

## 8. Disagreements between the checklists

The totals count each row as its own checklist records it, with the changes in §4 applied.

1. **RSU SSPs: open.** crypto #35 and proto E7 record a deviation. Gov E-8 and F-3 read CAMP
   Table 1 in full and found no SPaT, MAP or TIM SSP in it. On that reading both rows match,
   and the deviations would be 20.
2. **Pseudonym region: open.** crypto #32 says unverifiable. Gov F-2 says "no region", from
   [SMP] §6.3.3. The published ISS and SAESOL component certificates carry regions; the
   pseudonym profile's does not (rerun §8).
3. **i-period time base: owner decision** (crypto #10 against gov K-5).
4. **Path history at a change: the checklist text is out of date.** veh C10 says the engine
   does not clear it. `vehicle.rs` `set_identity` does clear it.
5. **MA concurrency: settled.** The MA ran one case at a time; it now runs cases side by side
   (R-SERIAL, 07 §5 bug 1). The "27 decisions in every variant" were the R-USFP false
   positives, and are gone (usfp §4; att10 now 4 decisions).
6. **RSU reporting: open.** veh M13 says unverifiable: no source states whether deployed RSUs
   run detection. The DETPRIV package read [SM-MBR] §3 (a report is signed with the
   end-entity certificate, a pseudonym in CAMP's design), §2.3.1.4.1 (the reporter is itself
   a BSM sender) and the USDOT RSU Specification v4.1 (no detection or reporting function)
   as "the US profile has no roadside reporter". If the owner accepts that reading, M13
   becomes a match and the total 276 of 326 (84.7 %).

## 9. What remains

**Fix first (engine defects, in order of impact)**
1. **R-FFHOLD**: include in-flight report deliveries in the fast-forward's next wake. Then
   check whether the quarter-fleet MA timings carry the same delay.
2. **R-FFBURST**: take the detector breakdown of the 10 s after a resume, and check how the
   GNSS model and the kinematics cross the gap.
3. **The slow second fast-forward** (bench §9.2), then run the full-scale file again with
   `--checkpoint-every 1h`.
4. **Full-scale memory**: reuse per-step buffers, keep latency windows as a sketch, drop RA
   batch files after a confirmed download, and share `Arc<HardwareProfile>` (ccsig-memory
   §2).
5. **The honest reports at full density**: the detector breakdown, and the 71 `positionJump`
   verdicts.
6. **The rollover's pseudonym change count** and **`ped_wait` across a fast-forward**.
7. **R-JOINCRL**: a joiner holds the newest list each CRACA published before it joined.
8. **MWP.off-no-backend**, the `node.rx` `crl-overdue` label, the 47 `Invalid` states under
   the cycle, and VRU handsets' PQ pricing.
9. **The enrolment certificate name** (moves the digests).

**Owner decisions**
- The two failing tests (`scms_wiring`, `pq_relay`) and R-PQUNSEC, which depend on each other
  (§6 items 9–10).
- The RSU-reporting reading (§8 item 6).
- CF-IPERIOD (2 s).
- The US J2945/1 window values.
- Whether SCMS Manager's artefacts may be vendored.
- The US profile stays the default of the US presets. It revokes no honest vehicle on any
  quarter-fleet run. It misses handset attackers, as the real profile does (§6 item 14).

Decided in this round: R-CCSIG's reversal of `45f0942f` was applied, and `target/agents` was
deleted.

**Engine work for the remaining deviations**
- The `infoStatus` body (A16).
- Per-channel MAC queues, a service channel and WSA bytes (M11, U5).
- Country polygons (R10).
- Governance scenario keys (G-10, C-9, C-10).
- Root termination (L-8).
- Stop issuing at 255 CRLs a week (H10).
- The post-quantum RSU certificate cycle.
- SPS-aligned BSM generation for LTE-V2X.

**Testing still owed**
- **The full-scale benchmark to the end**, after fix-first items 1–4.
- **Re-runs on wave 3 code.** The matrix and realism re-runs used `77cffb5b`; wave 3 changed
  the GNSS propagation, the RSU filing and the records. Also owed: the R-FS40 window (expected
  `constantPositionFrozen` near 0 and 2,873 spurious claims) and `gnss-raw` (expected 0
  implausible evidence).
- **The oracle on current records**: a fresh conf and mbr capture through `oracle.py
  records`, and a new rule that every device holds each CRACA's current list from t0 (OF-3).
  The fault "skip `note_prerun_crl`" was not injected.
- **The conformance run of `8245ef81`.**
- **UI**: re-record the Studio fixtures if they contain `node.security` or the new report
  fields, and re-run the Studio suites.
- **A fault proof for the OBU and VRU receiver fallback** (R-TRUSTLEAK). Only the engine and
  RSU paths were proved red.
- **F6/F7 fault proofs** (`inject2.py`), one test at a time.
- **Failing tests that predate this round:** `longrun_budget`, `timeline` and the 5 mobility
  traffic tests. v2xw-server and v2xw-cli were last run at `2004bb87`.

Done since the last version of this page: the ISS-profile check (10/10), the 6 sensitivity
failures and M-RX1, the full `scms_knobs_runs` (18/18), the three threat suites (1/1, 8/8,
2/2), all 34 matrix runs and the 62 realism rows, the stress R-NOCRL rows, realism V8, the
PQHFC/PQHMC re-runs, and `matrix.md` and `stress.md` brought up to date.

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
that measure, 275 of 326 rules (84.4 %) match. Every rule the runtime oracle checks passed on
the last recorded runs (`88e24b09`): 279,236 of 279,236 checks.
- **What is left on the checklists:** 22 known deviations, 14 of which have no trust effect,
  and 29 rules no public source can settle.
- **What is left at runtime:** the two fast-forward defects (R-FFHOLD, R-FFBURST), the
  honest reports at full density, the full-scale memory and the unfinished two-day run, the
  rollover's pseudonym count, R-JOINCRL, and two failing tests that wait on an owner
  decision. The post-quantum certificate cycle (R-CCSIG) is fixed.
