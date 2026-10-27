# Validation reports

Index of every report in this folder, newest first. Counts are rows (or checks) as each
report states them: **pass**, **fixed** (a defect fixed at its cause, pinned by a test),
**corrected** (a wrong expectation, corrected with evidence), **open** (documented, not
fixed). "Not counted" means the report gives measurements without a row tally; read it for
the figures. SCMS validation lives in `docs/design/scms-real/` and `testing/`, not here.

| Report | Date | Pass | Fixed | Corrected | Open | Notes |
|---|---|---|---|---|---|---|
| [live-scenarios-2026-10-25.md](live-scenarios-2026-10-25.md) | 2026-10-25 | 28 | 0 | 5 | 9 | rows in `live-scenarios.yaml`; 115 runs, 117 scenarios not run |
| [live-scms-2026-10-25.md](live-scms-2026-10-25.md) | 2026-10-25 | 36 | 0 | 1 | 3 | rows in `live-scms.yaml`; 68 pending; R-P2PCD-ASYM, R-KEMFILE open |
| [live-atk-2026-10-25.md](live-atk-2026-10-25.md) | 2026-10-25 | 107 | 0 | 5 | 10 | rows in `live-atk.yaml`; 54 pending; Sybil onset fixed `9cdb1639` |
| [live-rsu-2026-10-25.md](live-rsu-2026-10-25.md) | 2026-10-25 | 14 | 0 | 3 | 3 | rows in `live-rsu.yaml`; 54 pending; RSU CRL install fixed `4399f8f8` |
| [live-chains-2026-10-25.md](live-chains-2026-10-25.md) | 2026-10-25 | 54 | 0 | 3 | 16 | rows in `live-chains.yaml`; 21 not run |
| [live-studio-2026-10-25.md](live-studio-2026-10-25.md) | 2026-10-25, 2026-10-26 | 32 | 2 | 7 | 10 | rows in `live-studio.yaml` (59); compare peak pooling fixed `ce532c6e`, thin-window pooling fixed `dfa0e13e`; 8 rows pending (s3b, soak, lifecycle) |
| [platform-2026-10-24.md](platform-2026-10-24.md) | 2026-10-24/25, upd. 10-26 | 24 | 8 | 3 | 14 | rows in `platform-validation.yaml`; third session fixed CBR window share (`9de6e99e`) and MAP crosswalk lanes (`38230208`); open includes DCC x density interaction red (item 18) |
| [radio-2026-10-24.md](radio-2026-10-24.md) | 2026-10-24, upd. 10-26 | 31 | 3 | 1 | 2 | rows in `radio-validation.yaml`; §6: PC5 no-jitter fix landed (`454b4372`), sensing per TS 36.213/38.214 (`53b9aed8`), both NR selection-window rows fixed; open = fault runs for RV-PER-NIST, RV-BACKHAUL-CELL |
| [traffic-2026-10-24.md](traffic-2026-10-24.md) | 2026-10-24/25, upd. 10-26 | 8 | 2 | 4 | 9 | rows in `traffic-validation.yaml` (1 fixed in part, 1 inconclusive counted open); §5 diagnoses the GreenYield hold; its fix `455f0fcd` was reverted at merge (on `wip/traffic-permitted-left`) |
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
