# The full-scale benchmark to the end (after finalize 4)

Expectations: `crates/v2xw-engine/tests/expectations/scms-full-bench.yaml`, written before
each run (the FB3 rows were committed with `measured: pending`, `dc1e6d97`). Binary: an
opt-level-2 debug build of `fe3757b9` (main after the TESTS, FF and MEM merges), copied to
the scratchpad. Seed `0xBE7C_0001`, `--record-level metrics --no-verify`, 4 rayon threads,
one run per run-lock call, nothing else heavy on the machine during FB3.

## 1. Runs

| Id | Scenario | Result | Wall / CPU / peak |
|---|---|---|---|
| D1 | `scms-conformance.yaml` (15 actors, 433,200 s) | complete | 245 s / 284 s / 56 MB |
| QD | `real-world/midtown-scms-benchmark-quarter.yaml` (172,800 s) | complete | 189 s / 280 s / 227 MB |
| FB3 | `real-world/midtown-scms-benchmark.yaml` as committed (full fleet, 172,800 s), `--checkpoint-every 1h` | **complete, rc 0** | 17,381 s (2,126 s of it machine sleep) / 24,786 s / 2,105 MB |

## 2. R-FFSLOW: the fast-forward is no longer the long pole

| Change | Expected (before) | Measured | Verdict |
|---|---|---|---|
| D1, conformance on fe3757b9 against new3 | one stop per 0.1 s step from ~191,000 s, ~2.4 M backend ticks, one held cause | **8,310** ticks: crl-poll 2,259, 60 s max-jump 6,024, kernel 23, governance 1, crl-feed 1 | wrong expectation: the defect does not reproduce |
| QD, quarter end to end against R-Q | ≥ 1 stop per simulated second in the second window; 0.03–0.05 s a stop | **11,393** ticks in the whole run (R-Q 1,119,908): crl-poll 11,239, max-jump 144, kernel 8. About 3.5 ms a stop at 327 nodes; the whole file 189 s | wrong expectation |

Every stop now has a reason the real system has: an hourly conditional CRL poll by a
cellular device (`CRL_FETCH_INTERVAL_S`, 80 % of 303 devices × 48 h = 11,600; measured
11,291 polls, all answered 412), a backend kernel event, or the 60 s jump cap. No cause is
held at `now + 1`. Which commit after new3 removed the dense stepping was not bisected; the
`stops_by_cause` instrument (FF package) is what shows it is gone. No engine change was
needed for the benchmark.

## 3. The quarter fleet on current code (QD)

| Check | Expected | Measured | Verdict |
|---|---|---|---|
| Honest traffic | 0–5 cases, 0 honest revoked, no resume burst | 114 verdicts (implausibleAcceleration 110, positionJump 3, constantPositionFrozen 1); 50 reports decided, 37 uploaded, 13 not filed (10 RSUs hold no pseudonym certificate, 3 too few BSMs); 37 at the MAs; **0 cases, 0 revoked**; 0 teleports, removals or jams after the resume | pass |
| CRL and top-ups | hourly 412 polls; RSUs hold both t0 CRLs; one 20-certificate top-up per vehicle at 43,200 s; pool 20, then 40 | 11,291 polls answered 412, 0 downloads; 60 RSU installs (30 × 2); 278/278 top-ups, 5,560 certificates; pool 20.0 → 39.7 → 39.4 | pass |

## 4. FB3: the full-scale file to the end

The first full-scale run to reach 172,800 s. Every 10 s metric window, the run report and
the 30 s census are in the scratchpad (`fullbench/out-fb3`, not committed).

| Row | Expected (before) | Measured | Verdict |
|---|---|---|---|
| FB3.completes | rc 0 under 3.5 GB; first window 2–3 h, first fast-forward minutes, rollover 20–40 min, second fast-forward 10–30 min; peak 1.5–2.5 GB | rc 0; peak **2,105 MB** (FB2 3,109). Engine wall: 8,334 s, 148 s, **4,767 s**, **2,014 s** (69,076 CRL-poll stops at about 29 ms each). 15,255 s awake; the laptop slept 2,126 s on 1 % battery and the run carried on | open: the rollover window and the second fast-forward are slower than expected |
| FB3.traffic-radio | as FB2 | nodes 1,618 → 1,830; speed p50 0 from 200 s (gridlock); CBR mean 0.076–0.088, p95 0.14–0.17; PDR 100 m 0.83–0.84, 300 m 0.28–0.29; BSM e2e 12.4 ms (p99 15.0); 265 verifications per node per second | pass |
| FB3.honest-reports | 500–1,500 in the first window; 80–150 per 10 s at the rollover; no resume burst; 0–15 cases | **1,495** + **404**; fleet 1,830 → 1,891 across the resume; 1,447 at the MAs; **0 cases, 0 revocations**; 67 report-level false positives, 0 vehicles accused | pass |
| FB3.pki | ~944 RA requests in 240 s; one top-up per vehicle; ~74,000 hourly 412 polls; CRL only for a case | 938 home requests in 240 s; 1,773 of 1,800 top-ups (35,460 certificates); 70,946 polls answered 412; 0 CRLs issued | pass |
| FB3.privacy | no change counted in 60 s of gridlock | 0 changes, 0 link claims | pass |
| FB3.metric-window-after-ff | each live window reported at its end | the 230–240 s window is reported at 43,170 s, 43,220–43,230 s at 172,800 s, with rates over the frozen hours (verify_rate 115.7/s) and CBR 0 | **open (R-FFWIN)** |

### 4.1 What the full scale shows

1. **Honest traffic is not revoked at full density.** 2,083 detector verdicts in 300 s of
   live traffic, 2,001 reports, 1,447 at the two MAs, 0 cases. FB2's 71 honest cases were
   R-FFBURST's teleports.
2. **The MAs hear about misbehaviour a day late.** The report shuffle (CAMP-EE SCMS-765:
   10,000 reports or one day) never fills in a 2 km² box. Reports filed at 130–240 s reached
   the home MA at 86,430 s (census `scms.ma_reports` 0 → 1,067). An attacker's revocation
   therefore takes at least the day of shuffle, then the manual review, then the
   business-day CRL. That is a property of the specified system, and worth attacking.
3. **Honest report volume grows faster than the fleet.** Against QD it is 40× the reports
   (50 → 2,001) for 6.3× the devices and 3.3× the live time. The extra comes from density:
   constantPositionFrozen fires on standing vehicles (1 → 567), and there are more
   neighbours to judge each sender (implausibleAcceleration 110 → 1,249).
4. **The second fast-forward costs O(fleet) per stop.** 29 ms a stop at 1,945 nodes against
   3.5 ms at 327 nodes. The stops are real work (hourly polls), so the remaining fix is the
   event-indexed scan of fast-forward.md §5, part 2.

## 5. What was fixed

Nothing in the engine. R-FFSLOW did not reproduce on current code (§2), so the build budget
went to one optimised build and the full-scale run. One new open item, R-FFWIN (§4).
Carried open: the rollover window's cost (79 wall-s per simulated s, not profiled), the
O(fleet) stop cost, 27 incomplete top-ups (traced 2026-10-24: vehicles that left the map
with a top-up in flight, now `topups_left_area`; 06 §6 item 5), and 268 uploaded reports that never
reached an MA (1,715 uploaded, 1,447 received; not traced, possibly TESTS' open item that
`reports_uploaded_relay` counts on-air reports).
