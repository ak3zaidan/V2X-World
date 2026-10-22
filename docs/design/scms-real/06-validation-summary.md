# 06 — How close the engine's SCMS is to the real US deployment: validation summary

First written 2026-10-18 at `8245ef81`. **Rewritten 2026-10-22 at `0f7eda5c`** (main), after
the PKI, flows and policy fix wave (merged at `2004bb87`) and the oracle, stress and matrix
testing wave (merged at `0f7eda5c`). This page states the final state of the realism round:
the verdict on every real-world rule, the test totals across every configuration that ran,
what is confirmed working, and what still deviates or cannot be checked.

It is for the owner and for a reviewer from outside the project. It goes no further than its
sources. The sources are the four validation checklists, the testing reports and the merge
notes. Every claim names its row or report.

**How to read the row references:**
- **crypto #n**: `validation/crypto-formats.md` §1, row n.
- **proto Xn**: `validation/protocols.md`, item Xn.
- **gov X-n**: `validation/governance-deployment.md` §1, row X-n.
- **veh Xn**: `validation/vehicle-and-roadside.md` §1, row Xn.
- **07 §n**: `07-config-testing-report.md`.
- **matrix**, **stress**, **oracle-final**, **knobs**, **effects**, **edge**, **realism**: the
  reports of the same names in `testing/`.
- **04 §n**, **bench §n**, **RT Rn**: `04-validation-report.md`,
  `docs/benchmark/scms-real-benchmark.md` and `docs/redteam/pki-privacy.md`.

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
- the USDOT primer and the CV-pilot reports;
- the CTLs and certificate chains that SCMS Manager actually publishes.

Code comments were never used as a source. Each rule's evidence is one of four kinds:
- an official test vector;
- the independent Python oracle, which was written without reading the Rust;
- a byte-for-byte comparison with a published real artefact;
- a Rust test.

This round added a fifth kind: **runtime evidence**. The oracle checked the PDUs that real runs
recorded, and the configuration tables compared same-seed runs. Each expectation was committed
before the run it judges. Where runtime evidence contradicts a row that a static check had
passed, the row is reopened (§4.3).

Verdicts:
- **Matches.** The engine does what the source says, and the evidence is named.
- **Fixed.** The engine did not match. It was corrected in one of the two rounds, and the new
  test was shown to fail when the old code was put back.
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
| Protocols and backend flows | 109 | 87 | 9 | 6 | 7 |
| Governance, deployment and calendar | 70 | 44 | 9 | 11 | 6 |
| On-vehicle and roadside behaviour | 67 | 40 | 10 | 6 | 11 |
| **All** | **326** | **232 (71 %)** | **38 (12 %)** | **27 (8 %)** | **29 (9 %)** |

**Final match: 270 of 326 rules (82.8 %) do what the real system does** (matches plus fixed).
On 2026-10-18 it was 264 (81.0 %).

Since then, 11 rules were fixed and 5 were reopened:
- **11 rules fixed.** Ten were documented deviations: crypto #4 and #36; proto B6, D18, I7
  and I10; gov L-1; veh M2, M3 and M6. One was unverifiable: crypto #29.
- **5 rules reopened.** These are matches that runtime testing contradicted: proto C6, C8 and
  F6, gov E-1 and veh R1.

Without the reopenings, the figure would be 275 (84.4 %).

Of the 56 rules that do not match:
- 29 cannot be checked against any public source;
- 27 are known differences. Of these, 14 change nothing a device trusts or does (§5).

Six runtime defects found by testing have no checklist row. They are listed in §6. The
largest of them, R-USFP, affects every misbehaviour and revocation number of the US presets.

### 2.2 Tests across all configurations

**Configuration tables.** In each table the expectations were written before the runs, and
each run was compared with its base on the same seed.

| Table | Rows | Pass | Fixed (bug, then pass) | Expectation wrong, corrected | Open |
|---|---|---|---|---|---|
| Matrix: 32 runs, 31 one-change variants of the Midtown realism base (matrix) | 98 | 82 | 1 | 5 | 10 |
| Realism rows re-run on the matrix runs (matrix) | 62 | 49 | 0 | 1 | 12 |
| Settings knobs, same-seed pairs (knobs) | 21 | 15 | 3 | 2 | 1 |
| Effects table (effects) | 88 | 77 | 7 | 4 | 0 |
| Edge cases (edge; 3d fixed in 07 §7; 11 measured by matrix and stress) | 12 | 7 | 5 | 0 | 0 |
| Runtime realism, base (realism; R-LOP since fixed) | 27 | 22 | 5 | 0 | 0 |
| Runtime realism, changes (realism) | 8 | 6 | 1 | 0 | 1 |
| Stress: multi-week, post-quantum, full fleet (stress) | 24 | 14 | 1 | 1 | 8 |
| Oracle-final: expectation rows on two recorded runs (oracle-final) | 47 | 47 | 0 | 0 | 0 |
| Oracle-final: base against variant (oracle-final §5) | 7 | 6 | 0 | 1 | 0 |
| **All** | **394** | **325** | **23** | **14** | **32** |

**Two notes on the table:**
- **The knobs row** counts its 21 rows one by one. The 07 §2 summary gives 13 pass and 4 fixed
  for the same rows; the difference is only which column a row is counted in.
- **The policy package's before/after runs** (07 §8) are not rows, so they are not in the
  table. On `edge/clock-skew-ahead`, the honest vehicles revoked fell from 19 to 0. On the
  realism base and the replay attack, every effect was measured.

**Independent checks**

| Check | Result |
|---|---|
| Oracle self-test (official vectors) | 412/412 |
| Oracle mutation run | 24 faults, all red |
| Oracle `verify-emitted` | 10/10 |
| Oracle `records` on two recorded runs (conf and mbr, release binary of `4924a5d2`) | 272,778 of 281,240 checks pass. All 8,462 failures are OF-1 or OF-2 (§6) |
| Record faults injected into those runs | 67 of 67 applicable faults red (conf 32 + 3 not applicable; mbr 35) |
| `ctl-files` on the six CTLs SCMS Manager publishes | 30/30 elector signatures valid; 5/5 faults red |
| `check_run_crl.py` on a newly generated CRL corpus (`2004bb87`) | 19/19 |
| Published artefacts; `real_artefacts`; `profiles.py`; PDU checkers; conformance run (`8245ef81`, not re-run since) | 164/164; 7/7; 24/24; 24/24, 25/25, 18/18, 4/4; 19,183/19,183 |

**Rust test suites**

| Suite | Result |
|---|---|
| HEAD `0f7eda5c`: v2xw-sec, msg, proto, node, metrics | 964 pass, 0 fail |
| HEAD `0f7eda5c`: engine lib plus 19 SCMS test binaries | 187 pass, 0 fail, 5 ignored (the ignored tests include the two oracle-final reproductions) |
| `2004bb87`: sec, msg, proto, node, threat, metrics | 1,501 pass, 0 fail |
| `2004bb87`: engine lib plus 18 SCMS binaries, including the 37-minute `scms_knobs_runs` | 195 pass, 0 fail |
| Stress branch (from `2004bb87`, with F-OFF): `scms_knobs_runs` | 18/18 pass |
| Stress branch: `security_lifecycle` | 16/16 pass |
| v2xw-server and v2xw-cli | Pass except `longrun_budget`: 24.9 MB against a 24 MB budget. It already failed before the merges (24.8 MB) |
| `timeline::a_param_change_of_the_arrival_rate_takes_effect_at_its_instant` | Fails (5.29×, band 2.5–5.0). It is traffic-only, has no SCMS, and fails on the stress branch's base too |
| Release build and boot | Release build OK; the simulator boots on port 8758, and `/agent/status` and `/rpc/schema` both answer 200 |

## 3. What is confirmed working

Each item below was confirmed by an independent check or by every configuration that tested it.

**1. Formats and cryptography.** Wherever a public test vector or real artefact exists, the
engine reproduces it byte for byte:
- the COER encoding, re-encoding every published file (crypto #1);
- HashedIds and Time32 with leap seconds (#5–8);
- ECDSA P-256/P-384, ECIES, AES-CCM, KDF2 and MAC1 (#52–61);
- butterfly expansion and linkage values (#63–70);
- the six published CTLs (#78).

Oracle-final re-encoded all 54,422 recorded PDUs canonically (DECODE).

**2. Certificate profiles per provider (this round).**
- provider-a takes ISS's published form: a 70-year root, the root as CRACA, the CRL signer
  root-issued on series 256, and the region on sub-CAs.
- provider-b takes SAESOL's production form.
- The oracle's CRACA-CERT and CRL-SSP rules pass on both forms: 14,780 and 48 checks.
- A governance CRL can now revoke a governance sub-CA.

**3. SCMS architecture, in all 32 matrix runs.**
- No separation-of-duties violation, including under the CAMP topology with its LOP.
- Sealed messages are opened only by their recipient.
- A device talks only to its RA and the DCM.
- One provisioning request per OBU over its life (0.54 requests/s scaled to 17 M vehicles,
  against CAMP's 0.5).
- 20 certificates per device at the weekly top-off, and exactly ×3 under the NYC 60-a-week
  profile.
- I-S1: real and modelled cryptography give identical records.

**4. Message sizes.** provider-b matches CAMP Table 3 exactly: response 454/470, ack 292/294,
ECA response 958. CRL sizes are 328 B with one entry and about 33 B for each further entry.
provider-a sizes are under §6 item 7.

**5. Revocation path on the real calendar.** Every case went through the steps the deployment
prescribes:
- the shuffle (at most one day);
- the 8 h review;
- the next business-day 10:00 ET slot, with federal holidays skipped (the Thanksgiving week
  passes);
- the first fetch after publication.

The oracle confirmed the CRL rules independently: CRL-SEQ, CRL-KEEP, CRL-SLA, CRL-PUB,
CRL-MATCH and RA-BLOCK. Under the research suite and in the oracle runs, every revoked device
is an attacker in the ground truth (3/3 and 12/12).

**6. CRL freshness over five weeks** (stress multi-week).
- Five i-period rollovers, each signing under the right period.
- 10 CRL versions in 33 days, reissued by `nextCrl`.
- 12,592 conditional polls answered 412, and 0 refusals.
- The 2-week overdue rule acts in a run. With the CRL Generator down after its first CRL, every
  message under that CRACA was refused once the list became overdue (87,823 refusals), and
  traffic recovered after the Generator came back.
- Re-enrolment: 1 of 75 devices, the expected 1.1.

**7. Downloads.** Every rule on download requests passes the oracle on both runs, except
DL-SIGN (see OF-1). The passing rules are DL-ACK, DL-ENC, DL-TIME, DL-NAME, DL-AFTER and
DL-WAIT. Conditional GET with 412 is in force, and so is `Range` resume.

**8. Retries and outages.**
- The EE retries after 10 s, at most 10 times an hour. With an RA outage across the top-ups,
  12 of 12 complete.
- A message between components waits for a component that is down, and the component's timers
  run when it restarts.
- A device that is switched off reaches no part of the SCMS. This is F-OFF, fixed this round:
  before the fix, a parked car kept polling and topping up.

**9. Receiver rules.**
- 1609.2 receiver validation rule by rule (veh R2–R9, R12–R18).
- The device checks the CTL's elector quorum.
- A composite CRL is kept when one of its CRLs is unverifiable.
- Uncompressed explicit keys are accepted.
- The `generationTime` relevance window is enforced.
- With the MA's evidence-time rule and the dead-reckoning fix, one car with a 5,000 s clock
  error no longer gets anyone honest revoked (19 → 0).

**10. Metrics on the realism base.** All are inside their sourced bands:
- PDR 0.364;
- BSM latency p95 14.6 ms (budget 100 ms);
- CBR p95 1.4 %;
- AoI 82 ms, after this round's fast-forward fix (it was 28 minutes);
- security at 46 % of BSM bytes;
- full-certificate share 17 %. This re-measures veh S6, which had 25 % during the fill.
- certificate pool mean 25.7.
- no signature, parse or backend failures, and no starvation.

**11. Determinism.** Record digests are identical at 1 and 4 threads, on the 33-day run and on
the first 5 s of the full fleet (646 devices).

**12. Post-quantum at density** (owed until this round), on the quarter fleet:
- ML-DSA-44: 2.30 frames per BSM and BSM airtime ×12.6; CBR p95 rose from 0.061 to 0.338, and
  verified BSMs fell by 33 %.
- Hybrid Falcon-512: airtime ×4.4.
- Nobody starves.

## 4. Verdicts changed in this round

### 4.1 Fixed (11 rules)

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
| veh M3 | Research thresholds (12 m/s²; no MaxSpeed) | US profile: MaxSpeed 90 m/s, MaxAcceleration 10 m/s², RandomPosition 1 m | `c5d5c5b7` |
| veh M6 | Reports about messages that failed verification | The US profile judges only BSMs this receiver verified (SM-MBR §2.3.1) | `us_mbr.rs::only_verified_bsms_are_judged_or_remembered` |

Rules narrowed but not fixed:
- **proto A16.** The poll is now on the air. The `infoStatus` SPDU body is still not encoded.
- **R-TRUSTLEAK, OF-1, OF-2 and R-NOCRL** were found by this round's runs (§6).

### 4.2 Fixes that change runtime behaviour but no checklist row

These were each fixed and proved red, as their reports describe:
- **The MA ran one case at a time.** Its cases now run side by side (R-SERIAL, gap row 27).
- **The CRL Generator reissues by `nextCrl`** when no entry has been added.
- **Devices install a newer CTL.**
- **The node clock reaches security.**
- **No signing with an expired pseudonym across the rollover** (edge 1b/1c).
- **F-OFF.**
- **The AoI fast-forward charge** (matrix fix 2).
- **The pseudonym usability memo.** It removes a 13× slowdown, and its results are unchanged
  (matrix fix 1).
- **Skewed dead-reckoning** (07 §8.1).
- **Fast-forward stops**: 255,658 → 4,290 (R-FF).
- **The PKI-record defects** R-LOP, R-SHARE, R-CASE, R-CORR and R-OFFCTL, and the two merge
  fixes listed in 07 §5.

### 4.3 Reopened by runtime evidence (5 rules: Matches → Documented deviation)

| Row | Static verdict | Runtime evidence | Impact |
|---|---|---|---|
| proto C6 (the RA serves only registered enrolment certificates) | Matches (Rust test) | OF-1: the RA answers pre-drive Download-Reqs signed by an enrolment certificate it never registered. That is 112 on conf | Any defence keyed on the enrolment certificate is affected: RA blocklisting, and linking a report to a device |
| proto C8 and gov E-1 (enrolment certificate valid 6 years) | Matches (`model_card`) | OF-1: a second enrolment certificate, which no recorded exchange issued, lasts 0.9–5.5 years. Device age is modelled as a shorter lifetime, not as a back-dated start. ENR-LIFE fails on 56 of 114 certificates on conf and 97 of 196 on mbr | Wrong certificate bytes and expiry dates. Re-enrolment timing is per device, not per CAMP |
| proto F6 (each report is POSTed through the RA with its Download-Req) | Matches | OF-2: 1,695 of 2,991 cellular uploads on conf carry no SPDU. They are 1,119-byte placeholders, but they still go through the shuffle to the MA | Report counts at the MA are inflated. 1,536 of the 1,695 fall in the window after revocation |
| veh R1 (the signer chains to a root on the installed CTL) | Matches (Rust test) | R-TRUSTLEAK: with provider-b off the CTL, provider-a receivers verified 280 of its messages. Before the fix wave the figure was 7. Not root-caused | Security: a provider removed from the trust list is still partly trusted |

## 5. Every documented deviation and what it changes (27)

### Cryptography and data formats (4)

| Row | Difference from the real system | Impact |
|---|---|---|
| crypto #10 (CF-IPERIOD); also gov K-5 | i-periods start on the UTC 09:00 grid, not on a uniform Time32 grid from the epoch | 2 s on every pseudonym `validityPeriod.start`. The two validators read the sources in opposite directions. **Owner decision** |
| crypto #30 (CF-ENR-REQ) | The enrolment request carries `crlSeries` 4, which breaks the ASN.1 `(0)` | Follows [SMP] §6.3.1, which overrides the ASN.1. Same bytes on the wire. No trust effect |
| crypto #35 (CF-RSU-SSP); also proto E7 | RSU application certificates carry no SSP | Probably not a deviation: gov E-8 finds no SPaT, MAP or TIM SSP in CAMP Table 1 (§7 item 1) |
| crypto #38; also veh R11 | The published RA permission group breaks the strict chain-length rule. The engine accepts it, as deployed devices do, and counts each acceptance | A real-world quirk the engine reproduces on purpose |

### Protocols and backend flows (6)

| Row | Difference | Impact |
|---|---|---|
| proto A16 (narrowed) | The `infoStatus` SPDU body (crl/ctl/caCcf/ma/ra dates) and its RA signature are not encoded. The device polls the composite CRL conditionally instead | A change costs one exchange instead of two. Poll bytes are close to real |
| proto E7 | Same as crypto #35 | §7 item 1 |
| proto H10 | `indexWithinI` saturates at 255; a real CRL Generator would have to stop issuing | Only reached with more than 127 publications in one week |
| proto C6 (reopened) | OF-1: the RA accepts unregistered enrolment certificates | §4.3 |
| proto C8 (reopened) | OF-1: lifetime of the second enrolment certificate | §4.3 |
| proto F6 (reopened) | OF-2: uploads with no SPDU | §4.3 |

### Governance, deployment and calendar (11)

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
| gov E-1 (reopened) | OF-1: enrolment lifetime | §4.3 |

### On-vehicle and roadside behaviour (6)

| Row | Difference | Impact |
|---|---|---|
| veh R10 | Country regions are not checked against a location; they are only counted | None for US pseudonyms, which carry no region (`region_unchecked` = 0) |
| veh R11 | Same as crypto #38 | Counted |
| veh C6 | "2 km since the last change" is measured as distance travelled, not straight-line distance | On a winding route a change can come slightly early |
| veh M11 | Relayed report uploads go on safety channel 172, not on a service channel advertised by WSA | Report airtime on 172 is an upper bound. The radio has no per-channel MAC queues and no channel switch (07 §7 item 4) |
| veh U5 | No WSA encoder | The SCMS service cannot be advertised, so M11 cannot be fixed yet |
| veh R1 (reopened) | R-TRUSTLEAK | §4.3. **Security; fix first** |

Fourteen of these change nothing a device trusts or does: crypto #30, #35 and proto E7, crypto
#38 and veh R11, crypto #10 and gov K-5 (2 s), and gov G-2, G-11, C-9, C-10, C-11, C-14 and
L-11.

## 6. Runtime defects found by testing (no checklist row)

In order of impact. Each one has its evidence in the named report.

1. **R-USFP: the US misbehaviour profile revokes honest vehicles** (matrix Open 1; stress §1;
   07 §8.3). **The most important open item.**
   - On the realism base, the US SM-MBR default revokes 26 honest vehicles and only 1 of 3
     attackers. 7,200 of its 7,488 reports are RandomPosition. The research suite revokes 0
     honest and 3 of 3.
   - RTK positions (0.26 m) still revoke 26, so raw GNSS error is not the cause, as 07 §8.3
     had assumed. The next suspect is the pairing of claimed position and `generationTime`.
   - At quarter-fleet rush hour, 78 % of honest BSMs trip the check. Report uploads then take
     34 % of the channel.
   - **Impact:** until it is root-caused, no misbehaviour, MBR-airtime or revocation number
     from a US preset is a real-world number. The owner may revert the one line of the
     default (07 §8.3).
2. **R-TRUSTLEAK.** See veh R1 in §4.3.
3. **OF-1 and OF-2.** See §4.3. Both have reproduction tests in `tests/oracle_final.rs`, which
   are ignored until the defects are fixed.
4. **R-NOCRL: no CRL before a provider's first revocation** (stress §2).
   - The real deployment has run since 2015, so every CRACA's CRL exists at t0.
   - In the engine, the 2-week overdue rule therefore cannot act on a CRACA that has not
     revoked in the run.
   - A CRL outage from the start is invisible, and a car that was offline for weeks never sees
     the refusal.
   - Fix: publish each provider's CRL at t0, then re-bless.
5. **O-1: delta CRLs are signed but never sent** (oracle-final §3).
   - 1609.2 allows this.
   - Devices that see only full CRLs find gaps in `indexWithinI` (1 → 3).
6. **Post-quantum path** (matrix; stress §3).
   - **R-PQMBR.** Under ML-DSA-44 every over-the-air report is refused at the WSMP MTU and is
     still counted as uploaded. The refusal itself is a real post-quantum finding; the count is
     a defect.
   - Under the hybrid scheme there is no top-off and no RSU CRL install, and receivers check no
     CRL chain. The post-quantum path is labelled as modelled.
7. **provider-a sizes** (matrix Open 5).
   - The ISS form gives response 501/517, ack 304/306 and ECA response 1,064, against CAMP
     Table 3's 454/470, 292/294 and 958.
   - Most likely its published component certificates are longer. No oracle check of the ISS
     profile exists yet.
8. **Unexplained, not yet shown to be defects:**
   - **27 MA decisions in every variant.** M0, att10 and sybil all make 27, although att10 has
     9 attackers.
   - **Implausibly old evidence.** 10 reports in the realism base were set aside because their
     evidence was up to 43,125 s older than the report, yet no receiver refused anything.
   - **Fast-forward ticks.** 865,653 fast-forward ticks in the edge peer-root-removed run.
   - **Case-board review time.** The case board shows the review as 28,799.2 s, against the
     expected ≥ 28,800 s (matrix L5).
9. **Outside the SCMS:**
   - LTE-V2X BSM p95 latency is 112 ms, over the 100 ms budget, because the BSM is not aligned
     with its SPS reservation.
   - Rebroadcasting a 336 kB standing CRL every 5 s on channel 172 costs about 0.54 Mbit/s per
     RSU (07 §7 item 5). This is a configured behaviour, not a defect.
10. **Red-team finding.** The `generationTime` window makes a replay older than 2 s harmless,
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
  J2945/1 values are not public.

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
| veh S6 | The full-certificate share | Re-measured in steady state at 17 % (matrix), inside the sourced band of 5–30 % |
| veh P7 | P2PCD concurrency limits | 8 responses, 8 inline entries |
| veh P8 | Which P2PCD flavour US devices run | Out-of-band |
| veh C10 | Whether J2945/1 clears the path history at a change | The code clears it (§8 item 4) |
| veh C11 | The pseudonym-change rule for J2945/9 handsets | The scenario's rule |
| veh M9 | The device report store quota | 64 KiB |
| veh M10 | The report rate limit | At most one per second per (reporter, accused) |
| veh M13 | Whether RSUs run misbehaviour detection | They do, as reporters. Whether an RSU may sign an SM-MBR with its application certificate is an owner question (07 §7 item 2) |
| veh V3 | The PSM rate | 1 Hz with a 100 ms floor |
| veh U3 | RSU verification policy | Verify-all |

## 8. Disagreements between the checklists

The totals count each row as its own checklist records it, with the changes in §4 applied.

1. **RSU SSPs: open.** crypto #35 and proto E7 record a deviation. Gov E-8 and F-3 read CAMP
   Table 1 in full and found no SPaT, MAP or TIM SSP in it. On that reading both rows match,
   and the deviations would be 25.
2. **Pseudonym region: open.** crypto #32 says unverifiable. Gov F-2 says "no region", from
   [SMP] §6.3.3.
3. **i-period time base: owner decision** (crypto #10 against gov K-5).
4. **Path history at a change: the checklist text is out of date.** veh C10 says the engine
   does not clear it. `vehicle.rs` `set_identity` does clear it.
5. **MA concurrency: settled.** The MA ran one case at a time; it now runs cases side by side
   (R-SERIAL, 07 §5 bug 1). §6 item 8 is a new, separate question.

## 9. What remains

**Fix first (engine defects, in order of impact)**
1. **R-USFP**: root-cause the false positives, then add a filtered-position model.
2. **R-TRUSTLEAK.**
3. **OF-1**: one enrolment certificate, registered, with a back-dated start.
4. **OF-2**: no upload without an SPDU.
5. **R-NOCRL**: a CRL at t0.
6. **R-PQMBR.**

Items 3–6 change the digests and need a re-bless.

**Owner decisions**
- CF-IPERIOD (2 s).
- Keep or revert the US profile as the preset default while R-USFP is open.
- Whether RSUs may sign reports with their application certificate.
- The US J2945/1 window values.
- Whether SCMS Manager's artefacts may be vendored.

**Engine work for the remaining deviations**
- The `infoStatus` body (A16).
- Per-channel MAC queues, a service channel and WSA bytes (M11, U5).
- Country polygons (R10).
- Governance scenario keys (G-10, C-9, C-10).
- Root termination (L-8).
- Stop issuing at 255 CRLs a week (H10).
- Send the delta CRLs (O-1).
- Post-quantum top-off and RSU CRL.
- SPS-aligned BSM generation for LTE-V2X.

**Testing still owed**
- **The full-scale benchmark.** It needs 2–3.3 CPU-hours; only its first 5 s were run.
- **Oracle on HEAD.** `records` and `ctl-files` against a HEAD recording and the F1 `gov.json`.
  The oracle rules hold on `4924a5d2`.
- **An independent check of the ISS profile** (§6 item 7).
- **Long runs on HEAD.** `scms_knobs_runs` (it ran on the stress branch, not on HEAD), the
  conformance run, and the three threat simulation binaries in release.
- **Post-quantum runs.** The hybrid ML-DSA run, the certificate cycle, and post-quantum
  determinism.
- **F6/F7 fault proofs** (`inject2.py`), one test at a time.
- **The Studio PKI fixtures.** Re-record them; the certificate bytes changed.
- **Realism row V8.** Re-base it on `research-suite`.
- **Two non-SCMS failures**, `longrun_budget` and `timeline`, both already present before the
  merges.

**Paywalled standards: buying them would turn unverifiable rows into checked ones.** IEEE
1609.2.1 body text (crypto #10, proto D11, B9), IEEE 1609.2 §5.3.8 (crypto #59), SAE J2945/1
(veh S2, C10, the window values), SAE J2945/9 (V3, C11, V1) and SAE J3287 (proto F9).

**Values that no purchase can supply.** None of these is published anywhere, so the engine
will always model them, each labelled as a setting:
- data-centre placement and fleet shares;
- EAC vote records;
- the deployed OBU and RSU verification policies;
- live CRL sizes.

"Identical" can therefore only ever mean identical wherever the real system is observable. On
that measure, 270 of 326 rules match. The remaining work is the 22 non-reopened deviations
(14 of which have no trust effect), the five reopened rows, and the six runtime defects of §6.
