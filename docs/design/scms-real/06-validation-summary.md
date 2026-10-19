# 06 — How close the engine's SCMS is to the real US deployment: validation summary

Written 2026-10-18 at `8245ef81` (main, after the five validation merges). This is for the owner and for a reviewer from outside the project. It sums up the four validation checklists, and nothing here goes beyond them. Every claim names the checklist row it comes from, so it can be traced back to the source text, the code location and the test.

**How to read the row references:**
- **crypto #n**: `validation/crypto-formats.md` §1, row n.
- **proto Xn**: `validation/protocols.md`, item Xn.
- **gov X-n**: `validation/governance-deployment.md` §1, row X-n.
- **veh Xn**: `validation/vehicle-and-roadside.md` §1, row Xn.
- **04 §n**, **bench §n** and **RT Rn**: `04-validation-report.md`, `docs/benchmark/scms-real-benchmark.md` and `docs/redteam/pki-privacy.md`.

## 1. What was checked, and how

Four validators each took one part of the SCMS. Each one wrote down every rule that the real US system follows in that part, using primary sources:
- the IEEE 1609.2 / 1609.2.1 ASN.1 modules;
- the SCMS Manager documents (the 1609.2.1 Profile, Provider Requirements, the Elector Policy and Technical Specification, the EAC Requirements, the re-enrolment specification, and the misbehaviour report specification);
- CAMP's EE Requirements 1.2.2;
- the USDOT primer and the CV-pilot reports;
- the CTLs and certificate chains that SCMS Manager actually publishes.

Code comments were never used as a source. For each rule, the validator then found the code that implements it and the evidence that it does what the rule says. Evidence is one of four kinds:
- an official test vector;
- the independent Python oracle, which was written without reading the Rust;
- a byte-for-byte comparison with a published real artefact;
- a Rust test.

Each rule then got one of four verdicts:

- **Matches.** The engine does what the source says, and the evidence is named.
- **Fixed.** The engine did not match. It was corrected in this round, and the new test was shown to fail when the old code was put back.
- **Documented deviation.** The engine does not match yet. The row gives the reason and what the difference changes.
- **Unverifiable.** No public source fixes the behaviour or value. The row says how the engine models it. Each such value is labelled ASSUMPTION in `constants.md`.

A row with two verdicts is counted under its first.

## 2. Totals

| Area | Rules | Matches | Fixed | Documented deviation | Unverifiable |
|---|---|---|---|---|---|
| Cryptography and data formats (`crypto-formats.md`) | 80 | 61 | 7 | 6 | 6 |
| Protocols and backend flows (`protocols.md`) | 109 | 90 | 5 | 7 | 7 |
| Governance, deployment and calendar (`governance-deployment.md`) | 70 | 45 | 8 | 11 | 6 |
| On-vehicle and roadside behaviour (`vehicle-and-roadside.md`) | 67 | 41 | 7 | 8 | 11 |
| **All** | **326** | **237 (73 %)** | **27 (8 %)** | **32 (10 %)** | **30 (9 %)** |

After this round, 264 of 326 rules (81 %) match the real system. Of the other 62:
- 30 cannot be checked against any public source.
- 32 are known differences. Many of them, mostly in governance, change nothing a device trusts or does (§4).

After the merge, the independent checks all pass on main (BUILD-STATUS 2026-10-18):
- Oracle self-test: 412/412 vectors. Mutation run: 24 faults, none silent.
- Published artefacts: 164/164 checks, and `real_artefacts` 7/7.
- PDU checkers: `check_pdus.py` 24/24 and 25/25, `check_appcerts.py` 18/18, `check_run_crl.py` 19/19 and `check_mbr.py` 4/4.
- `profiles.py`: 24/24 certificates match their published role.
- Conformance run: 19,183/19,183 checks with 0 disagreements.

## 3. Area by area

### 3.1 Cryptography and data formats: as close as public material allows

Wherever a public test vector or real artefact exists, the engine reproduces it:
- the COER encoding, re-encoding every published file byte for byte (crypto #1);
- HashedIds (#5–7) and Time32 with leap seconds (#8);
- ECDSA P-256 and P-384 (#52–53);
- ECIES with the corrected P1 (#54–55), AES-CCM (#58), KDF2 and MAC1 (#60–61);
- butterfly key expansion (#63–66) and linkage values (#68–70);
- CTL verification over the six published CTLs (#78).

The engine's component certificates now have the same fields, issuer, CRL series and lifetime as the published certificate of their role (#17–25). Two cases have no public reference:
- ECQV implicit certificates, for which no 1609.2-profile vector exists. The engine and the oracle are two independent implementations that agree (#16).
- The DCM, for which no certificate is published (#26).

### 3.2 Protocols and backend flows: the deployed flows, with a few omitted message types

Every 1609.2.1 message the US flows use is built with the right signing and encryption layers, and the oracle decrypts and verifies it (proto A1–A15). This covers enrolment and re-enrolment (C1–C12), butterfly provisioning with the shuffle, `.info` and batch download (D1–D16), RSU and identification certificates (E1–E6), and SM-MBR reports (F1–F8). It also covers MA investigation through the LAs and the PCA (G1–G11), CRL issuance on the business-day SLA (H1–H9), and device-side CRL handling (I1–I12). What is missing:
- the EE retry timer (B6);
- the cheap "has anything changed?" poll (A16, I10);
- resuming a partial download (D18);
- charging the encoded size of the composite CRL instead of a per-entry estimate (I7).

### 3.3 Governance and deployment: faithful in the rules that affect trust, simplified in the paperwork

What matches:
- Electors, quorum and CTL format agree with SCMS Manager's texts and the decoded CTLs (gov G-3 to G-7, C-1, C-2, C-12).
- The production and test deployments have the published roots and components (D-1, D-2).
- The calendar is the real Eastern-time, federal-holiday calendar (K-1 to K-4, K-6 to K-8).
- RSU and enrolment lifetimes are cited (E-1, E-3 to E-8).

Most deviations here are things outside the network: the EAC's review delays (G-2), couriered artefacts (C-14) and the root termination notice (L-8). Others are bytes of a CTL that devices do not act on (C-9 to C-11).

### 3.4 On-vehicle and roadside behaviour: correct receiver rules; the misbehaviour side is a research model

What matches:
- Receiver validation follows 1609.2 rule for rule (veh R1–R9, R12–R18).
- P2PCD uses the Annex C timers (P1–P6).
- The pseudonym pool and rotation follow CAMP and J2945/1 (C1–C3, C8).

The largest remaining gap from a CV-pilot OBU is misbehaviour detection. The benchmark runs a research detector suite, and most of its detections have no SM-MBR v1.1 subtype (M2, M3, M6). Report uploads use the safety channel, because the radio has no service channel (M11).

## 4. Every documented deviation and what it changes

### Cryptography and data formats (6)

| Row | Difference from the real system | Impact |
|---|---|---|
| crypto #4 (CF-UNCOMP) | On the node path, a peer's *explicit* certificate with an uncompressed key is refused, although the wire format allows it | None for US traffic: every US end-entity certificate is implicit, and the implicit path accepts both forms |
| crypto #10 (CF-IPERIOD); also gov K-5 | i-periods start on the UTC 09:00 grid. A uniform Time32 grid from the epoch would put every start since 2017 2 s earlier | 2 s on every pseudonym `validityPeriod.start` and `startOfNextIPeriod`. The two validators read the sources differently: crypto cites the 1609.2-2025 `IPeriodInfo` text for a uniform grid, while gov cites CAMP's production table for UTC alignment. **Owner decision**; the fix is one line plus a re-pinned test |
| crypto #30 (CF-ENR-REQ) | The enrolment request carries `crlSeries` 4, which breaks the ASN.1 constraint `(0)` | Follows [SMP] §6.3.1, which overrides the ASN.1. Same 2 encoded bytes |
| crypto #35 (CF-RSU-SSP); also proto E7 | RSU application certificates carry no SSP | See §7 item 1: governance row E-8 found that CAMP Table 1 prints no SPaT/MAP/TIM SSP, so this is probably not a deviation |
| crypto #36 (CF-CRACA); also #24 | The device-facing CRL signer is issued by the ICA, so the ICA is the CRACA (SAESOL's production form). ISS and [SMP] Table 1 put the CRL signer under the root. The governance hierarchy mixes the two forms | In the device-facing PKI: none (consistent). In the governance hierarchy, a governance CRL can never revoke a governance sub-CA, so component revocation cannot be expressed. **Owner decision** (crypto §2 gives the options) |
| crypto #38; also veh R11 | The published RA permission group breaks the strict chain-length rule. Like a deployed device, the engine accepts it, and it counts each acceptance | A real-world quirk the engine reproduces: 1,780 tolerated findings in 60 s of the benchmark |

There are also two smaller choices inside fixed rows:
- **crypto #22.** The LAs are valid for 25 y where ISS's are 30 y. The validity of a certificate must fit inside its issuer's, and the engine's ICA has 25 y.
- **crypto #24.** The CRL signer stays ICA-issued (CF-CRACA, above).

### Protocols and backend flows (7)

| Row | Difference | Impact |
|---|---|---|
| proto A16 | The `infoStatus` poll is not built; devices fetch the CRL on a timer | Over-counts bytes when nothing has changed |
| proto B6 | No EE retry (10 s, at most 10 per hour, SCMS-522/523). A request to an offline RA is dropped | An RA-outage or DoS study under-counts retry load (up to 10 per device-hour) and over-counts lost requests. **The most important protocol gap for attack studies** |
| proto D18 | No HTTP `Range` resume of a batch download | An interrupted download costs a full retransfer. Matters only on lossy links |
| proto E7 | The same item as crypto #35 | See §7 item 1 |
| proto H10 | `indexWithinI` saturates at 255; a real CRL generator would have to stop issuing | Only reached with more than 127 publications in one week |
| proto I7 | Engine vehicles are charged 40 B per CRL entry plus the envelope, not the encoded composite CRL (real encoding: 33.1 B per entry) | About 20 % over-charge at 10,000 entries, and the CTL and hash CRLs are left out of the composite. Airtime and Uu bytes of CRL delivery are approximate by that much |
| proto I10 | No `If-Modified-Since` / 412 on CRL, CTL and CCF downloads | Poll bytes are over-counted when nothing has changed |

### Governance, deployment and calendar (11)

| Row | Difference | Impact |
|---|---|---|
| gov G-2 | The board's review and investigation delays before a vote are not modelled | None on the CTL rules, which start at the vote. A scenario can schedule its decision later |
| gov G-10 | The yearly Test Ballot exists in the library, but no scenario key schedules it | A run longer than a year has no test-ballot record |
| gov G-11 | Post-quantum elector key replacement is not modelled | None today. No quantum-safe method has been chosen |
| gov C-9 | CTL `effectiveDate` is set to the publication deadline, not the ballot date | 0–28 days later than SCMS Manager's practice. No effect on trust, because devices do not check it |
| gov C-10 | A run's first CTL is sequence 0; the live production CTL is sequence 1 | Sequence numbers run one lower than the real series. No trust effect |
| gov C-11 | A CTL that drops a root does not also carry the dropped root's certificate | Bytes only. No trust effect |
| gov C-14 | The couriered flash drive and video-call key confirmation are not modelled | Not on the network |
| gov L-1 | Roots are valid 30 y, as in SAESOL production. ISS-pattern roots are 70 y, and the 20-year key cryptoperiod is not enforced | Only the validity end changes, and the encoded size stays the same. No run reaches either end |
| gov L-8 | Root termination (365 days' notice) is not modelled. Removal is | Matters only for a run of a year or more |
| gov L-11 | The LA SSP is version 2 with `laId` 1 and 2. ISS's LAs carry version 1 with ids 4 and 5 | `laId` is a provider's own choice. Version 2 is the 2022 module's fixed value |
| gov K-5 | The same item as crypto #10 | 2 s on validity starts |

### On-vehicle and roadside behaviour (8)

| Row | Difference | Impact |
|---|---|---|
| veh R10 | Country regions are not checked against a location; the cases are only counted | None on US pseudonyms, which carry no region (`region_unchecked` = 0 on the benchmark) |
| veh R11 | The same item as crypto #38 | Counted |
| veh C6 | The "2 km since the last change" is measured as distance travelled, not straight-line displacement | On a winding route a change can come slightly early |
| veh M2 | Detections with no SM-MBR v1.1 subtype are still filed on a "modelled" path and counted | Unrealistic US report volume: 20,421 of 23,112 reports in the pre-fix baseline were unmapped. It is kept so that attack studies of the research detectors still reach the MA. A strict-US mode is still owed |
| veh M3 | The research suite's acceleration check fires at 12 m/s². SM-MBR's default is 10 m/s², and its MaxSpeed 90 m/s check is absent | Detection rates are those of the research suite, not of a CV-pilot OBU |
| veh M6 | A report is filed about a message that failed verification; a CV-pilot OBU has no subtype for that | The MA now sets such reports aside (fixed row M5), so they can no longer get an honest certificate revoked |
| veh M11 | Over-the-air report uploads go on safety channel 172, not a WSA-advertised service channel | MBR airtime on channel 172 is an upper bound. It was 29.3 % before the fixes and 0 % after them in the 60 s re-run |
| veh U5 | No WSA encoder | The SCMS service cannot be advertised, so M11 cannot be fixed yet |

## 5. Every unverifiable item and how it is modelled

### Cryptography and data formats (6)

| Row | Unknown | How the engine models it |
|---|---|---|
| crypto #26 | The DCM certificate (none published) | The RA's lifetime and the 1609.2.1 `DcmSsp` |
| crypto #29 | The enrolment certificate's `cracaId` (set by the ECA; none published) | `000000`. The consequence: a series-4 hash CRL can never match an enrolment certificate, so enrolment revocation is the RA's blocklist only. That is what 01 §4 reports as real practice |
| crypto #32 | Whether pseudonyms carry a region | None. Governance F-2 later decided this from [SMP] §6.3.3 (see §7 item 2) |
| crypto #59 | The CCM nonce rule (body text paywalled) | A fresh random 12 bytes each time, never reused under one key |
| crypto #71 | The group linkage `jValue` and its relation to CAMP's ei(j, k) | Group linkage is off by default (US practice). The CAMP `glv` vectors pass |
| crypto #73 | The LCI, PLV-request and LA key formats (NULL interfaces in 1609.2.1) | Modelled formats. The LCI is a 16 B ASSUMPTION |

### Governance, deployment and calendar (6)

| Row | Unknown | How the engine models it |
|---|---|---|
| gov G-1 | EAC votes and membership | A decision is an instant given by the scenario, logged as `eac-*` |
| gov C-4 | How an elector removal looks (none published) | Listed in `electorRemove`, the standard's informational form |
| gov D-9 | Data-centre regions | Assumed public-cloud metros, labelled |
| gov D-10 | Backend latency | Distance over fibre at about 2/3 c, plus a per-link overhead, labelled |
| gov D-11 | How the fleet is split between providers | Seeded and stable shares, labelled |
| gov E-2 | How "6 years" of enrolment is encoded | `hours(52560)`, 1.46 days shorter than `years(6)`, same size |

### Protocols and backend flows (7)

| Row | Unknown | How the engine models it |
|---|---|---|
| proto B5 | The RA's 1 s TCP ACK as a bound | An M/M/c queue; the delay is measured, not bounded |
| proto B9 | The 1609.2.1 URL paths and HTTP headers | CAMP REST paths and ports. HTTP request lines are not counted (tens of bytes per request) |
| proto C13 | The pilots' manual re-enrolment in a secure environment | The SCMS Manager online process through the RA |
| proto D11 | The meaning of `download-maxAge`, `ra-maxAge` and `eca-*` (paywalled tables) | Not enforced. Read as maximum request age, the engine never violates them |
| proto D17 | The first-batch delay (no published bound) | 60 s after the shuffle releases. The shuffle's own wait of up to 2 days dominates |
| proto F9 | J3287 / ETSI TS 103 759 report content | The deployed SM-MBR format |
| proto H8 | `nextCrl − issueDate` | 7 days, which keeps the "at least weekly" fetch current |

These MATCHES rows also contain one unpublished number each:
- proto D4: the LA–ACA key rotation, 7 d.
- proto E6: "1 month" read as 30 days.
- proto G7: the MA case cap, 1,000.
- proto G8: the manual review, 8 h.
- proto H7: the CRL slot, 10:00 ET.

### On-vehicle and roadside behaviour (11)

| Row | Unknown | How the engine models it |
|---|---|---|
| veh S1 | Deployed OBUs' verification policy | Verify-all, on-demand and prioritised are all modelled and labelled |
| veh S6 | The full-certificate share (25 % against 10 %) | Plausible while the run is filling; to be re-measured in steady state |
| veh P7 | P2PCD concurrency limits | 8 responses, 8 inline entries |
| veh P8 | Which P2PCD flavour US devices run | Out-of-band by default |
| veh C10 | Whether J2945/1 clears the path history at a change | The checklist says "not cleared", but the code does clear it (see §7 item 4). The open question is only what J2945/1 requires |
| veh C11 | The pseudonym-change rule for J2945/9 handsets | The scenario's rule, shared with vehicles |
| veh M9 | The device report store quota | 64 KiB |
| veh M10 | The report rate limit | At most one per second per (reporter, accused) |
| veh M13 | Whether RSUs run misbehaviour detection | They do, as reporters |
| veh V3 | The PSM rate | 1 Hz with a 100 ms floor |
| veh U3 | RSU verification policy | Verify-all |

## 6. What was fixed in this round (27 rules)

Each fix has a test that went red with the old code put back (crypto §4, gov §3, proto K, veh §3).

**Cryptography and data formats (7 rows, 5 findings)**
- **crypto #3, CF-CANON.** Received certificates are now hashed and used as signer input in their canonical form. Before, the same certificate could have two identities, and a peer that sent an uncompressed certificate failed to verify.
- **crypto #43, CF-EXPIRY.** `generationTime < expiryTime` is now enforced when sending and when receiving.
- **crypto #23, CF-MA.** The device-facing MA is root-issued on series 256 for 12 y, as in [SMP] Table 1 and both published MAs. Before, it was ICA-issued on series 2 for 3 y.
- **crypto #20–22, #24, CF-LIFE.** The RA (6 y), ACA (4 y), LAs (25 y) and CRL signer (1473 × 60 h) now have their published lifetimes. All of them were 3 y before.
- **CF-RULE.** The oracle's PDU checkers gained five missing rules. This is a tool fix, not a checklist row.

**Protocols and backend flows (5)**
- **proto I3.** A replayed older CRL can no longer erase a newer revocation in the device model (red-team R2).
- **proto I4.** A stale full or delta CRL can no longer move the node's overdue clock back.
- **proto G6.** Every chain of a blocklisted enrolment is revoked, not only the reported one. Before, 2 of a device's 4 certificates escaped (red-team B4).
- **proto H4.** `indexWithinI` counts the delta CRL as its own CRL.
- **proto D14.** A weekly pre-generation job that was armed before the blocklist now generates nothing (SCMS-509).

**Governance and deployment (8)**
- **gov C-6.** CTL deadlines now count business days on the Eastern date and skip federal holidays.
- **gov C-7.** An elector addition gets 5 business days, not 3.
- **gov G-8.** A replacement elector is valid for 15 y, not 24 y.
- **gov C-3.** A removed root is no longer listed in `rootCaRemove`, as in SCMS Manager's one published removal.
- **gov D-3.** The test-CTL preset approves 5 of its 6 roots.
- **gov F-1.** Both engine PKIs give the RA the production permission form.
- **gov F-4.** The duplicated preload constant is gone.
- **gov F-5.** The real-artefact tests run whenever the cache is present.

**On-vehicle and roadside (7)**
- **veh V1.** Pedestrian handsets now get their own permission profile, `{0x26, 0x27}`. Before, every receiver refused every PSM. That defect was behind the benchmark's report flood and its 10 honest revocations (bench §9.1).
- **veh M5.** The MA sets aside a report whose only evidence is a bad signature. Such a report could get any honest certificate revoked.
- **veh M4.** PSM and VAM stated accuracy is read by the detectors.
- **veh M14.** Detectors check safety messages only, not relayed reports, CRLs or P2PCD frames.
- **veh C7.** A handset's `msgCnt` restarts at a pseudonym change.
- **veh C4, C5.** The J2945/1 rule (5 min **and** 2 km) and the NYC pilot rule (2 km **or** 5 min) can now be selected, and the benchmark uses J2945/1.

The vehicle fixes were re-measured on the first 60 s of the quarter-fleet benchmark (veh §2):
- `psid-not-permitted` refusals fell from 32,052 to 0.
- Reports fell from 23,112 to 15.
- MBR airtime fell from 29.3 % to 0 %.
- Revoke decisions against honest devices fell from 73 to 0.

The BSM and PSM frame counts did not change.

## 7. Disagreements between the checklists (to settle)

The four validators worked in parallel, and a few rows disagree. The totals above count each row as its own checklist records it.

1. **RSU SSPs.** crypto #35 and proto E7 record a deviation, citing CAMP Table 1. Governance E-8 and F-3 read Table 1 in full and found no SPaT, MAP or TIM SSP in it. The one SSP it prints, Wyoming's distress notification, is transcribed and pinned byte for byte. On that reading, both rows are matches, and the deviations count would be 30.
2. **Pseudonym region.** crypto #32 says unverifiable. Gov F-2 decided "no region" from [SMP] §6.3.3, which sets no region on authorisation certificates, and recorded a match. Without a region, a pseudonym is 6 B smaller than CAMP's 2016-pilot example (80 B against 86 B, 04 §3).
3. **The i-period time base** (crypto #10 against gov K-5). Both rows call it a deviation but read the sources in opposite directions. The 1609.2.1 body text is paywalled, so this needs an owner decision.
4. **Path history at a pseudonym change.** veh C10 says the engine does not clear it. The code does clear it: `crates/v2xw-node/src/vehicle.rs:207` `set_identity` clears `history` when the identity changes, and RT §5 says the same. The row's description of the engine is out of date. Whether J2945/1 requires the clearing is still unverifiable.
5. **MA concurrency.** proto G7 and its test show concurrent cases. bench §9.3 saw one decision per provider every 8 h under the report flood. This is open: either the benchmark's queue is serial, or the reading is wrong.

## 8. What remains to make it identical

These are grouped by what they need, most important first.

**Owner decisions (no research needed)**
1. **CF-CRACA** (crypto §2): give the governance sub-CAs the root as CRACA, or move the governance CRL signer under the ICA, so that component revocation can be expressed.
2. **CF-IPERIOD / K-5**: choose UTC-aligned or uniform-Time32 i-period starts (2 s).
3. Whether SCMS Manager's "All Rights Reserved" artefacts may be vendored. Today they are fetched by `tools/scms-artefacts/fetch.py`.

**Engine work that would turn a deviation into a match**
4. EE request retry, SCMS-522/523 (proto B6). It changes every flow's event stream, so it needs a digest re-bless.
5. Charge the encoded composite CRL, with the CTL and hash CRLs, instead of 40 B per entry (proto I7). Add the `infoStatus` poll and 412 (A16, I10), and `Range` resume (D18).
6. A strict-US misbehaviour profile: SM-MBR v1.1 subtypes only, with the §2.3 thresholds (MaxSpeed 90 m/s, MaxAcceleration 10 m/s², RandomPosition 1 m), as the US default (veh M2, M3, M6).
7. A service channel and WSA in the radio, so report uploads leave channel 172 (veh M11, U5).
8. Country polygons for identified-region checks (veh R10). Accept uncompressed explicit keys on the node path (crypto #4, CF-UNCOMP).
9. Per-provider certificate profiles: ISS's 70 y root, regions on sub-CAs and request-form RA, beside SAESOL's production form (gov L-1, L-5, F-1).
10. Governance in scenarios: keys for `replace_elector`, `add_root` and `test_ballot` (G-10); CTL `effectiveDate` at the ballot (C-9); genesis at the real production sequence 1 (C-10).
11. A business-hours MA mode. The flat 8 h review is about 21 h optimistic at the median (RT §4.1). Also stop issuing CRLs at 255 per week (proto H10).
12. The test helper `pdu.rs` `issue_component` still issues for 3 y. It is used only in tests (BUILD-STATUS).

**Re-measurement and independent checks**
13. Re-run the full 172,800 s benchmark on a release build. Its §3–§6 numbers predate the vehicle fixes. Re-measure the full-certificate share in steady state (veh S6) and investigate the fast-forward stalls (bench §9.2) and the MA queue (§7 item 5).
14. Add independent oracle rules for `indexWithinI` and CRL freshness (proto L). Today only Rust tests check them.
15. Correct the checklist rows in §7 items 1, 2 and 4.

**Paywalled standards: buying them would turn unverifiable rows into checked ones**
16. IEEE 1609.2.1 body text: the time base, the `*-maxAge` meanings and the URL paths (crypto #10, proto D11, B9).
17. IEEE 1609.2 §5.3.8: the CCM nonce rule (crypto #59).
18. SAE J2945/1: CertAttachInt (450 ms or 1 s; veh S2) and path history at a change (C10).
19. SAE J2945/9: the PSM rate, handset permissions and pseudonym change (V3, C11, V1).
20. SAE J3287: misbehaviour content (proto F9).

**Values that no purchase can supply.** Data-centre placement, fleet shares, EAC vote records, the deployed OBU and RSU verification policies, and live CRL sizes are not published anywhere (gov D-9, D-11, G-1; veh S1, U3; 04 §7 U13). The engine will always model these, with each one labelled as a setting. "Identical" can therefore only ever mean identical wherever the real system is observable. On that measure, the remaining work is items 1–15.
