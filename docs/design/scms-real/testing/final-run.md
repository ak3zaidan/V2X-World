# Final runs, part 2: the full-scale benchmark to completion and the certificate cycle after R-CCSIG (2026-10-23)

Branch from main `e4004b4c` (after fix wave 3: PQMEM, DETPRIV, RERUNS). Every expectation
was written down and committed before its run
(`crates/v2xw-engine/tests/expectations/scms-final-run.yaml`, commit `0874e146`). All runs
use the release binary of main `09e55393`: the Rust code of `e4004b4c`, since only docs
changed after it. It was built at the wave-3 integration and copied to the scratchpad
(`v2xw-fr`, sha256 prefix `aee25e7d8e081322`), so this package needed no build. Each run
went through the machine's run lock, one run per call, with seed `0xBE7C_0001`,
`--record-level metrics` and `--no-verify`. No engine code was changed.

## 1. Runs

| Id | Scenario | Threads / cap | Result | Wall / CPU / peak |
|---|---|---|---|---|
| PQ0 | `stress/pq-base.yaml` (quarter fleet, 60 s live, ECDSA P-256) | 2 / 2.5 GB | complete | 85 s / 104 s / 223 MB |
| PQHFC | `stress/pq-hybrid-falcon512-certcycle.yaml` | 2 / 2.5 GB | complete | 69 s / 87 s / 231 MB |
| PQHMC | `stress/pq-hybrid-mldsa44-certcycle.yaml` | 2 / 2.5 GB | complete | 71 s / 90 s / 225 MB |
| FB2 | `real-world/midtown-scms-benchmark.yaml` as committed (full fleet, 172,800 s) | 4 / 3.5 GB | stopped by hand at 47,340 of 172,800 s (both live windows complete) | 9,792 s / ≈ 17,900 s (4.97 CPU-h) / 3,109 MB |

"old" means the final-bench.md run of the same file on `88e24b09`.

## 2. The certificate cycle after the R-CCSIG fix (quarter density, 60 s)

| Row | Expected (before) | Measured (PQ0 → variant) | Verdict |
|---|---|---|---|
| PQHFC.sizes | 1.00 frame per BSM; BSM PSDU ≈ 207 + 858/5 ≈ 380 B (330–420) | 1.00 frame per BSM; **207 → 380 B** (old 1,045); PSM 241 → 409 B | fixed (R-CCSIG, wave 3; confirmed at density) |
| PQHFC.airtime | BSM 20–27 s; PSM 4–6.5 s; CBR 0.014–0.022 | BSM 13.6 → 23.2 s (×1.71; old 60.4); PSM 3.0 → 4.9 s; CBR 0.0100 → 0.0159 | pass |
| PQHFC.delivery | verified −8 %…+2 %; PDR 80–100 m between 0.66 and 0.725; e2e +0.1–0.5 ms | −2.1 % (old −7.5 %); 0.710; 3.72 → 3.96 ms | pass |
| PQHFC.verify | ECDSA cost ±5 %; 2,000–12,000 PQ certificate checks, < 5 % of checks | 0.6278 → 0.6288 ms (old 0.751); `cert_pq_verifications` 2,813 (0.75 %) | pass |
| PQHFC.architecture | backend bytes ≈ old 146 kB ±10 %; 60 RSU CRL installs; relay reports refused at the MTU | 38,766 B; 60 installs (old 0); relay 0, refused 2; topups 0 | wrong expectation (bytes): see below |
| PQHFC.cost | attempts ±5 %; events 1.2–1.6 M; 60–130 s | −0.005 %; 1.45 M; 69 s | pass |
| PQHMC.sizes | transmits; 1.00 frame per BSM; PSDU 700–850 B | 50,183 frames (old 0, 50,180 MTU refusals); 1.00; **814 B** (+607 = 3,035/5) | fixed (R-CCSIG) |
| PQHMC.airtime | BSM 38–52 s; PSM 8–12 s; CBR 0.025–0.042 | 47.5 s (×3.49, ×0.27 of hybrid-on-every-SPDU's 176 s); 9.7 s; 0.0366 | pass |
| PQHMC.delivery | verified −2 to −24 %; e2e +0.3–1.5 ms | −5.7 % (every-SPDU hybrid: −33.8 %); 4.57 ms (+0.85; old PQHM 12.1) | pass |
| PQHMC.verify | ECDSA cost ±5 %; PQ checks ≤ PQHFC's | 0.6297 ms; 2,743 | pass |
| PQHMC.architecture | backend ≈ old PQHM 228 kB ±10 %; 60 RSU installs; relay refused | 103,575 B; 60; relay 0, refused 2 | wrong expectation (bytes) |
| PQHMC.cost | events 1.3–2.0 M; 60–180 s | 1.53 M; 71 s | pass |
| PQ.digests | all three move and differ | PQ0 `099bed7a…`, PQHFC `a73101dd…`, PQHMC `b33458e9…` | pass |

Sizes, airtime and delivery under the Partially-Hybrid cycle, against hybrid signatures on every SPDU:

| | ECDSA (PQ0) | Falcon-512 cycle | ML-DSA-44 cycle | ML-DSA-44 on every SPDU (old PQHM) |
|---|---|---|---|---|
| BSM PSDU mean | 207 B | 380 B | 814 B | 1,332 B per fragment, 2.30 frames per BSM |
| BSM airtime (60 s) | 13.6 s | 23.2 s | 47.5 s | 176.2 s |
| CBR mean | 0.010 | 0.016 | 0.037 | 0.125 |
| Verified messages | 380,664 | 372,587 (−2.1 %) | 359,146 (−5.7 %) | 251,882 (−33.8 %) |
| BSM e2e mean | 3.72 ms | 3.96 ms | 4.57 ms | 12.06 ms |
| Verify cost mean | 0.628 ms | 0.629 ms | 0.630 ms | 0.787 ms |

**Backend bytes were a wrong expectation.** The row assumed that the bytes were provisioning
traffic, which keeps the full hybrid scheme. Provisioning is preloaded, so the bytes are the
carried misbehaviour reports. PQ0 itself fell from 49,346 to 5,364 B because reports fell
from 54 to 15 with the wave-3 GNSS fix (`constantPositionFrozen` 28 → 0 at quarter
density). Per report the hybrid cost is unchanged:
- Falcon-512: 2,706 B per report on `88e24b09`, 2,584 B now;
- ML-DSA-44: 5,843 B per report then, 6,093 B now.

## 3. Found and not fixed

**R-PQUNSEC: under the post-quantum profile a roadside unit's own reports are filed.**
- Measured: PQHFC 7 and PQHMC 9 `reports_from_rsus`. On P-256 the same scenario files 0 and
  counts 7 under "a roadside unit holds no pseudonym certificate to sign an SM-MBR with".
- Cause: `Phase2::real_mbr` (`phase2.rs` ≈ line 4489) returns `MbrBuild::Off` when
  `home().state.pki` is `None` (as it is under PQ), before the R-UNSEC roadside check.
  `files_report` treats `Off` as "file it".
- The rule R-UNSEC cites (no roadside reporter in the US profile: [SM-MBR] §3, USDOT RSU
  Specification v4.1) does not depend on the signature scheme.
- Fix: put the roadside check between the ETSI return and the `pki` return. It was not made
  here, for two reasons:
  - `pq_relay.rs`'s R-PQRSUCRL test needs `ma_revoke_decisions > 0` on the same kind of
    scenario. Not filing the RSU reports may leave its MA without a case, the same owner
    decision `scms_wiring` is waiting on.
  - The machine was held for the full-scale run, so the change could not be built and
    fault-proved here.

**Invalid verification states under the cycle.** Both cycle runs have 47 `Invalid` states
(46 in the old PQHFC) and P-256 has 0, while `receiver_validation.refused` is empty. This is
not root-caused. It may be the RERUNS item that `node.rx` labels an out-of-date-list refusal
`signature-invalid`.

## 4. The full-scale benchmark (FB2)

**It did not complete.**
- Both live windows ran in full, under the 3,500 MB cap:
  - 0–240 s: 7,113 s of wall time;
  - the rollover window, 43,170–43,230 s: 1,855 s.
- The first fast-forward (42,930 s) took 207 s.
- The second, after the rollover, crawls at about 7 simulated seconds per wall second
  (benchmark §9.2, still open). The remaining 126,000 s would have needed about 5 more hours,
  so the run was stopped at 47,340 s after 9,792 s of wall time, at the package's time budget.
- So there is no end-of-run report. The numbers below come from three sources:
  - the complete 10 s metric windows (`metrics.json`, salvaged up to 43,220 s);
  - the census lines of `progress.jsonl`, every 10 simulated seconds;
  - the process accounting.

| Row | Expected (before) | Measured | Verdict |
|---|---|---|---|
| FB2.completes | rc 0 under 3.5 GB; 2.0–2.4 GB at 150 s | not completed (stopped in the second fast-forward). Footprint 2,690 MB at 150 s with 1,632 nodes (R-FS: 2,582 MB at ~155 s); peak 3,109 MB at the 43,170 s resume | open (memory part: wrong expectation) |
| FB2.cost | 3–4.5 CPU-h; 1.5–3.5 h wall | 0–240 s 7,113 s wall; FF1 207 s; rollover 1,855 s; FF2 at 6.7 sim-s per wall-s (≈ 5.4 h); 4.97 CPU-h when stopped | open |
| FB2.devices | 1,800–2,100 devices; split within 45/55 | 1,933 devices provisioned by 240 s (40 credentials each); 1,833 nodes on the map; home-provider RA 944 requests = 48.8 % | pass |
| FB2.provisioning | 1 request, 40 certificates per device; 900–1,050 requests per RA in 240 s | 40.0 per device; 944 requests (3.9/s against CAMP's ≈ 0.5/s design rate) | pass |
| FB2.topups-at-rollover | 75–100 % of devices top up inside the 1 h overlap; pool ~40 after | pool mean 20 → **39.83** in the 43,200–43,210 window. The home provider issued 12,580 top-up certificates (629 × 20, about 92 % of its devices on the map) within 10 s of 43,200 s | pass |
| FB2.radio | as R-FS: CBR 0.07–0.09, PDR 100 m 0.80–0.87, e2e 12–15 ms | 130–150 s **identical to R-FS**. 200–240 s: CBR 0.081–0.087, p95 0.15–0.17; PDR 100 m 0.833–0.841; e2e 12.4 ms. Hidden terminal 2.4 % of attempts | pass |
| FB2.verify-load | 200–290 per node per second | 240 at 130–150 s, 268 at 230 s (484 k/s); queue p99 0; unverified 5.2 → 6.2 % | pass |
| FB2.honest-reports | no constantPositionFrozen burst; 0–4 cases; 0 revoked | **1,935 reports in 240 s** (8.1/s), all honest, none delivered before the fast-forward. **10,357 more in the 60 s rollover window**, 4,876 of them in the 10 s after the resume. 15,062 reached the MAs at 43,200–43,220 s: **71 cases**, 71 honest vehicles falsely accused (vehicle-level FPR 3.3 %) | **open** |
| FB2.crl | 0 entries; 2–4 versions; 0 refusals | 0 CRL Generator entries through 46,790 s; the rest needs the run report | open |
| FB2.trust | no refusals on honest traffic | no signature, PSID, CTL or CRL loss cause; unknown-signer 1e-4 of attempts at the resume | pass (from metrics) |
| FB2.privacy | changes at the rollover; claims ≤ changes heard | `pseudonym_change_rate` 0 in every window, the rollover included; linkability 0/0 | open |
| FB2.traffic | 1.5–3 m/s | 2.75 → 1.82 m/s; **p50 0 m/s from 160 s** (gridlock, density rising) | pass |
| FB2.ff-ticks | < 5 × 10⁴ ticks | ticks only in the run report; FF1 42,930 s in 207 s wall, FF2 4,110 s in 617 s | open |
| FB2.backend-load | RA ~1,000 requests per provider; MA reports in the hundreds | RA as CAMP sizes it per device; MAs 15,062 reports in 20 s and 71 cases | open |

Open items this run found (none fixed: the machine was held for the run, so no build):

1. **R-FFHOLD: reports are held in flight for a whole fast-forward.**
   - The census `p2.in_flight` stayed at 1,258 from 240 s to 43,170 s, and `scms.ma_reports`
     stayed at 0.
   - The MAs received the first window's reports only at 43,200–43,210 s. At that point the
     backend kernel's ops went from 42 to 1,954, at the i-period boundary.
   - A report sent at 100 s should reach the MA within seconds. The fast-forward's
     `next_wake` apparently does not include the in-flight report deliveries.
   - Consequence: MA timing (report → case → decision) is 12 h late for anything reported
     before a fast-forward.
2. **R-FFBURST: honest-report burst on resuming after a fast-forward.**
   - In the 10 s after the 43,170 s resume, 4,876 reports were filed, against about 80 per
     10 s in the first window.
   - Unconfirmed hypothesis: the GNSS model's new mean-speed propagation (gauss-markov 1.2.0)
     or the vehicles' kinematics are stepped across the 42,930 s gap as one epoch. The node
     count also fell from 1,833 to 1,281 at the resume, which shows the fleet was rebuilt.
   - The detector breakdown is in the run report only.
3. **Pedestrian wait counts the fast-forward.** `ped_wait` has a mean of 3,465 s and a max
   of 4.3 × 10⁴ s in the 43,170–43,180 window. This is the same class of defect as the AoI
   fix in MATRIX.
4. **Memory was not lowered at full scale.** See FB2.completes.
5. **The second fast-forward is slow** (§9.2). The first is cheap. The second steps at about
   one SCMS kernel step per simulated second (`scms.kernel_steps` rose 2,560 over 2,600 s)
   and costs about 0.15 wall-s per simulated second.
6. **No pseudonym change is counted at the rollover.** See FB2.privacy.

To finish the file, fix item 5, then run it again with `--checkpoint-every 1h`. Fix items 1
and 2 first, or the MA outcome over the two days measures those artefacts, not the
deployment.

## 5. What was fixed

Nothing in the engine. The machine was held for the full-scale run, so no build was made. R-CCSIG (wave 3) is confirmed at density: two rows are `fixed`. Two expectations were corrected with evidence: backend bytes scale with reports, not with provisioning. Seven items are open: R-PQUNSEC (§3), R-FFHOLD, R-FFBURST, the uncounted rollover change, `ped_wait` across the fast-forward, full-scale memory and the slow second fast-forward (§4).
