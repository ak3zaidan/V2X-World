# Misbehaviour pipeline audit, hop by hop (audit-mbpipe)

Date 2026-10-24. Base `8becf72a`, integrator's release binary (copied). Table:
`crates/v2xw-engine/tests/expectations/scms-audit-mbpipe.yaml`, 41 rows, committed with
`measured: pending` before any judged run (`234b982a`). Scenario:
`scenarios/scms-audit-mbpipe.yaml`, which is the conformance deployment (two providers, real
calendar, the 660 s, 300 s and 300 s live windows across 5 days, manual review) plus
`report_records`, the PKI view and 40 % relay-only vehicles. Checker:
`tools/realworld/audit_mbpipe.py`. It reads the raw records only and shares no code with the
engine:

```
python3 -I tools/realworld/audit_mbpipe.py check    <table> <runs-dir> [--fill]
python3 -I tools/realworld/audit_mbpipe.py selftest <table> <runs-dir>   # each row's fault
python3 -I tools/realworld/audit_mbpipe.py hops     <run-dir>
```

Runs (`--record-level receptions`, run lock 1,200 MB):
- **AUD**: 1,127 s wall, 70 MB peak.
- **AUD2**: the same run again.
- **AUDF**: the same scenario on the fixed build (`v2xw-fix`, opt-level 2 dev build of this branch).

## 0. Verdict

**41 rows:**
- 29 pass first time;
- 2 engine defects fixed at their cause, each with a test that fails without the fix;
- 7 checks corrected with evidence (wrong-expectation);
- 3 open: two record gaps and one small metric defect.

**Fault proof:** every row goes red under its injected fault (`selftest`, 41 of 41).

**On the fixed build (AUDF):**
- 38 of 41 pass, and the 3 open rows are unchanged.
- AU.RA.shuffled: Kendall tau 0.009, against 1.000 before the fix.
- AU.STORE.engine: `reports_evicted` 228, equal to the records (it was 17).
- Every other row keeps its verdict:
  - the same 5 attackers are decided after the same 8 h review;
  - the same publication is at Thu 10:00:00 EDT;
  - no honest device is decided.

**Determinism:** AUD and AUD2 are byte-identical over every recorded message, the run
report and `metrics.json` (digest `681db690d9e6d7db…`).

**The pipeline is end to end:** every report and CRL entry can be followed from creation to
its effect. On AUD:
- 2,876 reports were stored. Each one ends exactly once: 2,473 uploaded, 81 evicted by the
  quota, 147 decayed out and 175 left with their device.
- All 2,473 uploads reached an RA. 1,330 were forwarded once each; 1,143 were still in the
  day-long shuffle at the end.
- 5 cases opened, all about attackers. After the 8 h review, 5 decisions were made, each
  taking the specified PCA→RA→ECA→LA1/LA2→CRLG path.
- 1 publication at the 10:00 ET slot. 4 of the decisions came on Friday evening, so their
  slot (Monday) is past the horizon.
- The CRL was installed by 14 vehicles by poll or broadcast and by 35 joiners, and the 2 RSUs
  refused the revoked sender.
- 7,201 receptions of the revoked sender were refused, none of them before the receiver held
  the list.

## 1. Inventory (AUD unless stated)

| Hop | Behaviour (row) | Spec value | Measured | Verdict |
|---|---|---|---|---|
| Detector | report names a pseudonym the reporter received (AU.DET.belief) | belief only; [SM-MBR] §2.3.1 | 0 of 2,876 unheard | pass |
| Detector | US subtypes only (AU.DET.subtypes) | v1.1 PSID 0x20 set, ObsImplausible off | 5 of 5 ConstantPosition | pass |
| Detector | ≤ 1 report a second per reporter and accused (AU.DET.rate) | 1 s (ASSUMPTION, veh M10) | 1.0008 s min on the judged messages | wrong-expectation (stored instants differ by 0.999 s) |
| Creation | one SecuredMisbehaviorReport per POST (AU.MBR.one-pdu) | [SM-MBR] §4 | 2,473 of 2,473 | pass |
| Store | stored = uploaded + evicted + decayed + expired + held + left (AU.STORE.conserve) | [SM-MBR] §2.2 | 2,876 = 2,473 + 81 + 147 + 0 + 0 + 175 | pass |
| Store | initial weight 1.0 (AU.STORE.weight) | [SM-MBR] §2.2 | 2,876 of 2,876 | pass |
| Store | highest weight, newest first (AU.STORE.order) | [SM-MBR] §2.2 | not observable (282 relay confirmations out of send order) | **open** (record gap) |
| Store | run report = fates (AU.STORE.engine) | identity | engine 17 vs 228 deletions (AUD); 228 = 228 (AUDF) | **fixed** |
| Upload | cellular over Uu, relay-only via unit (AU.UP.link) | [SM-MBR] §4 | 1,447 Uu / 1,026 `rsu-backhaul`, 0 wrong | pass |
| Upload | relay upload counted only once a unit decoded it (AU.UP.relay-heard) | 06 §6 item 5 | 1,026 of 1,026 (within +10 ms) | wrong-expectation (stamp order) |
| Upload | run-report upload counters (AU.UP.engine) | identity | 1,447 / 1,026 / 2,473 equal | wrong-expectation (see §3) |
| Upload | cellular store→upload p95 (AU.UP.cell-latency) | immediate | 0.000 s | pass |
| RA | every report forwarded once (AU.RA.once) | — | 0 lost, 0 dup, 1,143 held at end | pass |
| RA | batch closes at 1 day or 10,000 (AU.RA.window) | [CAMP122] SCMS-765 | 4 batches, open + 86,400.072–.077 s | pass |
| RA | batch order shuffled (AU.RA.shuffled) | SCMS-765 | tau 1.000 (AUD), 0.009 (AUDF) | **fixed** |
| RA | RA forwards only the encrypted report (AU.RA.strip) | [SM-MBR] §4 | 1,330 of 1,330 = PDU length | pass |
| MA | manual review 8 h (AU.MA.review) | 28,800 s (G8, ASSUMPTION) | 28,799.84–28,800.00 s | pass |
| MA | case stage order (AU.MA.stage-order) | [BRECHT] §VI | 5 of 5 | pass |
| MA | decision rule support (AU.MA.rule) | ≥ 3 reporters, span ≥ 10 s | 3–10 reporters, 34.9–273.6 s | pass |
| MA | no decision on set-aside reports (AU.MA.set-aside) | veh M5 | 0; 0 of 1,330 set aside | pass |
| MA | no honest device decided (AU.MA.honest) | 06 §3 | 0 of 5 | pass |
| MA | case at the issuing provider (AU.MA.home) | G9 | 5 of 5 | pass |
| LA | PCA/RA/ECA/LA1/LA2/CRLG path (AU.LA.path) | [BRECHT] §VI-D | 5 of 5 | pass |
| LA | forward-only seeds (AU.LA.forward) | G3 | i_rev 614/615 ≥ reported i | pass |
| CRLG | first 10:00 ET business-day slot (AU.CRL.slot) | [SM-PROV] §3.4.9–10 | Wed 12:59 → Thu 10:00:00 EDT | pass |
| CRLG | nextCrl − issueDate 7 d, series 1 (AU.CRL.next) | H8, H2 | 3 of 3 lists | pass |
| CRLG | entries = decisions; revocations_by_provider (AU.CRL.entries) | — | 1 = 1; a 1, b 4 | pass |
| CRLG | no list rollback on a node (AU.CRL.no-rollback) | I3, I4 | 0 over 130 | pass |
| Distribution | cellular install ≤ 3,660 s after publication (AU.DIST.cell-poll) | hourly poll, I6 | 8 of 8, latest 3,314 s | pass |
| Distribution | vehicles hold the list (AU.DIST.held) | conservation | 49 of 49 | wrong-expectation (relay-only in fast-forward) |
| Distribution | relay-only learns only from a unit frame (AU.DIST.relay-heard) | I9, I12 | 6 of 6 | pass |
| Distribution | RSUs hold the list (AU.DIST.rsu) | WP-B3b | no record of a unit install | **open** (record gap) |
| Distribution | crls_installed = records (AU.DIST.engine) | identity | 49 = 14 + 35 at join | wrong-expectation (joiners) |
| Enforcement | revoked sender refused after install (AU.ENF.drop) | 1609.2 5.1.3.4 | 0 of 6,013 accepted | pass |
| Enforcement | no refusal before the list is held (AU.ENF.not-before) | who learns what | 0 of 6,013 (1,188 unit refusals not judged) | wrong-expectation (joiners) |
| Enforcement | honest self-revoked device goes silent (AU.ENF.self) | [CAMP122] §5.2.10.2.3 | none honest; attacker keeps sending by design | wrong-expectation |
| Enforcement | revoked_receptions, rsu_revoked_receptions (AU.ENF.engine) | identity | 7,201 / 1,188 equal | pass |
| Metrics | revocation_latency_stage (AU.MET.stages) | security.rs | 9 transitions equal | pass |
| Metrics | crl_entries, crl_bytes (AU.MET.crl) | last published | 1 / 252 B equal | pass |
| Metrics | det_* cells, false_accusations, time_to_* (AU.MET.detection) | detection.rs | cells, false_accusations, time_to_decision equal; time_to_detect 27 ms long for one attacker | **open** (metric keeps the first-emitted report, §3) |
| Determinism | AUD2 = AUD byte for byte (AU.DET.same-seed) | — | identical | pass |

## 2. Hop latency and loss (AUD, `hops`)

| Hop | n | median | p95 | max |
|---|---|---|---|---|
| stored → uploaded, cellular | 1,447 | 0.000 s | 0.000 s | 0.000 s |
| stored → uploaded, relay-only | 1,026 | 4.47 s | 195,782 s | 236,598 s (waits through fast-forwards out of range) |
| RA upload → forward (shuffle) | 1,330 | 86,219 s | 86,389 s | 86,400.08 s |
| forward → MA received | 1,330 | 0.071 s | 0.151 s | 0.166 s |
| reports-in → corroborated | 5 | 0.000 s | 0.020 s | 0.026 s |
| corroborated → decision (review) | 5 | 28,800 s | 28,800 s | 28,800 s |
| decision → la-seeds | 5 | 0.004 s | 0.004 s | 0.004 s |
| la-seeds → crl-published | 1 | 75,644 s | | |
| publication → install (vehicles) | 14 | 3,314 s | 4,845 s | 4,855 s (relay-only vehicles after the fast-forward) |

**Losses:**
- 403 of 2,876 stored reports never left their device: 228 to the quota and decay, and 175
  that drove out of the run.
- 912 detections were not filed:
  - 477 by RSUs (R-UNSEC);
  - 420 with no v1.1 subtype;
  - 15 with too few BSMs.
- No upload was lost between the RA and the MA.

## 3. Fixes and findings

**Fixed (engine defects; tests fail without the fix):**

1. **The RA report shuffle did not shuffle** (AU.RA.shuffled).
   - **Defect:** `release_report_shuffle` forwarded each batch in arrival order: Kendall tau
     1.000 in all 4 AUD batches. The MA's receipt order therefore gave away the upload order
     that CAMP-EE SCMS-765's shuffle exists to hide.
   - **Fix:** a seeded Fisher–Yates permutation on its own stream (`Backend`,
     `scms/report-shuffle`, generation). No other draw moves, and the same seed gives the same
     order.
   - **Test:** `v2xw-proto lockstep::the_ra_forwards_a_report_batch_in_shuffled_order`. It is
     red with the permutation disabled ("the batch reached the MA in arrival order").
   - **Goldens:** one record digest moved, the `phase2-path` record:
     - before: `fd19a3ed…`;
     - after: `e723df97…`.

     The record count (6,372,470) and the frame digest are unchanged, because only the MA's
     receipt order moves. Re-blessed with `V2XW_BLESS_SCMS_DIGESTS=1` on the `phase2`,
     `scms_scale` and `bsm_timing` tests, and every test in them passes: 3, 13 and 1 tests,
     with 2 ignored.
   - **The certificate-request shuffle** (`provision.rs` `release_jobs`) also releases in
     arrival order. It belongs to the provisioning owner and is not changed here.
2. **Store deletions of vehicles that left the run were lost** (AU.STORE.engine).
   - **Defect:** `Phase2::retire` dropped a leaving vehicle's `MbrQueue` evicted and expired
     counters. The run report said 17 quota deletions where the fates hold 228.
   - **Fix:** the counters are now kept.
   - **Test:** `phase2::evidence_time_tests::a_retired_vehicles_store_deletions_stay_counted`.

**Open:**

- **AU.STORE.order (record gap).** A relayed report's `node.tx` row carries no report id, so
  transmission order cannot be joined to the store. Confirmation order differs from
  transmission order whenever a unit misses a frame. Suggested fix: the report id on the
  relayed frame's tx row.
- **AU.DIST.rsu (record gap).** No record shows an RSU installing a CRL. The only evidence is
  the engine counter `rsu_crls_installed` (8) and the 1,188 unit refusals. Suggested fix: a
  `proto.revocation` row or `node.security` rows for RSUs.

- **AU.MET.detection (metric defect, small).**
  - **The defect:** `DetectionProvider::on_report` keeps the first `ma.report` emitted for each
    subject (`or_insert`). The engine emits a step's `ma.report` rows out of time order.
  - **Example:** actor 16's earliest report at the MA is at 86,535.830 s, but the first one
    emitted is at 86,535.857 s. Its `time_to_detect` is therefore 27 ms long.
  - **Suggested fix:** keep the minimum t.
  - **What does match:** with two checker corrections, every other detection metric equals
    the records. The latencies are cumulative per run, and the onset is the first action that
    changed bytes on the air.

**Findings without a row:**

- **`time_to_decision` reads `ma.decision`.** That record is stamped when the MA pipeline
  recommends (86,955.8 s), 8 h before the operator's decision (pki.case `decision` at
  115,755.8 s). Under ManualReview the metric therefore understates the authority's decision
  by the review time. Vehicle-level `det_*` uses the same instant.
- **`reports_sent` is off by 15.** It is 3,773, which is stored (2,876) plus not filed (912)
  minus 15. The 15 are "too few BSMs" refusals of reports that were already stored, and
  `reports_received` (2,488) exceeds the posts (2,473) by the same 15. Not traced.
- **A relayed report frame on channel 172 carries `claimed_x/y/speed` content** in its tx row.
  It is not checked as a beacon (`messages_not_checked`), but its sender's position is on the
  air in the engine's relay model. The real upload is IP over a service channel (veh M11,
  documented deviation).
- **Not checked here:**
  - the server API and the Studio PKI view (live VWP only, no recorded-run endpoint);
  - the PSID 0x26 inside the encrypted report (the oracle's MBR rules cover it);
  - the quota's 64 KiB value (ASSUMPTION).
