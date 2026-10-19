# SCMS settings audit: every knob, and proof that it works

Test method: for each knob, the expectation was written down before the run. It covers
(a) the metrics, (b) the run's cost and (c) the SCMS architecture as the PKI records show
it. Then the base and the variant ran on the same seed and were compared. The inventory of
every knob is `docs/guides/config-inventory-scms.md`.

- Fast checks: `crates/v2xw-engine/tests/scms_knobs.rs`. Each field reaches the model
  exactly as its `security.protocol.params` key does, the loader refuses bad values, and
  the published defaults are the model's own.
- Paired runs: `crates/v2xw-engine/tests/scms_knobs_runs.rs`. Each test prints one `KNOB`
  line with the numbers below.
- Expectation rows: `crates/v2xw-engine/tests/expectations/scms-knobs.yaml`, on the new
  `scms` sensitivity family.

**Base.** `scenarios/credential-lifecycle.yaml`, unchanged: 300 s, i-period 60 s,
5 certificates a period, 2 periods preloaded, 5 % ConstPos attackers from 60 s, automatic
MA, CRL every 30 s, cellular CRL fetch every 30 s, 30 s report shuffle, two providers
(`us-production-2025`).

**Cost.** The release binary from main d3039b53 ran the base in 226 s wall, with a
209 MB peak footprint and 4,075,407 events. The engine-test build ran it in 209 to 262 s.
The two builds' run reports are identical field for field: 950 certificates topped up,
190 of 274 top-ups done, 26 re-enrolments, 4 cases, 116 CRL downloads, 193 pseudonym
changes. So the typed-settings refactor leaves the default path unchanged.

## Results

"First expectation wrong" means the measurement showed the expectation to be wrong. The
row then gives the corrected expectation and the evidence for it. Each test asserts the
corrected expectation.

| Change | Expected (source) | Measured, base → variant | Verdict |
|---|---|---|---|
| `certs_per_period` 5 → 10 | twice the certificates per top-up; the same top-ups (CAMP-EE §2.1.5.3.2) | certificates topped up 950 → 1,900 (×2.000); top-ups 190 → 190; backend bytes 8.03 → 10.00 MB; events unchanged | pass |
| `topup_below_periods` 1 → 0 | no top-up, so the fleet starves after its two-period preload | top-ups 274 started → 0; vehicles starved 2 → 62; events 4.08 M → 2.15 M (starved vehicles cannot sign) and wall time 209 → 160 s | pass |
| `cert_shuffle_threshold` 1000 → 1 (window 5 → 600 s too) | first expectation wrong: shorter top-ups | identical run, event for event (4,075,407) | **finding**, see below; published as partial |
| `download_poll_interval_s` 2 → 20, ACA on time | longer top-up | identical (2,342 ms mean top-up) | first expectation wrong: `nextDlTime` is exact, so the first poll always succeeds |
| `download_poll_interval_s` 2 → 20, ACA 4 s slow | a late batch waits for the next poll | mean top-up 10,237 → 15,637 ms; topup-flow messages 6,260 → 3,960 (fewer repeat polls) | pass |
| `max_download_polls` 100 → 1, ACA on time | fewer completed top-ups | identical | first expectation wrong: as above |
| `max_download_polls` 100 → 1, ACA 4 s slow | the one poll finds nothing and the device gives up | top-ups done 188 → 104; vehicles starved 2 → 30; events 4.08 M → 3.24 M | pass |
| `enrolment_lifetime_s` 600 → 200 | more re-enrolments (SM-REENR) | 26 → 66 | pass. First expectation wrong: "none in the base". The base's 600 s enrolments began at the pre-run bootstrap |
| `pseudonym_change.period_s` 300 → 30 | first expectation wrong: ten times the changes | 193 → 547 (×2.8) | pass at "more than ×2". The base already changes at every 60 s i-period rollover |
| `report_shuffle_window_s` 30 → 5 | the first decision comes earlier (SCMS-765) | first decision 141.0 → 139.2 s; CRL downloads 116 → 90 | pass. First instrument wrong: `revocation_latency_ns` starts at the triggering report, 15.6 s in both runs |
| `ma_review_s` 0 → 60 | revocation later by about the review | first revocation 15.6 → 75.6 s; CRL entries 4 → 3 | pass |
| `crl_cadence_s` 30 → 0 | publishing on decision is faster | first revocation 15.6 → 6.6 s; CRL downloads 116 → 131 | pass |
| `crl_fetch_interval_s` 30 → 120 | about a quarter of the downloads | 116 → 40 | pass |
| `ma_max_open_cases` 1000 → 1, 20 % attackers, 60 s review | fewer CRL entries by the end | before the fix: identical (6 → 6) | **bug fixed**, see below |
| `security.scms.tls` 1.3 → 1.2, packet-level LTE | one more round trip per session; first expectation wrong: fewer bytes, from the payload sizes alone | handshake bytes 6,184 → 6,316 (+2 %: two more flights, each with its own headers); mean top-up 1,376 → 1,388 ms | pass |
| `deployment.preset` → `camp-reference` | one SCMS instead of two providers | devices by provider {a: 65, b: 60} → {a: 125}; no MA-to-MA sharing; backend bytes 8.03 → 6.65 MB | pass |
| Architecture of the base (PKI records) | devices talk only to the RA (CAMP-EE; 03 §3-9); RA ↔ ACA, LA1, LA2; MA → ACA, LA1, LA2, CRLG; the LAs never talk to each other or to a device | as expected, apart from the two record defects listed under "Open" | pass |
| "enforced by n of N" on `scms-conformance.yaml` | N never falls and n ≤ N | both cases: 40 of 41, every stamp n ≤ N, N non-decreasing | **bug fixed** |

## What was fixed

1. **"Enforced by n of N" read "88 of 74".** N was `self.nodes.len()` at the view, so it
   fell as vehicles drove away. `Phase2::retire` now records when each device leaves.
   N is the number of devices present at some instant since the CRL's `Published`
   stage: those still present, plus those that left at or after publication. The
   conformance run now reads 40 of 41.
   - Tests: `pki_view.rs` (N never falls) and
     `scms_knobs_runs.rs::every_enforcement_count_is_out_of_the_devices_it_could_have_reached`.
2. **The MA ran one case at a time, whatever its capacity.** `Phase2::start_cases_of`
   stopped at the first open case, so `MaPolicy::max_open_cases` (1,000, the model card's
   assumption) never applied, and an MA under attack took its decisions one by one. It
   now opens cases until the run refuses one at the cap.
   - Evidence: before the fix, the busy-MA pair was identical (6 → 6 CRL entries).
   - Test: `an_ma_with_one_open_case_revokes_fewer_in_the_run`.
   - This changes every run in which two decisions overlap. The SCMS regression digests
     need re-blessing at merge.
3. **The lifecycle knobs had no settings surface.** They are now typed, validated and
   published fields under `security.scms.lifecycle`. The settings window shows them under
   a new "Certificates & SCMS" section. Three are new to scenarios: `max_download_polls`,
   `download_request_tolerance_s` and `ma_max_open_cases`. `security.scms.tls` is new too.

## Faults injected, to prove the checks can fail

The faults were applied together by a scratch script, which saved each file's bytes and
restored them with a fresh mtime, as `tools/mutate/mutate.py` does. The targeted tests
then ran once.

| Fault | Test that went red |
|---|---|
| F1: N back to the devices present at the view | `every_enforcement_count_is_out_of_the_devices_it_could_have_reached` (conformance run). `pki_view.rs`'s new N-never-falls check stayed green: its 150 s Manhattan run loses no vehicle after a publication, so it is a guard, not the detector |
| F2: `certs_per_period` dropped from `LifecycleSpec::entries` | `every_lifecycle_field_moves_the_model_exactly_as_its_protocol_param_does`, `the_typed_field_wins_over_the_protocol_param` |
| F3: lifecycle defaults not published | `every_lifecycle_field_is_published_with_words_a_default_and_a_level` |
| F4: `scms_lifecycle` validation not called | `the_loader_refuses_what_the_model_cannot_run` |
| F6: TLS 1.2 never selected | `tls_1_2_costs_more_handshake_and_a_slower_topup` |
| F7: the MA back to one case at a time | `an_ma_with_one_open_case_revokes_fewer_in_the_run`: the run before the fix is the fault run (6 → 6) |

## Findings, left as they are, with the reason

- **The request shuffle has no in-run effect.** A device sends one butterfly request, at
  bootstrap. In a run that happens before the vehicle joins (`ScmsRun::preload`), with the
  window collapsed. Every later week is the RA's pre-generation from the same keys, which
  goes to the ACA at the period boundary without waiting in the request shuffle (CAMP
  Primer p.6; `provision.rs`, `ready && pregen`). This is the deployment's design, so
  `cert_shuffle_threshold` and `cert_shuffle_window_s` are published as partial, with the
  reason, and are Advanced. The test `the_request_shuffle_does_not_touch_in_run_topups`
  goes red the day a run makes a first request in-run.
- **`download-minWait` and `download-maxReqs` act only on a late batch.** The model's
  `nextDlTime` is the latest the shuffle can close, so the poll at that instant succeeds.
  The knobs move the run only when the backend is slow or down. The tests use a 4 s slow
  ACA.

## Open

- **The LOP is recorded when it is not deployed.** Under `us-production-2025` the PKI
  records show ee → lop → ra. The hop is transparent (zero latency, no queue,
  `ScmsRun::new_deployed`), so timing is right, but the architecture the records show is
  CAMP's, not the US deployment's. Fix: in `pki_view::drain_flows`, fold a transparent
  LOP hop into ee ↔ ra.
- **A shared report shows up as a device talking to the MA.** The other provider's MA
  shares a report with this MA (`shared_report_at_ma`). That MA's node is not in this
  run's id map, so `pki_view::node_id` names it `ee/<n>`. Fix: add the peer providers'
  components to `component_ids`.
- **The two-week overdue-CRL tolerance and the validated-certificate cache (4,096) are
  fixed constants in `v2xw-node`.** Exposing them needs a path through the node runtime.
  See the inventory.
- **The Studio's new section is checked by its unit tests, not on screen.**
  `settings-sections.test.ts`: 23 tests pass, including the new section and its
  sub-headings. A screenshot against a live engine is UI work for E2/E3.
