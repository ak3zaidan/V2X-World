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

## 6. FB4: the full-scale file on `f4982788` (2026-10-27), and QD4

Expectations: rows `FB4.*` and `QD4.*` in `scms-full-bench.yaml`, committed before each run
(`7369a0dc`, `65deb86f`). Each row now carries machine `checks`, judged by
`tools/realworld/fullbench.py` (`summarise` reads progress.jsonl, metrics.json, run-report.json
and the `/usr/bin/time -l` tail; `check --inject` moves every measured value just outside its
expectation and requires the check to go red: 0 of 33 QD4 checks could not fail). Binary:
the integrator's release build of `f4982788`, copied to the scratchpad. Seed `0xBE7C_0001`,
`--record-level metrics --no-verify --progress-every 30s`.

### 6.1 What changed since FB3, and what it should do (written before the runs)

| Change since `d90a6298` | Expected against FB3 | Row |
|---|---|---|
| Density rules on in the benchmark (`0588ae6c`: `standing_leader_stop`, `lane_bend_speed`) | implausibleAcceleration 1,249 → 500–1,000 (QD −28 %); constantPositionFrozen untouched; 0 cases | FB4.honest-verdicts |
| Relay confirmation, RFC 6298 retries (BACKEND) | uploaded = at the MAs within 2 % (FB3: 268 of 1,715 missing); lost ≤ 5 | FB4.reports-relay |
| RA report shuffle permuted (`58367e2d`) | counts and release instant unchanged | FB4.reports-relay |
| `topups_left_area` (`bdcac292`) | FB3's 27 incomplete counted as left the area; 0 unexplained | FB4.topups |
| RA shuffles certificate requests, one per message; weekly job as one population batch (`0d62f322`) | same certificates, more backend messages; its known wrong download time could delay top-ups | FB4.topups, FB4.pki |
| R-FFWIN (`Engine::next_metric_flush`) | windows at 240 s and 43,230 s at their own end; live verify rate in the 230–240 s window | FB4.metric-windows |
| Provisioning crypto on rayon, key-range `ops_of`, explicit-certificate memo | wall: first window 5,000–8,000 s, rollover 2,500–4,800 s, second fast-forward 700–2,100 s | FB4.completes, FB4.ff-stops |
| `mac.cbr` own-window share, unknown-signer cost, detection skips unverified | CBR mean as FB3, p95 ≤ 0.17; unverified share ≤ 7.1 % | FB4.traffic-radio |
| QD +9 % memory (QD25, untraced) | peak 1,900–2,450 MB under the 2,500 MB cap | FB4.completes |

### 6.2 FB4 did not finish in this session

FB4 started at 23:45 on the main run slot (4 rayon threads, `--checkpoint-every 1h`). The
machine was shared the whole time (load average 10–18 on 8 cores; other agents' builds and
simulations; 23 GB of pages in the compressor). The first window ran at **15, 33 and 61
wall-s per simulated second** for 0–30, 30–60 and 60–90 s (FB3 averaged 35 over 240 s on
an idle machine): 55 min for 90 simulated seconds, footprint 452 → 990 MB at 888 → 1,463
nodes. At that rate the 240 s window alone needed about four more hours, past this
session's end (a background run is killed when the agent reports).

A SIGTERM writes a checkpoint at the next simulated second (`signal.rs`), and one was
written at 96 s. But a checkpoint here is a **replay point** (`checkpoint.rs`: the engine's
state is not stored; `v2xw resume` re-runs the elapsed time with outputs off, documented at
about 0.6 of the original cost). On this machine the replay ran at the original speed (30 s
in 496 s). So splitting FB4 over sessions saves almost nothing. **FB4 needs one unbroken
run of about 5 h on a quiet machine, or a session allowed to outlive it.** Its rows stay
`pending`. A resume was started and stopped during its replay (21 min, 30 s replayed) to give the
slot back; the 96 s checkpoint is intact. The partial run's checkpoint and progress are in
the scratchpad (`fb4/out`). One thing the partial run does show: the full fleet files
reports inside the first window (1, 6 and 53 by 30, 60 and 90 s), unlike QD4, so FB4's MA
release should come about a day after roughly 30 s.

### 6.3 QD4: the quarter fleet end to end on `f4982788`

Tiny run slot (1 rayon thread, 450 MB cap), run beside FB4's replay.

| Row | Expected (before) | Measured | Verdict |
|---|---|---|---|
| QD4.honest | 0 cases, 0 honest revoked; 20–140 verdicts | 0 / 0; 80 verdicts (implausibleAcceleration 79, positionJump 1), identical to QD25; all 16 reports filed in the rollover window | pass |
| QD4.relay-reports | uploaded = at the MAs; 0 lost; MA receipt at 86,400–86,500 s | 12 = 12 (11 cellular, 1 relay), 0 lost; first MA receipt **129,600 s**, 86,400 s after the first filing | **wrong expectation**, corrected |
| QD4.topups | 278/278 of 20; pool 20 → ~40 | 278/278, 5,560 certificates, 0 left the area, 0 incomplete; pool 20.0 → 39.71 at 43,210 s | pass |
| QD4.pki | 11,291 polls all 412; 0 CRLs; 60 installs; PCA 40 + 20 per top-up (+ ≤ 20 per account) | 11,291 / 0 / 60; home PCA 8,880 = 40 × 149 + the top-ups + 2.9 per device | pass |
| QD4.metric-windows | windows at 60 and 43,215 s; live verify rate | both; frozen spans end at 43,185 and 172,800 s; 13,049/s and 19,247/s | pass |
| QD4.cost | stops 11,000–11,800; peak 220–275 MB; wall 180–400 s | 11,393 stops; **278.5 MB**; **446.6 s** (1 thread, contended) | open |
| QD4.privacy | 0 changes, 0 links | 0 / 0 | pass |

**The wrong expectation.** The RA arms its one-day shuffle timer at its first upload
(`v2xw-proto` `scms/run/report.rs`). FB3 filed reports in the first window, so its release
came at 86,430 s. With the density rules, QD files none in the first window: census
`p2.filed` is 0 at 60 s and 16 at 43,200 s. So the first upload falls in the rollover
window, and the MAs hear about it at 129,600 s, a day and a half after the start. The
check is now the delay from the first filing (86,370–86,460 s). FB4's row was corrected
the same way before FB4 reports.

**Backend load per entity, quarter fleet, against CAMP sizing** ([CAMP122] §6.3: one
provisioning request per vehicle life, then 20 certificates a week):

| Entity | QD4 (48 h, 333 devices; home 169) | Per device | CAMP | Reading |
|---|---|---|---|---|
| ECA | 169 home enrolments, 162 in the first 60 s | 1 | 1 in life | scenario-start burst |
| RA, provisioning | 149 butterfly requests, 142 in 60 s (2.4/s) | 1 | ≈ 0.54/s national design rate | 4× in one box: provisioning at entry is a modelling artefact |
| PCA | 8,880 home certificates | 40 at entry + 20 a week + 2.9 | 20 a week | the population batch also pre-generates for accounts that left the map |
| RA, top-ups | 278 of 278, 5,560 certificates, in the 10 s after 43,200 s; home kernel steps 0 → 7,626 in that minute; 0.95 MB backend in that window | 0.83 in 2 days | 1 batch file a week | as specified |
| RA, CRL polls | 11,291 conditional polls, all 412 | ≈ 34 in 48 h | at least weekly | the 1 h interval is an assumption inside CAMP's weekly bound |
| MA (×2) | 12 reports (9 shared to the home MA), 0 cases | 0.04 | — | released one day after the first upload |
| CRL Generator / Store | 2 versions (the t0 lists), 0 entries; 60 RSU installs | — | one-business-day SLA | nothing to revoke |

### 6.4 Fixed, corrected, open

- **Fixed in the engine:** nothing. No engine defect was found, and no golden moved.
- **Corrected with evidence:** QD4.relay-reports and FB4.reports-relay. The MA release is
  one day after the first upload, not one day after 0 s.
- **Open:**
  - FB4 itself (§6.2).
  - QD4.cost: peak footprint +12.5 % on QD25 (278.5 against 247.5 MB). It is a transient
    inside one 30 s progress slice, since the samples peak at 223 MB. Not attributed; a
    bisect of `8becf72a..f4982788` is owed. The same growth at full scale puts FB3's
    2,105 MB near 2,370 MB, under the 2,500 MB cap with little margin.
  - The wall timing (contended, 1 thread).
- **Not reached at full scale:** the acagroup fix's known wrong download time. It did not
  delay any QD4 top-up.
