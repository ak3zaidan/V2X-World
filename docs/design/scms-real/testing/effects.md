# SCMS effects: how SCMS and environment changes move metrics, cost and architecture

Round: SCMS realism testing, 2026-10-18 and 2026-10-19 (two sessions; the second resumed
the first after a spend limit). Key `effects`. Table:
`crates/v2xw-engine/tests/expectations/scms-effects.yaml`, which
`tests/scms_effects.rs` checks for completeness.

## Method

- **Base run L0.** `scenarios/credential-weeks.yaml` at 300 veh/h, cut to 2.27 days. It
  runs on the real calendar from Tue 2026-10-20 08:50Z, with the i = 614 → 615 rollover
  at 600 s. Radio is on for 0–900 s and for 196,200–196,500 s (Thu 15:20Z). Between the
  two windows the run fast-forwards and only the backend runs.
- **Deployment.** Two providers split 50/50, the manual MA (8 h review), the business-day
  CRL rule (10:00 ET slot) and 5 % ConstPos attackers. Seed 0xC0FFEE5EED.
- **Variants.** Each variant changes one setting on the same seed. Every run went
  through the run lock, one run per `heavy.sh` call.
- **Binaries.** There are two:
  - *Release.* Round 1 used the release `v2xw` built from main `d3039b53`, with
    `--no-verify` (see fix 1).
  - *Branch.* Round 2's runs are `L0N`, `ATT20N`, `OND`, `REPLAY` and `RSU5`. They used
    a `v2xw` built from this branch at opt-level 2, so they carry fixes 1–3 and verify
    their recordings.
  - Compare wall time and CPU time only within one binary. They are noisy even then:
    the L0 rerun (L0b) took 36 % more CPU for the same 6,665,153 records, because other
    agents' runs shared the machine. Use record counts, which are deterministic, as the
    stable measure of cost.
- **What was read from each run:**
  - `run-report.json`, flattened.
  - The `metrics.json` samples, pooled the way the sensitivity fingerprint pools them.
  - `/usr/bin/time` wall time and peak RSS.
  - The PKI view records (`pki.flow`, `pki.case`, `pki.knowledge`, `pki.state`),
    summarised by flow, message, sender and receiver role, and case stage times.
- **ACA blindness check (`aca_check.py`, round 2).** For ATT20N, every RA-bound and
  ACA-bound PDU was decoded with the independent oracle's COER (`tools/oracle`).
- **Expectations first.** Every expectation was written down before its run, with its
  source. Each row is judged by direction, ratio band or value band.
- **Invariant rows.** A row whose base and variant are the same run checks a property
  of that one run's recorded architecture.

## What I fixed

1. **Fast-forward recordings failed the read-back** (round 1, `ec8a582f`).
   - Every `v2xw run` of a scenario with `time.fast_forward` failed with "N ns between
     keyframes exceeds the cadence" and left no `run-report.json`.
   - Fix: the reader accepts a keyframe gap or a snapshot gap only when it spans one of
     the manifest's declared `time_dilation_windows`.
   - Test: `crates/v2xw-cli/tests/fast_forward_recording.rs`.
   - Confirmed in real runs: the round-2 branch binary verifies a recording of
     `credential-weeks` (L0N and the others), and the main binary refuses the same
     scenario.
2. **The case board recorded the wrong times** (round 1 found it; round 2 finished the
   fix, `1a33e650`).
   - Symptoms in L0:
     - The board stamped `crl-published` at the issue instant, 115,423 s. The CRL Store
       had the list at the Thu 10:00 ET slot, 191,400 s, so the board was 21 h early.
     - It stamped `reports-in` and `corroborated` at the decision, 8 h late.
   - Cause: the engine drains the board every backend step, and the stage fell back to
     `issued` on the first drain and was never revisited. `reports-in` was read from the
     case's own run, which never receives a report.
   - Round 1 left the fix uncompiled. It also had two defects:
     - the `issued` fallback still came first, so the new lookup never ran;
     - it recorded times as `at + work`, which misses queueing at the entity, so the
       board and the stage log disagreed by 1 ms in the test.
   - Fix: record the runs, not the times.
     - `CrlState::published_runs`: `crl-published` is the first `published` stamp at the
       CRL Store at or after the entry's issue.
     - `MaState::report_runs`: `reports-in` is the earliest `report-received` stamp of
       the case's reports.
     - `Case::opened_at`: `corroborated` is when the MA opened the case, and never
       before its evidence arrived.
   - Measured on L0N: `crl-published` 191,400 s and `reports-in` 86,623 s, both in
     their bands (rows `conc-board-*`).
   - Test: `v2xw-proto/tests/pki_view.rs::the_board_stamps_publication_when_the_store_has_the_list_and_reports_when_they_arrived`.
     It goes red under each of three injected faults: the `issued` fallback put back;
     `reports-in` read from the case's run; `corroborated` stamped at the decision.
3. **The MA worked one case at a time** (round 2, `6e580cd7`; round 1 left this open).
   - `Phase2::start_cases_of` opened a case only when no other case was open. Under the
     pilots' manual review, the n-th attacker decided in one shuffle release therefore
     waited n review periods.
     - In L0 the second decision came 8.0 h after the first.
     - In ATT20, 3 of the 10 decisions never opened a case before the run ended.
   - This contradicted gap row 27 and the proto's own case table
     (`MaPolicy::max_open_cases` 1,000, which already ran cases concurrently).
   - The driver serialised because it took each case's entry as `crlg.entries.last()`.
     A case's entry is now found by its released seeds, and every decision opens its
     case at once, up to the table's capacity.
   - Measured on L0N: the two decisions came 0.09 s apart, and the first revocation's
     latency was unchanged at 53.02 h. The ATT20N rows (below) measure it at 20 %
     attackers.
   - Test: `v2xw-engine/tests/phase2.rs::every_decision_opens_its_case_at_once_under_manual_review`.
     With the old `while !case_open()` gate restored, it is red: 2 cases opened, fewer
     than the decisions.
   - `phase2-path` was re-blessed. Its record digest moves with the board's stage
     times, while its frame digest and record count are unchanged. The `i-s1` digest is
     unchanged.
4. **The sweep loader skips this round's tables** (round 1). `load_expectations` passes
   over a file whose `schema` is `v2xw/scms-run-expectations/1`.
5. **Checks that could not fail, corrected.**
   - *ACA blindness.* Round 1's HashedId8 byte search found 0 leaks, but its positive
     control also found 0, so the check could not fail. An `EeRaCertRequest` carries
     the whole enrolment certificate, not its HashedId8. `aca_check.py` replaces it:
     - it decodes the signer of each RA-bound request with the oracle;
     - it searches the ACA-bound PDUs for those certificates and their HashedId8s;
     - it injects a leak to prove the search can fail.
     Rows `aca-*` hold the results. The old rows are marked `wrong-expectation`.
   - *Metric keys.* `cbr` and `verify_cost` named metrics that do not exist, so they
     read n/a. They now read `channel_load` and the verify stage's latency share.

## What the runs show

The table has 88 rows: 77 pass, 7 fixed and 4 wrong expectations, each corrected with
the evidence. No row is open.

### The base run against the real deployment

L0's first revocation, by its recorded stages. Times are measured from detection.

| Stage | Time | Driven by |
|---|---|---|
| Report sent | 9 ms | J2945/1 detection, cellular upload |
| RA shuffle release | 23.9 h | the 1-day maximum of the 10,000-report / 2-day shuffle ([CAMP122] SCMS-765) |
| MA decision | 31.9 h | 8 h manual review (pilot MA, ASSUMPTION) |
| CRL entry issued | +10 ms | 2 PCA lookups, 1 RA blocklist, 2 LA seed releases |
| CRL published | 53.0 h | the next business-day 10:00 ET slot (Thu 14:00Z) |
| First vehicle enforces | 53.0 h + 91 s | cellular CRL fetch |

### Which settings move what

- **Revocation latency is set by the calendar, not by computation.**
  - The automatic MA cut it from 53 h to 29 h, because a decision at the shuffle
    release makes the Wednesday 10:00 ET slot.
  - On-decision publication cut it to 32 h.
  - The backend's own work is 10 ms.
- **The concurrent case table (fix 3) removes the queue behind the review.** At 20 %
  attackers:
  - 10 of 10 decisions became cases, up from 7;
  - revocations rose from 6 to 10;
  - all decisions came within 0.12 s of each other, instead of 16 h apart;
  - each case made 2 seed requests, one per LA;
  - no honest device was revoked;
  - run size grew by 0.01 %.
- **Credential settings.**
  - 40 certificates per week doubled the certificates issued (280 → 560) and the
    RA → ACA requests (4,603 → 9,203). The top-up count stayed at 14.
  - A 60 s pseudonym period tripled the pseudonym changes (111 → 330). It raised P2PCD
    requests by 21 % and the unknown-signer loss by 27 %.
- **Certificate attachment.**
  - At 450 ms the full-certificate share doubled (0.10 → 0.20) and the unknown-signer
    loss nearly halved.
  - On every message: full-certificate share 1.0, no P2PCD, no unknown-signer loss,
    +66 % security bytes on air, and channel load 0.0117 → 0.0147.
- **Crypto hardware and PQ.**
  - A software-only SoC raised the verify stage's latency share from 0.034 to 0.169.
  - Hybrid Falcon-512 + ECDSA raised security bytes on air 9.2× and channel load 4.2×,
    with fragmentation on.
- **Verification policy (finding).** On-demand verification cut verifications from 756/s
  to 46/s.
  - It also left 89.5 % of receptions unverified.
  - Detection runs only on verified messages, so reports fell from 897 to 79, and no
    attacker was decided or revoked in 54 h.
  - A fleet that verifies on demand is blind to these attackers.
- **Attacker type.** DataReplay, as modelled here, stamps a stale generation time, which
  the freshness check catches on every message (3,434 verdicts). It was revoked like
  ConstPos.
- **Fleet.** Doubling the demand gave:
  - 1.77× the provider-a devices and 1.7× the top-ups;
  - 4.3× the CRL installs (the second radio window holds more vehicles);
  - 3.15× the records and 2.4× the CPU time.
- **Providers.**
  - A 90/10 split cut cross-provider verification from 48 % to 16 %, and the reports
    shared between MAs from 426 to 98.
  - The single-provider CAMP reference shared none.
- **Roadside and cellular.**
  - Five RSUs gave 5× the CRL broadcasts (60 → 300). Cellular report upload changed
    by 9 % and top-ups not at all, because cellular penetration is full.
  - With no cellular, 1,523 reports went up through the RSU relay.
  - With neither cellular nor RSUs, nothing was revoked and no top-up happened.
- **Recorded architecture.**
  - The ACA was blind in every run. In ATT20N, all 4,603 ACA-bound requests are
    RA-signed and carry neither an enrolment certificate nor any of the 128 enrolment
    HashedId8s.
  - The check has a positive control, which finds the enrolment certificate in each
    decoded RA-bound request, and an injected leak turns it red.
  - Coverage is partial: the recording keeps the plaintext of only 6 EE → RA PDUs, so
    only one device's whole certificate is searched for.

## Table

Each row's source and notes are in the YAML. In the `change` column, "(invariant)"
means one run checked against a property of itself.

| id | change | measure | expected | measured | verdict |
|---|---|---|---|---|---|
| base-batch | L0 (invariant) | certs_per_topup | eq 20 | 20 | pass |
| base-shuffle | L0 (invariant) | stage_shuffled_h | value-band [23.0, 24.01] | 23.89 | pass |
| base-review | L0 (invariant) | review_h | value-band [7.99, 8.01] | 8 | pass |
| base-sla | L0 (invariant) | stage_published_h | value-band [52.9, 53.1] | 52.99 | pass |
| base-board-published | L0 (invariant) | first_case_crl_published_s | value-band [191399, 191401] | 1.154e+05 | fixed |
| base-board-reports | L0 (invariant) | first_case_reports_in_s | value-band [86000, 86700] | 1.154e+05 | fixed |
| base-concurrent-cases | L0 (invariant) | decision_spread_h | value-band [0.0, 0.5] | 8 | fixed |
| base-no-honest | L0 (invariant) | revoked_honest | zero | 0 | pass |
| base-seeds | L0 (invariant) | seed_requests | eq 4 | 4 | pass |
| base-one-publication | L0 (invariant) | crl_publications | eq 1 | 1 | pass |
| base-split | L0 (invariant) | share_a | value-band [0.4, 0.6] | 0.5327 | pass |
| base-cross-verify | L0 (invariant) | cross_share | value-band [0.35, 0.65] | 0.4841 | pass |
| base-shared | L0 (invariant) | reports_shared | positive | 426 | pass |
| base-cost | L0 (invariant) | cpu_s | value-band [30, 900] | 130.6 | pass |
| base-rerun | L0b | reports_at_ma | same | 897 -> 897 | pass |
| base-rerun-flows | L0b | seed_requests | same | 4 -> 4 | pass |
| att-attackers | ATT20 | attackers | band [2.5, 6.0] | 5 -> 20 | pass |
| att-reports | ATT20 | reports_at_ma | up | 897 -> 3140 | pass |
| att-cases | ATT20 | cases_opened | up | 2 -> 7 | pass |
| att-seeds | ATT20 | seed_requests | up | 4 -> 12 | pass |
| att-honest | ATT20 (invariant) | revoked_honest | zero | 0 | pass |
| att-aca-blind | ATT20 (invariant) | aca_leaks | zero | 0 | wrong-expectation |
| att-aca-control | ATT20 (invariant) | aca_control | positive | 0 | wrong-expectation |
| att-spread | ATT20 (invariant) | decision_spread_h | value-band [0.0, 8.5] | 16 | fixed |
| ma-auto-decision | MAAUTO | stage_decision_h | band [0.7, 0.78] | 31.89 -> 23.89 | pass |
| ma-auto-latency | MAAUTO | revocation_latency_h | band [0.5, 0.6] | 53.02 -> 29.02 | pass |
| ma-auto-honest | MAAUTO (invariant) | revoked_honest | zero | 0 | pass |
| c40-certs | C40 | certs_topped_up | band [1.9, 2.1] | 280 -> 560 | pass |
| c40-aca | C40 | ra_aca_requests | band [1.8, 2.2] | 4603 -> 9203 | pass |
| c40-topups | C40 | topups_completed | same | 14 -> 14 | pass |
| c40-changes | C40 | pseudonym_changes | same | 111 -> 111 | pass |
| c40-cost | C40 | cpu_s | band [0.7, 1.6] | 130.6 -> 148.8 | pass |
| aall-share | AALL | full_cert_share | band [5.0, 12.0] | 0.1006 -> 1 | pass |
| aall-p2pcd | AALL | p2pcd_requests | down | 26.44 -> 0 | pass |
| aall-air | AALL | air_bytes_security | up | 1.637e+07 -> 2.722e+07 | pass |
| aall-cbr | AALL | cbr | up | 0.01171 -> 0.01472 | pass |
| aall-backend | AALL | topups_completed | same | 14 -> 14 | pass |
| f2x-devices | F2X | devices_a | band [1.6, 2.4] | 57 -> 101 | pass |
| f2x-topups | F2X | topups_completed | band [1.4, 2.8] | 14 -> 24 | pass |
| f2x-crl | F2X | crls_installed | up | 84 -> 365 | pass |
| f2x-cost | F2X | cpu_s | band [1.2, 5.0] | 130.6 -> 313.9 | pass |
| p90-split | P90 (invariant) | share_a | value-band [0.82, 0.97] | 0.9065 | pass |
| p90-cross | P90 | cross_share | down | 0.4841 -> 0.1644 | pass |
| p90-shared | P90 | reports_shared | down | 426 -> 98 | pass |
| camp-shared | CAMP (invariant) | reports_shared | zero | 0 | pass |
| camp-split | CAMP (invariant) | share_a | eq 1.0 | 1 | pass |
| pen0r0-uploads | PEN0R0 (invariant) | reports_uploaded_cellular | zero | 0 | pass |
| pen0r0-relay | PEN0R0 (invariant) | reports_uploaded_relay | zero | 0 | pass |
| pen0r0-revocations | PEN0R0 | cases_opened | down | 2 -> 0 | pass |
| pen0r0-topups | PEN0R0 | topups_completed | down | 14 -> 0 | pass |
| slow-verify | SLOW | verify_cost | up | 0.03376 -> 0.1689 | pass |
| pqh-cert | PQH | air_bytes_security | up | 1.637e+07 -> 1.505e+08 | pass |
| pqh-cbr | PQH | cbr | up | 0.01171 -> 0.0493 | pass |
| p60-changes | P60 | pseudonym_changes | band [2.5, 6.0] | 111 -> 330 | pass |
| p60-p2pcd | P60 | p2pcd_requests | up | 26.44 -> 31.95 | pass |
| pen0-relay | PEN0 | reports_uploaded_relay | up | 0 -> 1523 | pass |
| ondec-latency | ONDEC | revocation_latency_h | down | 53.02 -> 31.98 | pass |
| a450-share | A450 | full_cert_share | band [1.6, 2.6] | 0.1006 -> 0.2005 | pass |
| conc-spread | L0N (invariant) | decision_spread_h | value-band [0.0, 0.5] | 2.614e-05 | fixed |
| conc-board-published | L0N (invariant) | first_case_crl_published_s | value-band [191399, 191401] | 1.914e+05 | fixed |
| conc-board-reports | L0N (invariant) | first_case_reports_in_s | value-band [86000, 86700] | 8.662e+04 | fixed |
| conc-same-base | L0N | reports_at_ma | same | 897 -> 897 | pass |
| conc-first-latency | L0N | revocation_latency_h | same | 53.02 -> 53.02 | pass |
| conc-one-publication | L0N (invariant) | crl_publications | eq 1 | 1 | pass |
| conc-att-waiting | ATT20N (invariant) | decisions_waiting | zero | 0 | pass |
| conc-att-spread | ATT20N (invariant) | decision_spread_h | value-band [0.0, 8.5] | 3.397e-05 | pass |
| conc-att-revoked | ATT20N | revoked_attackers | up | 6 -> 10 | pass |
| conc-att-seeds | ATT20N (invariant) | seeds_per_case | value-band [2.0, 4.0] | 2 | pass |
| conc-att-honest | ATT20N (invariant) | revoked_honest | zero | 0 | pass |
| conc-att-cost | ATT20N | records | band [0.98, 1.05] | 6738586 -> 6739404 | pass |
| aca-control | ATT20N (invariant) | aca2_control | positive | 3 | pass |
| aca-control-search | ATT20N (invariant) | aca2_found_ratio | value-band [1.0, 1.0] | 1 | pass |
| aca-blind | ATT20N (invariant) | aca2_leaks | zero | 0 | pass |
| aca-inject | ATT20N (invariant) | aca2_injected_leaks | positive | 1 | pass |
| aca-signers | ATT20N (invariant) | aca2_enrolment_signed | zero | 0 | pass |
| ond-verify | OND | verify_rate | down | 756.2 -> 46.3 | pass |
| ond-reports | OND | reports_at_ma | same | 897 -> 79 | wrong-expectation |
| ond-topups | OND | topups_completed | same | 14 -> 14 | pass |
| replay-reports | REPLAY | reports_at_ma | down | 897 -> 1101 | wrong-expectation |
| replay-honest | REPLAY (invariant) | revoked_honest | zero | 0 | pass |
| replay-topups | REPLAY | topups_completed | same | 14 -> 14 | pass |
| rsu5-broadcasts | RSU5 | crl_broadcasts | band [2.0, 6.0] | 60 -> 300 | pass |
| rsu5-cellular | RSU5 | reports_uploaded_cellular | same | 1564 -> 1707 | pass |
| rsu5-topups | RSU5 | topups_completed | same | 14 -> 14 | pass |
| rsu5-cost | RSU5 | cpu_s | band [0.95, 1.3] | 144.5 -> 163 | pass |
| aall-unknown | AALL | unknown_signer_loss | down | 0.006811 -> 0 | pass |
| a450-unknown | A450 | unknown_signer_loss | down | 0.006811 -> 0.00364 | pass |
| p60-unknown | P60 | unknown_signer_loss | up | 0.006811 -> 0.008619 | pass |

## Open items (documented, not fixed this round)

- **Verify-on-demand blinds misbehaviour detection** (row `ond-reports`).
  - This is realistic, given that a report must carry authentic evidence. It is also a
    weakness an attacker benefits from.
  - Whether a deployment should verify a message before reporting on it, and whether
    detection may run on unverified receptions, is a policy question for the owner. It
    is not an engine defect.
- **10 decisions for 19 reported attackers** (ATT20/ATT20N). The corroboration rule
  (`threat/ma/corroborated`) decided only about half of the attackers it heard about
  within the 54 h. That rule's thresholds are outside this package.
- **Reports without a signing certificate** (round 1). In L0, 94 of 1,672 reports went
  unsecured because the reporter had no active pseudonym certificate. This is probably
  the known rollover defect, seen from the reporter's side.
- **A replay with a fresh timestamp is not modelled.** The legacy DataReplay stamps the
  replayed generation time, so the freshness check catches it. A replay attack that
  re-signs old content with a current time would test the plausibility bounds instead.
- **ACA-blindness coverage.** The recording keeps the plaintext of only a few EE → RA
  PDUs, so the whole-certificate search covers one device. The HashedId8 search
  covers all 128 enrolments.
- **Settings the sweep cannot reach** (round 1). None of these is a published sweep
  setting:
  - the `security.protocol.params.*` keys;
  - `threats.attackers[].fraction`;
  - `net.uu.params.penetration`;
  - the `security.scms.deployment.providers[]` fields.

  This table is the only systematic check of them.
- **The shared target directory.** Another worktree's `cargo build -p v2xw-cli`
  overwrote `target/agents/debug/v2xw` between this round's build and its copy. Build
  and copy inside a single `heavy.sh` hold (the round's `t4.sh` does this).

## How to reproduce

1. Write each variant as a full scenario (the YAML's `runs`), on top of `sc/L0.yaml`.
2. Run each one with `v2xw run <variant> -o <dir> --build-utc 2026-10-18T00:00:00Z`
   through the run lock. Add `--no-verify` only for the release binary.
3. Summarise each run's PKI records.
4. Run `tools/pki_view/aca_blindness.py <dir>` and then
   `tools/pki_view/aca_blindness.py <dir> --inject`, both with the oracle venv
   (`python -I`).
5. Judge every row against its expectation.
