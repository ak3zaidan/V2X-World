# Validation reports

Index of every report in this folder, newest first. Counts are rows (or checks) as each
report states them: **pass**, **fixed** (a defect fixed at its cause, pinned by a test),
**corrected** (a wrong expectation, corrected with evidence), **open** (documented, not
fixed). "Not counted" means the report gives measurements without a row tally; read it for
the figures. SCMS validation lives in `docs/design/scms-real/` and `testing/`, not here.

| Report | Date | Pass | Fixed | Corrected | Open | Notes |
|---|---|---|---|---|---|---|
| [platform-2026-10-24.md](platform-2026-10-24.md) | 2026-10-24/25 | 24 | 6 | 2 | 9 | rows in `platform-validation.yaml`; CBR window aliasing (CW.cbr-window) open |
| [radio-2026-10-24.md](radio-2026-10-24.md) | 2026-10-24 | 31 | 1 | 1 | 5 | rows in `radio-validation.yaml`; open = 2 NR selection-window rows FAIL (fix `877d530c` reverted at merge, on `wip/radio-sidelink-nojitter`), 3 fault runs pending |
| [traffic-2026-10-24.md](traffic-2026-10-24.md) | 2026-10-24/25 | 8 | 2 | 4 | 9 | rows in `traffic-validation.yaml` (1 fixed in part, 1 inconclusive counted open); GreenYield queue-head hold open |
| [sensitivity-radio.md](sensitivity-radio.md) | 2026-10-12, upd. 10-22 | 24 | - | - | - | re-stated rows 24/24 on `77cffb5b` |
| [network.md](network.md) | 2026-10-06, upd. 10-23 | 44 | - | - | 1 | 45 checks, each proved able to fail |
| [radio-fidelity.md](radio-fidelity.md) | 2026-10-07, upd. 10-23 | not counted | | | | channel, receiver and MAC against Veins, OpenCV2X and the literature |
| [foundations.md](foundations.md) | 2026-10-07, upd. 10-18 | not counted | | | | `foundations.rs` suite and `settings_audit` |
| [sensitivity-expectations.md](sensitivity-expectations.md) | 2026-10-13 | 310 | | | 11 | 321 expectation rows on `08e71fb` |
| [sensitivity-catalogue.md](sensitivity-catalogue.md) | 2026-10-13 | not counted | | | | every published setting at low/high |
| [test-security.md](test-security.md) | 2026-10-13 | 36 | | | 5 | first run, 41 rows |
| [test-traffic-flow.md](test-traffic-flow.md) | 2026-10-13 | 29 | | 6 | 12 | 47 rows |
| [test-mac-congestion.md](test-mac-congestion.md) | 2026-10-13 | not counted | | | | rows in `mac-congestion.yaml` |
| [test-networking.md](test-networking.md) | 2026-10-13 | not counted | | | | rows in `networking.yaml` |
| [test-phy-channel.md](test-phy-channel.md) | 2026-10-13 | not counted | | | | rows in `phy-channel.yaml` |
| [test-vru-safety.md](test-vru-safety.md) | 2026-10-13 | not counted | | | | rows in `vru-safety.yaml` |
| [test-coupling.md](test-coupling.md) | 2026-10-13 | not counted | | | | rows in `interactions/coupling.yaml` |
| [test-run-cost.md](test-run-cost.md) | 2026-10-13 | not counted | | | | rows in `run-cost.yaml` |
| [agent-harness-live-1.md](agent-harness-live-1.md), [-2](agent-harness-live-2.md), [-3](agent-harness-live-3.md) | 2026-10-13 | scored | | | | agent harness evals, four criteria per case |
| [sensitivity-security.md](sensitivity-security.md) | 2026-10-11, upd. 10-12 | not counted | | | | |
| [sensitivity-traffic.md](sensitivity-traffic.md) | 2026-10-11, upd. 10-12 | not counted | | | | |
| [cross-validation.md](cross-validation.md) | 2026-10-07, upd. 10-13 | not counted | | | | against SUMO, 6 cases: flow -9 to +1 %, >= 97 % edges GEH < 5 |
| [traffic.md](traffic.md) | 2026-10-07, upd. 10-13 | 126 | | | 31 | 23 checks; held figures: 17 more pass but not proved |
| [traffic-calibration.md](traffic-calibration.md) | 2026-10-07 | not counted | | | | fundamental diagrams, waves, headways |
| [agent-eval.md](agent-eval.md), [agent-eval-before.md](agent-eval-before.md) | 2026-10-07 | scored | | | | agent evals before/after |

The `.json` files beside the agent reports are their raw scores.
