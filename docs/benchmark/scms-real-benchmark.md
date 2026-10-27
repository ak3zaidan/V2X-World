# The realistic SCMS benchmark: Midtown at the evening rush

What the engine measures when everything is set to the real-world US default: a weekday rush
hour on a real map, the 2025 two-provider SCMS on the real weekly calendar, honest traffic
only. It is the baseline an attack study is compared against. Every number below names the
run it came from (§1.3), and each one is compared with a published or literature value
where one exists. A value that looks implausible is flagged and was investigated (§9).

**Status (2026-10-27).** §0000: FB4, the full-scale re-run on `f4982788`, did not finish on the shared machine (stopped at 96 s of the first window); the quarter fleet (QD4) was run end to end on the same code instead. **Status (2026-10-23, evening).** §000 has the end-of-run report: the full-scale file ran to 172,800 s on `fe3757b9` (after finalize 4). **Status (2026-10-23).** §00 has the full-scale run after fix wave 3: both live windows complete, stopped in the second fast-forward. **Status (2026-10-22).** §0 has the full-scale run on current code. It was stopped by the
memory cap at about 155 simulated seconds; its complete metric windows and a complete 40 s
run are reported there. §1–§10 are the 2026-10-18 report, unchanged.

**Status (2026-10-18).** The full-scale file could not be run to completion on the owner's
laptop while it was shared with other builds (§8). The numbers below come from:
- the full-scale file's first 10 simulated seconds (run cost, provisioning load, report
  load), and
- the quarter-fleet file run end to end (everything else).
The LTE-V2X and Portland variants, and the 2× and 4× fleet runs, were not run (§10 gives
their commands and the cost model that bounds them).

**Headline.** The honest benchmark found an engine defect with a security consequence
(§9.1): pedestrian handsets sign PSMs (PSID 0x27) under pseudonym certificates whose
`appPermissions` list only 0x20 and 0x26. Every receiver refuses every PSM, the
`signatureVerification` detector turns 97 % of all verdicts into reports against honest
pedestrians, the reports fill a quarter of the channel's airtime, and the MAs revoked 10
honest devices in two days. Until it is fixed, no misbehaviour, airtime or revocation
number from a scenario with VRU handsets is a real-world number.

## 0000. FB4 on `f4982788` (2026-10-27): not finished; the quarter fleet on current code

Method, expectations and the full tables: `docs/design/scms-real/testing/full-bench.md` §6
and the `FB4.*` / `QD4.*` rows of `crates/v2xw-engine/tests/expectations/scms-full-bench.yaml`.
All of them were written before the runs and are machine-judged by `tools/realworld/fullbench.py`.

**FB4 (full fleet, 172,800 s).** Started on the main run slot with 4 threads. On a machine
shared with other agents (load 10–18), the first window slowed from 15 to 61 wall-s per
simulated second as the fleet grew from 888 to 1,320 nodes. Footprint was 990 MB at 90 s.
The run would have needed about five more hours, and a run is killed when its session
ends. A checkpoint resumes by replaying the elapsed time, and the replay ran at full cost
here, so splitting the run across sessions saves almost nothing. Every FB4 number is
still owed. It needs one unbroken run of about 5 h on a quiet machine.

**QD4 (quarter fleet, 172,800 s, same code).**
- **Honest traffic:** 0 cases and 0 revocations. 80 detector verdicts, the same as on
  `8becf72a`, and no report filed in the first window (density rules).
- **Reports:** 12 uploaded (11 cellular, 1 relayed), and all 12 reached the MAs, 0 lost.
  This confirms the relay fix.
- **Top-ups:** 278 of 278, 20 certificates each, inside 10 s of the 43,200 s boundary.
  The pool went from 20 to 39.7.
- **CRLs:** 11,291 hourly CRL polls, all answered 412. 0 CRLs issued, 60 RSU installs.
- **Metric windows:** every live metric window is reported at its own end (R-FFWIN fix
  confirmed).
- **Run cost:** peak footprint 278.5 MB, +12.5 % on `8becf72a` (open, not attributed).

**What QD4 shows about the specified system.**
- **The day-long report shuffle starts at the RA's first upload.** With honest traffic
  that files nothing in the first window, the MAs first heard of a report at 129,600 s,
  36 h into the run. The CAMP-EE SCMS-765 one-day shuffle compounds with low report
  volume: the quieter the region, the later the first decision can start.
- **The RA's weekly job is one population batch.** It pre-generates a week of 20
  certificates for accounts that did not ask, here 146 weeks for about 124 home top-ups by
  fleet share, so about 20 accounts that had left the map (2.9 extra certificates per home
  device). That stays inside CAMP's 20 a week per
  account. It also means the PCA's weekly load follows the registered population, not the
  vehicles present.

| Entity (QD4, home provider) | Load | CAMP per device | Reading |
|---|---|---|---|
| ECA / RA provisioning | 169 enrolments; 142 butterfly requests in 60 s (2.4/s) | 1 in life (≈ 0.54/s national) | scenario-start burst |
| PCA | 8,880 certificates (40 at entry + 20 a week + 2.9) | 20 a week | as specified, plus the population batch |
| RA top-ups | 278/278 in 10 s; kernel steps 0 → 7,626 in that minute | 1 batch file a week | a synchronous boundary burst |
| RA CRL polls | 11,291 (≈ 34 per device in 48 h), all 412 | at least weekly | 1 h is an assumption inside the bound |
| MA (×2) | 12 reports, 0 cases | — | released a day after the first upload |

## 000. The full-scale run to the end on main `fe3757b9` (2026-10-23, after finalize 4)

**Status.** Complete, rc 0. The file as committed (full fleet, 172,800 s, seed
`0xBE7C_0001`) ran on an opt-level-2 build of `fe3757b9`, with 4 threads, a 3,500 MB cap and
`--checkpoint-every 1h`. Nothing else heavy ran on the machine. The expectations were
written before the run: `crates/v2xw-engine/tests/expectations/scms-full-bench.yaml`;
method and verdicts are in `docs/design/scms-real/testing/full-bench.md`. QD is the
quarter file run end to end on the same binary.

**Run cost.**

| | QD (quarter) | FB3 (full) |
|---|---|---|
| Wall | 189 s | 15,255 s awake (17,381 s, of which 2,126 s machine sleep) |
| CPU (user + sys) | 280 s | 24,786 s (6.9 CPU-h) |
| Peak footprint | 227 MB | **2,105 MB** (FB2: 3,109 MB, stopped at 47,340 s) |
| First live window | — | 8,334 s for 240 s (35 wall-s per simulated s) |
| First fast-forward | — | 148 s |
| Rollover window | — | 4,767 s for 60 s (79 wall-s per simulated s; not profiled) |
| Second fast-forward | about 1 s | 2,014 s: 69,076 stops, all hourly CRL polls, about 29 ms each |
| Fast-forward stops | 11,393 | 69,130 (R-FFSLOW's dense stepping is gone, testing/full-bench.md §2) |

**Every metric** (10 s windows; FB3's first window is 130–240 s, its rollover window
43,170–43,230 s; QD's are 40–60 s and 43,185–43,215 s):

| Quantity | QD live 1 / rollover | FB3 live 1 | FB3 rollover |
|---|---|---|---|
| Nodes (with 30 RSUs) | 327 | 1,618 at 150 s → 1,830 at 240 s | 1,830 → 1,891 → 1,945 (kept across the resume; FB2 fell to 1,281) |
| Speed mean / p50 | 4.11 / 3.71 m/s; 4.22 | 2.84 → 1.86 m/s; p50 0 from 200 s (gridlock) | 1.48 → 1.38 m/s; p50 0 |
| CBR ch 172 mean / p95 | 0.010 / 0.026; 0.016 / 0.040 | 0.076–0.088 / 0.14–0.17 | 0.072–0.085 / 0.14–0.18 |
| PDR within 100 m / 300 m | 0.858 / 0.300; 0.908 / 0.321 | 0.829–0.844 / 0.280–0.292 | 0.835–0.845 / 0.287–0.295 |
| Loss causes (whole run) | below sensitivity 99.1 % | below sensitivity 96.8 %, hidden terminal 2.6 %, half-duplex 0.43 %, collision 0.06 %, fading 0.03 % | — |
| BSM e2e mean / p99 | 12.39 / 14.89 ms | 12.39–12.44 / 14.94–14.97 ms | 12.42–12.48 / 14.97–15.07 ms |
| PSM e2e mean | 3.76 ms | 3.88 ms | — |
| Verifications per node per second | ~33 | 245 (150 s) → 265 (230 s); wait p99 0.28–0.29 ms | 276–279; wait p99 0.40–0.52 ms |
| Unverified share | 3.9 %; 2.4 % | 5.4–6.0 % | 6.8–7.1 % |
| Full-certificate share | 20.6 %; 17.4 % | 16.1–17.4 % | 17.5–18.3 % |
| AoI (100 m) | 79.9 ms; 74.3 | 119–176 ms | 164–195 ms |
| Pseudonym pool | 20; 39.7 | 20.0 | 20 → **39.9 at 43,210 s** (week 615 valid from the boundary) |
| Pseudonym changes / linkability | 0 / insufficient | 0 / insufficient | 0 / insufficient (no CERTCHG in 60 s of gridlock; the old week is valid to 46,800 s) |
| `ped_wait` mean / p99 | 0.63 / 8.2 s | 0.69–1.23 / 7.9–38 s | 0.82–1.24 / 11–32 s |
| Teleports / removals / jams | 0 | 0 | 0 |

The windows that end where a fast-forward starts (230–240 s, 43,220–43,230 s) are reported
at the next live step with their rates divided over the frozen hours (`verify_rate`
115.7/s, CBR 0). That is R-FFWIN (open); those two samples are left out above.

**Detection outcomes at full density (honest traffic only).**
- Detector verdicts: 2,083 in 300 s of live traffic: implausibleAcceleration 1,249,
  constantPositionFrozen 567, positionJump 267. QD had 114.
- Reports: 2,001 decided. 1,715 were uploaded (1,371 cellular, 344 relay) and 255 were not
  filed (212 with too few BSMs on hand, 43 RSUs holding no pseudonym certificate); 30
  unsent, 1 lost. 1,447 real SM-MBR reached the MAs, 726 of them shared between providers.
  268 uploads never reached an MA (not traced).
- Report timing: 1,495 in the first window and 404 in the rollover window, about 67 per
  10 s. There is no resume burst; FB2 had 4,876 in 10 s.
- **The MAs received the reports at 86,430 s, one day after the first.** The report shuffle
  (CAMP-EE SCMS-765: 10,000 reports or one day) never fills in this box.
- Outcome: 67 report-level false positives, **0 vehicles accused, 0 cases, 0
  revocations**, 0 CRL entries. FB2's 71 honest cases were R-FFBURST artefacts.
- Scaling: honest report volume is super-linear in density. The fleet is 6.3× QD's and the
  live time 3.3×, but there are 40× the reports. Standing vehicles trip
  constantPositionFrozen, and each sender has more neighbours to judge it.

**Backend load per entity against CAMP's sizing.** CAMP sizes a national deployment
([CAMP122] §6.3; 04 §4.1: one provisioning request per vehicle life, then one `.info` and
one batch file a week). So the comparison is per device.

| Entity | FB3 (48 h, 2,112 devices; home provider 1,040) | Per device | CAMP per device | Reading |
|---|---|---|---|---|
| ECA | 1,040 home enrolments, most in the first 240 s | 1 at entry | 1 in the vehicle's life, at the factory | Provisioning at entry is a scenario-start burst |
| RA, provisioning | 1,020 home butterfly requests, 938 in 240 s (3.9/s) | 1 (`ra-maxReqs` 1) | 1 in life; national design rate ≈ 0.54 req/s | One 2 km² box at its start is 7× the national design rate: modelling artefact |
| PCA | 58,460 home certificates (40 per device at entry + 20 per top-up) | 40 + 20 a week | 20 a week (`certs_per_i_period`) | As specified |
| LA (×2) | 1,020 home chains | 1 | 1 per request | As specified |
| RA, top-ups | 1,800 started, 1,773 completed (35,460 certificates), downloaded in the first fast-forward, valid from 43,200 s | 0.84 in 2 days | 1 batch file a week | As specified; 27 incomplete (not traced) |
| RA, weekly pre-generation | 57.8 MB of backend traffic in the 10 s after 43,200 s (home kernel steps 1,266 → 13,854 in that minute; by its timing the weekly pre-generation, not traced) | about 27 kB per account (2,112) | `ra-maxGenDelay` 1 week | One synchronous burst at the boundary: a load peak a real RA spreads or provisions for |
| RA, CRL polls | 70,946 conditional polls, all answered 412 (0.41/s) | about 42 in 48 h (hourly) | at least weekly | The engine's 1 h interval is an assumption inside CAMP's weekly bound (constants.md) |
| MA (×2) | 1,447 reports, 0 cases | 0.69 | — (no CAMP sizing) | Arrivals come once a day, by the shuffle timer |
| CRL Generator / Store | 2 versions (the t0 lists), 0 entries; 60 RSU installs | — | one-business-day CRL SLA | Nothing to revoke |

**Compared with the quarter fleet.** The fleet is 6.3×, and the per-node quantities that
grow with density all move. CBR is 7.5–8.6× (0.010 → 0.076–0.088). Verifications per node per
second are 8× (33 → 265). AoI doubles (80 → 120–195 ms) and the unverified share rises
(3.9 % → 5.4–7.1 %). PDR within 100 m falls by 1–7 points. BSM latency stays at 12.4 ms,
because the channel is far below J2945/1's `vMinCU` 0.5. The cost is 81× the wall and 89×
the CPU of QD, and 9.3× its memory. Both runs end with 0 cases and 0 revocations.

## 00. The full-scale run on main `e4004b4c` (2026-10-23, after fix wave 3)

**Status.** The full-scale file was run again on the release build of current code. It
used 4 rayon threads, a 3,500 MB cap and seed `0xBE7C_0001`; expectations and method are in
`docs/design/scms-real/testing/final-run.md` and
`crates/v2xw-engine/tests/expectations/scms-final-run.yaml`.
- Both live windows ran in full: 0–240 s and the 43,170–43,230 s rollover.
- The run stayed under the cap: peak 3,109 MB.
- It was stopped by hand at 47,340 s. The second fast-forward was running at about 7
  simulated seconds per wall second (§9.2, still open), about 5.4 more hours.
- So there is no end-of-run report. The numbers come from the 10 s metric windows and the
  10 s census.

| Quantity | Quarter (R-Q, live 1) | Full, first window (FB2, 130–240 s) | Full, rollover window (43,170–43,230 s) |
|---|---|---|---|
| Nodes on the map | about 350 | 1,632 at 150 s → 1,833 at 240 s; 1,933 devices provisioned (home provider 48.8 %) | 1,281 at the resume → 1,395 |
| Mean speed / p50 | 4.12 m/s | 2.75 → 1.82 m/s; **p50 0 from 160 s** (gridlock) | 4.1 → 2.8 m/s after the fleet thinned |
| CBR ch 172 mean / p95 | 0.011 / 0.027 | 0.073–0.087 / 0.13–0.17 | 0.066–0.081 / 0.17–0.20 |
| PDR within 100 m / 300 m | 0.838 / 0.290 | 0.833–0.843 / 0.280–0.290 | 0.846–0.853 / 0.297–0.308 |
| Loss causes (share of attempts) | below sensitivity 98.8 % of losses | below sensitivity 82.9 %, hidden terminal 2.4 %, rx overflow 0.35 %, half-duplex 0.36 % | similar; unknown-signer 1e-4 |
| Reception attempts | — | 3.1 M/s (150 s) → 3.7 M/s (230 s) | 2.0–2.3 M/s |
| BSM e2e mean / p99 | 12.2 ms / — | 12.39–12.42 ms / 14.94–14.96 | 12.42–12.46 / 14.96–15.07 |
| Verifications per node per second | ~30 | 240 (150 s) → 268 (230 s); queue p99 0; wait p99 0.28–0.31 ms | 2.7–3.2 × 10⁵ /s fleet-wide |
| Unverified share | — | 5.2 → 6.2 % | 5.4–7.1 % |
| Full-certificate share | 25.1 % | 16.1 → 17.7 % | 17.9–19.8 % |
| AoI | 79.3 ms | 119 → 173 ms | 104–114 ms |
| Pseudonym pool (`cert_pool_valid`) | 20 | 20 at every census | **20 → 39.8 within 10 s of 43,200 s** (top-ups) |
| Provisioning | 40 per device | 40.0 per device; home RA 944 requests in 240 s (3.9/s; CAMP design ≈ 0.5/s) | 629 top-ups (12,580 certificates) by the home provider within 10 s of the boundary |
| Misbehaviour reports | 36,024 (PSM defect) | **1,935 in 240 s, all honest; none reached an MA before the fast-forward** | **10,357 more in 60 s; MAs received 15,062; 71 cases; 71 honest vehicles falsely accused** |
| CRL Generator entries | 10 | 0 | 0 (through 47,340 s) |
| Pseudonym changes / linkability | 314 at the rollover / 0 | 0 / 0 of 0 | **0 counted at the rollover** / 0 of 0 |
| Footprint | 428 MB peak | 2,690 MB at 150 s (R-FS: 2,582 MB), 3,057 MB at 230 s | 3,109 MB at the resume, then 2.5–2.9 GB |

**Run cost.**
- The first live window took 7,113 s of wall time, about 30 wall-s per simulated second,
  rising to 45. It used about 2 of the 4 threads.
- The first fast-forward took 207 s; the rollover window took 1,855 s.
- When the run was stopped it had used 17,900 CPU-s (4.97 CPU-h).
- The wave-3 memory de-duplication does not show at full scale: the footprint at 150 s is
  the same as R-FS's.

**What the full scale shows that the quarter fleet did not** (each is open; details in
`testing/final-run.md` §4):
1. **R-FFHOLD.** Every report filed before the fast-forward stayed in flight for its 12 h.
   The MAs received them only at the 43,200 s i-period boundary.
2. **R-FFBURST.** In the 10 s after the resume, 4,876 honest reports were filed, against
   about 80 per 10 s live. Together with item 1 this gives 71 MA cases about honest
   vehicles. Under manual review these would be decided during the second fast-forward,
   which this run did not reach.
3. The rollover registers no pseudonym change (`pseudonym_change_rate` 0), and `ped_wait`
   counts the fast-forward as waiting (mean 3,465 s in one window).
4. The second fast-forward is slow: 6.7 simulated s per wall s, about 5.4 h at full scale.

None of these honest-report numbers is a real-world false-positive rate until items 1 and
2 are fixed.

## 0. The full-scale run on main `88e24b09` (2026-10-22)

**Status.** The full-scale file was run again, on current code, and again did not finish. It
was stopped by the machine's 2.5 GB memory cap, not by time: the engine's footprint grows
by about 1.4–2 MB for each road user on the map, and the transient peaks on top of that
reached 2,582 MB at about 155 simulated seconds, with 1,580 nodes on the map. So the full
fleet's live window, the rollover window and the two fast-forwards have no end-of-run
report. What exists at full scale (expectation rows and verdicts:
`crates/v2xw-engine/tests/expectations/scms-final-bench.yaml`; method and analysis:
`docs/design/scms-real/testing/final-bench.md`):

| Id | What | Result |
|---|---|---|
| R-FS | `midtown-scms-benchmark.yaml` as committed, opt-level-2 debug build of `88e24b09` (sha256 prefix `9bdf0275318dcbf8`), `--record-level metrics`, 2 rayon threads, seed `0xBE7C_0001` | stopped at about 155 of 172,800 s (79 min wall, about 6,300 CPU-s). The metric windows ending at 130, 140 and 150 s were already written and are complete. They are the first full-fleet radio, latency and traffic numbers after the fill |
| R-FS40 | the same file, 0–40 s, 10 s warm-up, census every 10 s | complete: 535 s wall, 623 CPU-s, 910 MB peak. The full-fleet run report for provisioning, reports and trust |

**Full fleet against the quarter fleet.** "Quarter" is §2–§6 below (R-Q, older build, before
the PSM, R-USFP, R-NOCRL and trust fixes). Rows that those fixes change are marked.

| Quantity | Quarter (R-Q, live 1) | Full (R-FS, 130–150 s) | Reading |
|---|---|---|---|
| Nodes on the map | about 350 | 1,580–1,632 (R-FS40 at 40 s: 1,000 = 389 vehicles, 581 handsets, 30 RSUs; provider split 496 / 504) | Vehicles pile up: the 4× fill puts 900 vehicles into a congested box |
| Mean speed | 4.12 m/s | 2.75 → 2.60 → 2.33 m/s; p50 0.82 → 0.18 m/s | Now in the band of Midtown's measured ~5 mph (2.2 m/s), and still falling at 150 s |
| Time headway | p50 2.67 s, mean 3.20 s | p50 2.94 s, mean 3.11 s | — |
| Pedestrian wait | mean 0.63 s, p99 8.2 s | mean 0.70–0.82 s, p99 14–19 s, max 119 s | Longer waits under the heavier traffic |
| CBR, ch 172 | mean 0.011, p95 0.027 | mean 0.074–0.078, p95 0.14–0.16, max 0.24 | 7× the quarter. Still below J2945/1 `vMinCU` 0.5, so no power adaptation |
| PDR within 100 m / 300 m | 0.838 / 0.290 | 0.84 / 0.28 | Unchanged. 80–100 m 0.66, 0–20 m 0.998 |
| Loss causes (of all attempts) | below-sensitivity 98.8 % of losses | below-sensitivity 83.8 %, hidden terminal 2.0 %, half-duplex 0.39 %, rx overflow 0.27 %, collision 0.04 % | Hidden terminals and receiver overflow appear at full density |
| Reception attempts | — | 2.96–3.06 M per simulated second | — |
| BSM end-to-end latency | mean 12.2 ms, p95 14.6 | mean 12.39 ms, p95 14.62, p99 14.94, max 214 ms | The fixed 9 ms CRATON2 sign plus a 2.5 ms handoff dominate (§9.4). MAC defer mean 0.05 ms |
| Verifications | 10,163/s fleet-wide, about 30 per node per second | 376–390 k/s fleet-wide, **about 240 per node per second**; queue p99 0, max 31; wait p99 0.28 ms | 8× per node. Still a tenth of CRATON2's > 2,500/s |
| Unverified share | — | 5.3 % (R-FS40: 89,536 of 2.1 M) | `prioritized` sheds under load (the scenario's own validator note) |
| Full-certificate share / P2PCD | 25.1 % / 387 per s | 16.1 % / 21,450 per s | Nearer J2945/1's 10 % once the fill is over |
| AoI / peak AoI | 79.3 ms / p50 112, p99 215 ms | 119–130 ms / p50 113, p99 414–510 ms | The tail doubles |
| Backend reachability | 0.963 | 0.961 | — |
| Valid pseudonyms in the pool | 20 | 20 at every census (0 starved) | — |
| MBR on the air | 24.5 % of airtime (PSM defect, fixed) | R-FS40: 43 MBR frames, 68.8 ms of 32.6 s airtime (**0.21 %**) | Fixed |
| Honest reports | 36,024 (PSM defect) | R-FS40: 229 reports about 32 honest devices in 40 s; verdicts `constantPositionFrozen` 165, `positionJump` 71; 0 cases, 0 revocations in the window | The residual false-positive rate (§0.1) |
| Receiver refusals | 48,123 `psid-not-permitted` | R-FS40: 0 refusals of any cause; 2,000 CRLs accepted; 60 RSU CRL installs | Trust and CRL fixes hold at scale |
| Privacy observer | 183 claims, 0 correct | R-FS40: **2,873 link claims, 0 correct, with 0 pseudonym changes** | Open (§0.1) |

**Run cost at full fleet.**
- R-FS: 6,300 CPU-s and 79 min wall for about 155 simulated seconds (2 rayon threads, on a
  machine mostly idle after the first 10 minutes). That is 40 CPU-s per simulated second
  on average, rising with the fleet.
- R-FS40: 0–10 s took 182 s of wall time (provisioning 701 devices at entry), 10–20 s took
  70 s, 20–30 s 129 s and 30–40 s 150 s.
- Footprint by census: 450 MB at 701 nodes, 609 MB at 783, 817 MB at 888 and 868 MB at 992.
  R-FS: 795 MB at 13 min, 1,079 MB at 23 min, 1,457 MB at 43 min (peak 1,998), 1,418 MB at
  63 min (peak 2,225), then the cap at 2,582 MB.
- Where the time goes (a `sample` at 23 min): radio obstacle geometry (`segment_blocked`,
  `ring_crossing`, the building index) and the reception fan-out (`finish_rx_only`,
  `dsrc_outcomes`), then P-256 field arithmetic. A few per cent goes to `serde_json`
  serialising and parsing `NodeRxView` and `PhyRxView` records, even at record level
  `metrics`.
- To finish the file, the remaining 145 live s at about 60 CPU-s each, plus the 60 s
  rollover window, add roughly 3 CPU-hours. In all it is 4–5 CPU-hours, more than the
  2–3.3 the stress package estimated. Memory is the binding limit, though: at 2 MB a node,
  1,600 nodes plus the transient peaks need 3–4 GB.

### 0.1 Open items found at full scale

1. **Memory grows by about 2 MB per node on the map**, and transient peaks of up to
   +0.8 GB come on top. The engine's census shows no collection growing out of proportion
   (`p2.checked_at` 44,260 entries at 40 s; credentials 40 per device), so the bytes are
   inside per-node state the census does not size: detector suites, verification caches
   and the per-step reception fan-out. The next step is a heap profile.
2. **The passive observer claims links when no pseudonym changed.** In R-FS40: 3,120 link
   decisions, 2,873 claims, 0 correct, and `pseudonym_changes` 0 in 40 s. A link claim
   needs a change to link across, so the linker is either counting something else or
   acting on vehicles entering and leaving its coverage. §9.6 saw the same 0-correct
   pattern at the quarter fleet.
3. **Honest false positives at full density.** In 40 s the US SM-MBR suite reported 32
   honest devices (229 reports). Most verdicts were `constantPositionFrozen` (165 of 236). The likely
   cause is vehicles standing in queues, but that is not yet checked against the recording. The end-to-end consequence under manual review over the
   two days (cases, decisions, honest revocations) is unmeasured because R-FS did not
   finish. With a median speed of 0.18 m/s, this is the kind of false positive the benchmark is
   meant to expose.

Update (fix wave 3, `docs/design/scms-real/testing/measure.md`): items 2 and 3 are
root-caused and fixed in the engine, and neither benchmark number has been re-measured yet.

- Item 2 was a run-report defect. The 2,873 claims are links to cars newly heard, now
  `privacy_links_spurious`. `privacy_links_claimed` counts only claims across a change.
- Item 3 was 8 decelerating cars whose consecutive latency-compensated BSMs coincided,
  each heard by about 20 receivers. The smoothing receiver now propagates by the mean
  velocity over the epoch (`gnss/error/gauss-markov` 1.2.0).

## 1. What was run

### 1.1 The scenarios (`scenarios/real-world/`)

| File | What it is |
|---|---|
| `midtown-scms-benchmark.yaml` | The benchmark. Monday 2026-10-19 17:00 EDT. D7 Midtown box with buildings. 9,000 veh/h with the peak-load mix, 2,000 pedestrians and 200 cyclists (25 % with handsets). DSRC ch 172 under J2945/1. 30 RSUs on fibre. `us-production-2025` on the real calendar, manual MA, 1-business-day CRL. 5-minute pseudonym change. legacy-12 detectors. A passive observer at 10 % of the signals |
| `midtown-scms-benchmark-quarter.yaml` | The same with a quarter of the road users (2,250 veh/h, 500 pedestrians, 50 cyclists). The run quoted here |
| `midtown-scms-benchmark-lte.yaml` | The benchmark on LTE-V2X PC5 mode 4 (J3161/1). Not run |
| `portland-scms-benchmark.yaml` | Downtown Portland, OR, Monday 17:00 PDT, 3,000 veh/h, 12 RSUs. Not run |

The timeline, on the real calendar (i = 614 at t0; the week boundary is 09:00 UTC because
the grid is fixed weeks from 2015-01-06 04:00 EST):

| Simulated s | Window | What it shows |
|---|---|---|
| 0–240 | live (120 s warm-up, 4× demand for the first 90 s) | the rush hour: traffic, radio, verification, reports |
| 240–43,170 | fast-forward | the backend overnight: polls, shuffles, MA review |
| 43,170–43,230 | live | the rollover to i = 615 at 43,200 s on the air |
| 43,230–172,800 | fast-forward | Tuesday's business day and on to Wednesday 17:00 EDT: CRL SLA, fetches |

### 1.2 Settings that are assumptions

RSU density (30 in the box, from NYC's planned ~450 on Manhattan's avenues; 01 §8, as-built
count UNCONFIRMED), RSU backhaul medium (fibre; the NYC pilot's RSUs reach a TMC proxy over
IPv4, RSU LL p.4), 80 % of vehicles with a cellular modem, 25 % of pedestrians with a
handset, the observer's coverage (10 % of signals, 200 m), provider regions and fleet split
(D1), MA review 8 h (D3). Everything else is the deployment's default (constants.md).

### 1.3 Runs

All runs used the release binary built from `3a30f2c` (the code of `18f9291`; sha256 prefix
`11b2ef0eac95cbf2`), `--record-level metrics --no-verify`, seed `0xBE7C_0001`, through the
machine's run lock (`nice 10`). Outputs are in the session scratchpad and are not committed.

| Id | Scenario | Simulated | Result |
|---|---|---|---|
| R-FULL | `midtown-scms-benchmark.yaml` | 10 s of 172,800 (stopped) | census at 10 s; cost and profile |
| R-Q | `midtown-scms-benchmark-quarter.yaml` (0–60 s and 43,185–43,215 s live) | 172,800 s, complete (rc 0) | run report and metrics |

Windows: "live 1" is the R-Q sample ending at 50 s (the 40–50 s metric window, inside the
4× fill); "rollover" is the sample ending at 43,210 s; "end" is the whole-run report.

## 2. Traffic

| Quantity | R-Q value | Real-world / literature | Reading |
|---|---|---|---|
| Mean vehicle speed, live 1 | 4.12 m/s (9.2 mph); p50 3.63, p95 11.37 m/s | Midtown average travel speed about 5 mph (2017), below 5 mph (2023) [NYC DOT Mobility Report 2018, 2019; cited in `midtown-peak-load.yaml`] | Faster than Midtown, as expected with a quarter of the fleet: the quarter run is not congested. The full file is the one to compare; not measured (§8) |
| Time headway, live 1 | p50 2.67 s, mean 3.20 s | HCM 6th ed. base saturation flow 1,900 pc/h/ln, about 1.9 s discharge headway | Above saturation headway: unsaturated approaches |
| Pedestrian wait, live 1 | mean 0.63 s, p99 8.2 s, max 35.7 s | HCM 6th ed. Ch. 19: pedestrian delay at signals is tens of seconds for a 90–120 s cycle | Low; the `observed` behaviour crosses into gaps. Not checked further |
| Density / flow metrics | 0.78 veh/km, 11.6 veh/h (network averages) | — | Averages over every lane-km, so not comparable with an approach count |
| Control delay and HCM LOS | not measured | HCM 6th ed. Exhibit 19-8 (LOS A ≤ 10 s … F > 80 s per vehicle) | No per-approach control-delay metric is published at record level `metrics`; `v2xw check-traffic` needs a full recording |

## 3. Network (DSRC, ch 172, J2945/1)

**PDR against distance, live 1** (BSM, PSM and MBR receptions, 20 m bins):

| m | 0–20 | 20–40 | 40–60 | 60–80 | 80–100 | 100–120 | 140–160 | 180–200 | 240–260 | 300–320 | 400–420 | 480–500 | > 500 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| PDR | 0.994 | 0.978 | 0.883 | 0.845 | 0.698 | 0.591 | 0.324 | 0.185 | 0.097 | 0.063 | 0.039 | 0.030 | 0 (range cap) |

- PDR within 100 m is 0.838 (43,278 trials) and within 300 m 0.290.
- Losses: 98.8 % below sensitivity, 0.58 % half-duplex, 0.48 % hidden terminal, 0.08 % fading, 0.02 % collision.
- Against the literature: the engine's own WINNER+ B1 NLOS validation on the Manhattan grid gives 0.67 at 0–100 m and 0.28 at 100–200 m (`docs/validation/radio-fidelity.md` §3a). This run, with most pairs in line of sight along the avenues, sits above it.
- J2945/1 measures its PER over `vPERRange` = 100 m (CAMP, IEEE TVT 68(3) 2019, Table I). No published Midtown PDR curve was found.
- The 0 beyond 500 m is the scenario's `range.max_m` cap, not a measurement.

| Quantity | R-Q value | Reference | Reading |
|---|---|---|---|
| CBR, ch 172, live 1 | mean 0.011, p95 0.027, max 0.046 | J2945/1 power control acts from `vMinCU` 50 % to `vMaxCU` 80 % (CAMP 2019 Table I) | Far below the J2945/1 range at a quarter fleet; J2945/1 never adapted |
| E2E latency (BSM) | mean 12.2 ms, p50 12.3, p95 14.6, max 16.8 | J2945/1 and the FMVSS 150 NPRM budget a BSM within 100 ms | Within budget. Stages: sign 9.0 ms (fixed, §9.4), app→radio handoff 2.4 ms, verify 0.53 ms, airtime 0.30–0.49 ms, MAC access ≈ 0, propagation 1 µs |
| MBR latency stages | airtime 1.6 ms (1,162 B frame), RSU backhaul p50 19.3 ms, Uu p50 18.1 ms | — | — |
| AoI, live 1 | mean 79.3 ms; peak AoI p50 112 ms, p99 215 ms; PIR p50 0.10 s | 10 Hz BSM: a loss-free peak AoI is 100 ms plus latency (≈ 112 ms) | Consistent |
| MAC access delay | mean 0.05 ms, p99 1.7 ms; queue depth 0 | — | Idle channel |
| Overhead by layer (whole run) | PSDU 28.6 MB: payload 20.8 %, security 37.5 %, MAC header 9.6 %, LLC/SNAP 2.9 %, WSMP 1.8 %, FCS 1.5 %; 43.1 s airtime, 4.2 s of it preambles | — | Security bytes are 1.75× payload bytes (`envelope_overhead`) |
| Frame sizes | BSM PSDU mean 208 B; PSM 241 B; MBR 1,162 B | pseudonym certificate 80 B on air (CAMP Table 3, 04 §3) | Consistent with 04 §3 |
| Airtime by type (whole run) | BSM 28.0 s (65 %), **MBR 10.6 s (24.5 %)**, PSM 4.5 s (10.5 %) | — | Implausible: a quarter of the airtime is misbehaviour reports from honest traffic (§9.1) |
| Uu / backhaul / backend bytes | UL 26.9 MB, DL 2.8 MB; RSU backhaul 9.7 MB; backend 77.5 MB | — | Uplink dominated by reports |

## 4. Security processing (receivers)

| Quantity | R-Q value | Reference | Reading |
|---|---|---|---|
| Verifications, live 1 | 10,163 ECDSA-P256 verifies/s across the fleet (≈ 30 per node per second) | CRATON2 line-rate verifier > 2,500/s (Unex OBU-301E sheet, `obu-unex-obu-301-craton2.yaml`) | Two orders below capacity; verify-queue p99 0.09 ms, max 2.0 ms |
| First sight vs repeat | 7,188 end-entity and 9,160 CA-path validations vs 781,879 cache hits (0.9 % first sight). Verify stage: 0.40 ms repeat, 0.645 ms first sight (p95) | VCOST: 400 µs repeat, plus the priced SHA-256 and ECQV on first sight | As modelled. 243 cache evictions; 5,819 entries invalidated by CRLs |
| Unknown signer / P2PCD, live 1 | 386.6 P2PCD learning requests/s; full certificate in 25.1 % of signed frames | J2945/1: full certificate once a second at 10 Hz (10 %), digest otherwise; P2PCD per 1609.2 §8.2.4 | Above 10 % (§9.5) |
| Receiver refusals (whole run) | 837,190 SPDUs checked; **48,123 refused `psid-not-permitted`**; 1,832 tolerated RA-profile findings | — | Every refusal is an honest PSM (§9.1) |
| Verification states seen by detectors | Verified 112,582; **Invalid 45,875**; Unverified 11,009 | — | — |

## 5. SCMS backend

| Quantity | R-Q value | Reference | Reading |
|---|---|---|---|
| Devices by provider | provider-a 177, provider-b 172 (349: vehicles and 161 VRU handsets) | Seeded split (ASSUMPTION, D1) | — |
| Provisioning at entry | 40 certificates per device (2 weeks × 20), 1 request each | CAMP 1.2.2 Table 67 `certs_per_i_period` 20; `ra-maxReqs` 1 (04 §4.1) | As CAMP sizes it |
| Provisioning load (R-FULL, first 10 s) | 701 nodes and 26,920 certificates in 10 s of simulated time (the pedestrians' handsets and the fill arrive at once) | CAMP design rate ≈ 0.5 requests/s for 17 M vehicles a year (04 §4.1) | A scenario start is a burst no real RA sees: a real fleet is provisioned at the factory over years |
| Top-ups at the 614 → 615 rollover | 275 started, 273 completed, 5,460 certificates (20 each), 2 refused because the device was blocklisted | NYC weekly top-off; CAMP one `.info` and one file a week per device (04 §4.1) | One per device per week, as specified. The 2 refusals are revoked honest pedestrians (§9.1) |
| Top-up latency | pools grew at 43,290 s for a rollover at 43,200 s: ≤ 90 s from the boundary (30 s census granularity) | — | Inside the 1-hour certificate overlap |
| Certificate availability | `cert_pool_valid` 20 at live 1, mean 39.8 at the rollover, 39.5 at the end; 0 vehicles starved; 0 s without a valid certificate | — | No starvation |
| Backend reachability | 0.963 of attempts reached the backend | — | RSU-only devices out of RSU range |
| Re-enrolment | 1 started, 1 completed | 6-year lifetime, 18-month lead (CAMP 1.2.2) | One device was inside its lead window |
| Misbehaviour reports | 36,024 sent: 23,862 over Uu, 6,601 over the air to an RSU, 4,714 from RSUs. 31,963 reached the MAs, 15,809 shared between them, 116 lost, 304 unsent. 102 real SM-MBR, 31,823 unmapped, 38 unsecured | — | All about honest devices (§9.1) |
| MA cases and decisions | 12 cases and 128 revoke decisions, all about honest devices; 10 honest devices revoked (5 per provider) | Manual review 8 h (ASSUMPTION, D3) | **Security failure caused by §9.1** |
| Revocation stages (means) | detect → report 9 ms; sent → received 45.4 s; received → decision 100,800 s (16–40 h); decision → issued 14 ms; issued → published 26,583 s (1.0–17 h); published → downloaded 12.4 s; downloaded → enforced 0 | SCMS Manager one-business-day CRL SLA (03 §9) | The decision wait grows by 8 h per case (§9.3) |
| CRL | 10 entries, 612 B. 10 issued, 4 versions published, 470 downloads, 2,350 installs, 120 RSU installs | Brecht T-ITS 2018 ≈ 40 B per linkage entry; F2 measured 33.08 B (04 §5) | (612 − 328) / 9 = 31.6 B per added entry, consistent with F2 |

## 6. Privacy (passive observer at 10 % of signals, 200 m)

| Quantity | R-Q value | Reference | Reading |
|---|---|---|---|
| Frames read by the observer | 57,024 | — | — |
| Pseudonym changes | 314, almost all at the week rollover (the live windows are shorter than the 5-minute period) | J2945/1 5-minute change | The fleet changes at once at the boundary |
| Link decisions / claimed / correct | 403 / 183 / **0** | Wiedersheim et al. 2010: a global observer links nearly every change without silent periods; the engine's global-observer sensitivity gives 0.88 (`docs/validation/sensitivity-security.md`) | Plausible for 10 % coverage and a simultaneous change, but suspicious (§9.6) |
| Linkability metric | 0.0 (0/308 at the rollover, 0/314 at the end) | — | — |
| Tracking | 139 vehicles tracked, mean 27.9 s, max 57.9 s | — | — |

## 7. Run cost (release build, shared machine)

| Run | Simulated | Wall | CPU (user + sys) | Peak footprint |
|---|---|---|---|---|
| R-FULL (stopped) | 10 s of the first window, 701 nodes | 1,449 s to 10 s; 2,111 s in all | 453 s in all | 758 MB (909 MB in an earlier 120 s calibration attempt, also stopped) |
| R-Q | 90 s live + 172,620 s fast-forward | 5,979 s | 1,067 s | 428 MB |

- The load average was 18–41 from other jobs during every run, so each run had about 20 % of one core under `nice 10`. CPU time, not wall time, is the comparable figure.
- Where the time goes (R-FULL, `sample` of the running process): 82 % of samples are under `phase2::provision` → `ScmsRun::preload`. That is the real 1609.2.1 provisioning of each entering device: ACA issuance, RA and EE opening of the encrypted responses, verification of the signed SPDUs and the device's import check, all in real P-256. It costs about 0.4 CPU-s per device, linear in the fleet.
- Fast-forward is not cheap after the rollover. The first window (60 → 43,185 s) took about 5 s of wall time, the second (43,215 → 172,800 s) about 3,850 s. The report counts 1,119,908 backend ticks for 607,192 skipped mobility steps, where the D2 design expects about 10,080 a week (§9.2).

## 8. Why the full-scale file was not run to the end

R-FULL reached 10 simulated seconds in 24 minutes of wall time. At that rate its 300 live
seconds need many hours on the shared 8 GB laptop, so the quarter-fleet file replaced it. The
full file is unchanged and validated. On 2026-10-22 it was run on current code and stopped
at the 2.5 GB memory cap at about 155 s (§0). Run it with
`v2xw run scenarios/real-world/midtown-scms-benchmark.yaml --record-level metrics` on a
machine with 4 GB or more to spare (§0 has the memory model).

## 9. Implausible results and what they are

### 9.1 Engine defect: PSMs are signed under certificates that do not permit their PSID (cause confirmed in code)

> **Fixed** (branch `wf_fcf1da54-4cd-5`, docs/design/scms-real/validation/vehicle-and-roadside.md):
> handsets now request `{0x26, 0x27}`; the MA sets aside signature-only evidence; the detectors
> read PSM accuracy and skip relayed reports. On the first 60 s of the quarter file, MBR airtime
> fell from 29.3 % to 0 % and `psid-not-permitted` refusals from 32,052 to 0. The numbers below
> are the pre-fix run.

- **Observed:**
  - 48,123 receptions refused `psid-not-permitted` and 45,875 `Invalid` verification states.
  - 41,624 of 42,968 detector verdicts (96.9 %) came from `signatureVerification`.
  - 36,024 reports about honest devices, filling 24.5 % of the airtime with MBR frames.
  - 10 honest devices revoked.
- **Cause:** `crates/v2xw-engine/src/phase2.rs` (≈ line 638) builds `pdu.pseudonym_psids` from BSM, MBR and the vehicle services (CAM, DENM, CPM, SRM). The PSM (PSID 0x27, `MsgType::Psm => PSID_VRU` in `v2xw_msg::registry`) is never added. Yet VRU handsets are provisioned from the same pool (161 of the 349 devices) and sign PSMs. WP-C2's receivers check the SPDU PSID against `appPermissions` and refuse every PSM, and the `signatureVerification` detector reports the sender.
- **Fix (not made here: it needs an engine build and a fault-proved test):** add `MsgType::Psm` to `psids` when `actors.vru.device_fraction > 0`, or give VRU handsets their own pseudonym profile with PSID 0x27.
- **Regression test:** an honest run with VRU handsets has zero `psid-not-permitted` refusals and zero `revoked_honest`. The current code is the fault that must turn it red.
- **Why it matters for attack studies:** the failure is itself an attack surface: an MA flooded with false reports, and honest revocations. The real deployment would show it only if its PSM certificates were misprovisioned in the same way.

### 9.2 Fast-forward stops about every 150 ms after the rollover (open)

- 1,119,908 backend ticks over 172,710 s of skipped time is one tick per 0.154 s. The D2 design allows at most one stop a minute when nothing is due.
- The first window was fast, so something that starts at or after the rollover keeps the backend waking.
- Candidates: the MAs' queued cases under manual review, the blocklisted devices' refused top-ups, or the two items that stay in `p2.queue`. Not investigated further.

### 9.3 The manual MA decides one case per provider every 8 hours (open)

- Cases rose 2 → 4 → 6 → 8 → 10 at 28,860, 57,660, 86,460 and 115,260 s, and report → decision spans 16–40 h. Under a report flood the queue grows without bound.
- This may be the intended model of one operator reviewing in series (D3 ASSUMPTION). But B4 says cases run concurrently, so one of the two is wrong.

### 9.4 BSM signing is charged a constant 9 ms

Every BSM's sign stage is exactly 9.0 ms: the CRATON2 datasheet's upper bound ("<9 ms
signing latency") used as a constant. It makes the end-to-end latency 12 ms where a hardware
signer would give less. This is a modelling choice to revisit, not a defect.

### 9.5 Full-certificate share 25 % against J2945/1's 10 %

Each P2PCD learning request (387/s) makes the sender attach its certificate on its next
message, and during the fill almost every neighbour is new. Plausible; re-measure it in a
steady-state window.

### 9.6 The observer's 183 link claims are all wrong

0 correct out of 183 is possible for a 10 % observer at a simultaneous fleet-wide change. But
the linker is right less often than chance at an anonymity set of about 1.2 per claim, so it
should be checked against ground truth.

## 10. Not run, and how to run them

| Item | Command | Expected cost |
|---|---|---|
| LTE-V2X variant | `v2xw run scenarios/real-world/midtown-scms-benchmark-lte.yaml --record-level metrics` | as the full DSRC file |
| Portland | `v2xw run scenarios/real-world/portland-scms-benchmark.yaml --record-level metrics` | a third of the Midtown fleet |
| 2× and 4× fleet | the full file with `rate_veh_per_h` 18,000 and 36,000, and the VRU sample doubled and quadrupled | provisioning about 0.4 CPU-s per device (linear); reception attempts grow with the square of the fleet (F3: 4× vehicles gave 15× the attempts, 05 §5) |

Fix §9.1 before running any of them. Once PSMs verify, every number in §3 to §6 that
involves reports, MBR airtime or revocation will change.
