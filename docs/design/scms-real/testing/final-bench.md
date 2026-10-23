# Final runs, part 1: the full-scale benchmark and hybrid ML-DSA-44 (2026-10-22)

Branch `worktree-wf_18692539-875-5`, from main `88e24b09` (after fix wave 2: TRUST, REPORTS
and DETECT). Every expectation was written down and committed before its run
(`crates/v2xw-engine/tests/expectations/scms-final-bench.yaml`, commits `c574bb9a` and
`e9648040`). One opt-level-2 debug build of `88e24b09` was used for every run (`v2xw-fb`,
sha256 prefix `9bdf0275318dcbf8`). The release binary was not used, because it predates
the fix wave. Each run went through the machine's run lock, one run per call, with seed
`0xBE7C_0001`, `--record-level metrics` and the 2.5 GB memory cap. No engine code was
changed, and the only build was that one.

## 1. Runs

| Id | Scenario | Threads | Result | Wall / CPU / peak |
|---|---|---|---|---|
| R-FS | `real-world/midtown-scms-benchmark.yaml` (full fleet, 172,800 s) | 2 | **stopped by the memory cap** (2,582 MB) at about 155 s; the 130 / 140 / 150 s metric windows are complete | 79 min / ≈ 6,300 s / 2,582 MB |
| R-FS40 | the same, 0–40 s, 10 s warm-up, census every 10 s (scratch `fd.yaml`) | 2 | complete | 535 s / 623 s / 910 MB |
| PQ0 | `stress/pq-base.yaml` (quarter fleet, 60 s live, generic-sdu, software OBU, ECDSA) | 2 | complete | 105 s / 125 s / 249 MB |
| PQHM | `stress/pq-hybrid-mldsa44.yaml` | 4 | complete | 583 s / 726 s / 367 MB |
| PQHM1 | the same | 1 | complete, identical to PQHM | 624 s / 613 s / 373 MB |
| PQHFC | `stress/pq-hybrid-falcon512-certcycle.yaml` (run to test R-CCSIG; row written before it) | 2 | complete | 83 s / 104 s / 235 MB |
| PQHMC | `stress/pq-hybrid-mldsa44-certcycle.yaml` (new: hybrid ML-DSA-44 under the NDSS certificate cycle) | 2 | complete, **but silent**: 0 frames | 30 s / 30 s / 172 MB |

PQ0 took 125 CPU-s where stress.md's identical scenario took 1,175 CPU-s. That is the effect
of the verification-cache fix ("one check was redoing real cryptography thousands of
times"); the radio numbers are unchanged.

## 2. Full-scale benchmark: expected against measured

| Row | Change | Expected (before) | Measured | Verdict |
|---|---|---|---|---|
| FB.devices | quarter → full | 1,800–2,100 devices at the end; ~550 handsets; split 45/55 | 40 s: 1,000 nodes (496 / 504), 581 handsets; 130–150 s: 1,581–1,632 nodes | open (run stopped; handsets and split pass) |
| FB.provisioning | — | 1 request, 40 certificates per device | 331 requests / 13,240 PCA certs = 40 × 331 (provider-a, 10 s); 16,960 = 40 × 424 at 40 s | pass |
| FB.provisioning-burst | — | 10–25 requests/s, 20–50× CAMP's 0.5/s | about 10/s (701 → 992 nodes in 30 s) | pass |
| FB.receptions-scale | PQ0 → full, same window | 8–16× | 15.4× (459 k → 7.06 M attempts, 30–40 s) | pass |
| FB.cbr | PQ0 → full | 3–5×; mean 0.05–0.12 after the fill; < 0.5 | 3.6× same-phase; 0.074–0.078 after the fill, p95 0.14–0.16 | pass |
| FB.pdr-100m | quarter → full | ≤ quarter, > 0.6 | 0.838 → 0.84 | pass |
| FB.latency | — | BSM e2e 12–15 ms | 12.39 ms mean, p99 14.94 | pass |
| FB.verify-load | quarter → full | 100–150 per node per second | ≈ 240; queue p99 0; first sight 3.7 % | wrong expectation (nodes 4.5×, packed into a queue) |
| FB.mbr-airtime | — | < 1 % | 0.21 % | pass |
| FB.honest-revoked | — | a few reports, 0 decisions, 0 revoked | 40 s: 229 reports about 32 honest devices (`constantPositionFrozen` 165 of 236 verdicts); 0 cases; 2-day outcome unmeasured | open |
| FB.crl | — | CRL at t0 per CRACA, 0 entries, 0 overdue | 2 versions at t0, 2,000 receiver installs, 60 RSU installs, 0 refusals | pass (t0 part) |
| FB.trust | — | 0 refusals | 2,019,081 SPDUs, 0 refused | pass |
| FB.privacy | — | linkability ≤ quarter, correct links ≈ 0 | 2,873 link claims, 0 correct, **with 0 pseudonym changes** | open |
| FB.traffic-speed | quarter → full | 2–4 m/s | 2.75 → 2.33 m/s, p50 0.18 | pass |
| FB.topups-at-rollover, FB.ff-ticks | — | (see yaml) | not reached | open |
| FB.cost | — | 1.5–4 CPU-h; 0.8–2.0 GB | 40 CPU-s per simulated second and rising; **memory cap at 155 s** | open |

The comparison with the quarter fleet, row by row, is in `docs/benchmark/scms-real-benchmark.md` §0.

## 3. Hybrid ML-DSA-44 at quarter density

| Row | Expected (before) | Measured (PQ0 → PQHM) | Verdict |
|---|---|---|---|
| PQHM.sizes | 2.3–2.7 frames per BSM | 41,903 → 96,561 BSM frames (2.30 per BSM, the same as pure ML-DSA-44); PSDU 207 → 1,332 B | pass |
| PQHM.airtime | BSM airtime ×12–15; CBR 0.13–0.17 | ×13.0 (13.6 → 176.2 s); CBR 0.010 → 0.125 | airtime pass; CBR wrong expectation (anchored on a base that still carried the report storm) |
| PQHM.loss | verified −30 to −40 % | −33.8 % (380,617 → 251,882); frame PDR 80–100 m 0.725 → 0.519 | pass |
| PQHM.verify-load | 0.9–1.2 ms, above hybrid Falcon's 0.751 | 0.628 → 0.787 ms; BSM e2e 3.7 → 12.1 ms | ordering pass; band wrong (the fleet mixes handset and RSU price profiles) |
| PQHM.reports | 0 relay reports refused at the MTU; none counted wrongly | 6 relay uploads delivered, `reports_refused_mtu` 0, `net_mtu_refusals` 0 | pass (R-PQMBR holds) |
| PQ.topups | none in 60 s | 0 / 0 | pass |
| PQHMC.sizes / verify / reports | ECDSA BSMs, α = 2, ~1 frame per BSM, airtime ×3–5 | **0 frames transmitted; 50,180 SDUs refused at the MTU** | **open: engine defect R-CCSIG** |

**R-CCSIG (open, engine defect).**
- Under `fragmenter/cert-cycle-partial-hybrid` the engine still signs every SPDU with the
  scenario's hybrid scheme. `run.rs` `hand_down_app` calls `signature::resize` with
  `security.signature`, and commit `45f0942f` sized the cycle's fragment to fit beside the
  post-quantum signature.
- The design this fragmenter cites (Twardokus et al., NDSS 2024, Partially-Hybrid;
  `v2xw-net/src/frag/cert_cycle.rs` header) keeps ECDSA on every BSM: "verification cost
  and message size are unchanged", and frames 2–5 of the cycle are 204 B (Fig. 4).
- With Falcon-512, each SPDU is 666 B too large but still fits the 1,400 B WSMP MTU.
  PQHFC measured it, with its expectation written before the run:
  - 1.00 frame per BSM and a BSM PSDU mean of **1,045 B**, the R-CCSIG prediction, where the
    NDSS design gives about 204 + 858/5 ≈ 380 B;
  - BSM airtime ×4.4 and CBR 0.044;
  - verify cost 0.751 ms, the hybrid sum rather than ECDSA's 0.628.
  Every certificate-cycle number so far carries these.
- With ML-DSA-44 (2,484 B of signature), every BSM and PSM is refused. The run transmits
  nothing, and the validator accepts the scenario with notes only.
- Fix (next wave):
  - under the cycle, size and price each SPDU as ECDSA P-256, and verify the
    certificate's post-quantum signature once per new certificate;
  - make the validator refuse a configuration whose every SDU exceeds the MTU.
  - The test: PQHMC's `frames_transmitted > 0` and BSM PSDU at most 204 B plus the cycle
    fragment. It is red on `88e24b09`, as this run shows.
- Not fixed here: it changes a deliberate earlier decision and every certificate-cycle
  digest, and this package had no build left for a fault-proved fix.

## 4. Determinism (PQHM at 4 against 1 rayon thread)

**Pass.**
- The record digest is `47fe89fb95c7b8c4…` at 4 threads and at 1 thread.
- `metrics.json` and `run-report.json` are byte-identical (sha256 `4e761a1a…`, `8b7cf6cc…`).
- The check can fail: PQ0, PQHM and PQHMC differ in one setting each and give three different digests.
- Four threads saved 7 % of wall time (583 s against 624 s) and cost 18 % more CPU.

## 5. What was fixed

Nothing in the engine. Found and documented: R-CCSIG (§3), and three full-scale items in
benchmark §0.1: memory per node, observer link claims without pseudonym changes, and
honest `constantPositionFrozen` reports at queue speeds. The two other "fixes" are
expectation corrections, each with its evidence in the yaml.
