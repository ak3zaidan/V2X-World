# Run cost: what settings do to engine performance

This page checks that settings change engine **cost** the way theory says, and looks for super-linear blow-ups and leaks.

- **Binary:** release `v2xw` built from main at e94e43c, run under `heavy.sh` (one heavy job machine-wide) on the owner's 8-core, 8 GB machine.
- **Timing:** `/usr/bin/time -l`. Recording was off (`--no-recording`) unless the row says otherwise.
- **Threads:** two (`RAYON_NUM_THREADS=2`) unless the row says otherwise.
- **Scenario:** the Manhattan scaling ladder (`scenarios/scale/base.yaml`: DSRC, IEEE 1609.2 signed BSMs, buildings on, 2 s simulated). Each variant is a one-change overlay in `scenarios/scale/cost/`.
- **Columns:**
  - **run/sim** is the engine's own `timing.wall_s_per_sim_s`: event loop only, with the ~0.9 s world build excluded.
  - **attempts** is `report.reception_attempts`.
  - **RSS** is the peak resident set size.

## Cost table

| Setting | Change | Veh | run/sim (s/s) | RSS MB | Frames | Attempts | Expected | Pass |
|---|---|---:|---:|---:|---:|---:|---|---|
| fleet | 10 (cap 15) | 15 | 0.034 | 109 | 299 | 2 813 | ~N x receivers in reach | yes |
| fleet | 100 (cap 150) | 145 | 1.742 | 143 | 2 834 | 271 566 | " | yes |
| fleet | 300 (cap 480) | 453 | 14.27 | 279 | 8 171 | 2 375 369 | " | yes |
| fleet | 1000 (cap 1600) | 1 267 | 86.38 | 902 | 16 651 | 11 468 181 | " | yes (see 1) |
| threads, 145 veh | 1 / 2 / 4 | 145 | 2.47 / 1.74 / 1.38 | 142 | same | same | speed-up, identical work | yes |
| threads, 453 veh | 1 / 2 / 4 | 453 | 22.6 / 14.3 / 10.4 | 206-279 | same | same | larger speed-up on larger fleet | yes (2.2x at 4) |
| duration, 145 veh | 2 / 10 / 30 s | ~150 | 1.74 / 1.80 / 1.86 | 143 / 139 / 138 | 2 834 / 14 673 / 44 650 | linear | linear in time, flat memory | yes |
| `radio.range.max_m` | 300 m | 145 | 0.348 (-80%) | 128 | 2 834 | 32 063 | fewer links evaluated | yes |
| `radio.range.margin_db` | 5 dB (open grid) | 40 | - | - | - | unchanged | fewer attempts | **corrected** (see 3) |
| `world.buildings.enabled` | false | 145 | 1.778 (+2%) | 170 | 2 833 | 403 863 (+49%) | more successful links; no ray tests | yes (see 2) |
| `radio.tiers.mac` | abstract | 145 | 1.745 | 142 | 2 833 | 272 010 | no contention events | yes (0 grants) |
| `radio.tiers.mac` | high | 145 | 1.715 | 140 | identical | identical | **does nothing** | label fixed |
| `radio.tiers.phy` | abstract | 145 | 1.750 | 143 | identical | identical | documented "runs as Standard" | yes |
| `radio.tiers.propagation` | high | 145 | 1.992 (+14%) | 144 | 2 834 | 209 914 | geometric law costs more | yes |
| all three tiers | high | 145 | 1.948 | 143 | 2 834 | 209 914 | slowest | yes |
| all three tiers | abstract | 145 | 1.549 (-11%) | 153 | 2 833 | 404 192 | "cheapest, for very large fleets" | **weak** (see 4) |
| `messages.rate_hz` (step 50 ms) | 5 / 10 / 20 Hz | 145 | 1.02 / 1.70 / 3.37 | 137-143 | 1 417 / 2 808 / 5 626 | linear | linear in rate | yes |
| `net.layer` | gn-btp + CAM, against wsmp + CAM | 145 | 1.058 / 0.916 | 136 / 140 | 1 447 / 1 447 | same | small GN header and processing overhead | yes (+15%) |
| `net.geonet.hop_limit` (gn grid, 24 s) | 1 / 10 | 40 | 0.100 / 0.095 | 33 / 34 | 6 594 / 6 624 | +0.5% | forwards are a small share of frames | yes |
| `metrics` | `[pdr]` / `[all]` | 145 | 1.742 / 1.784 | 143 / 146 | same | same | small (+2%) | yes |
| `--record-level` | metrics / events / receptions / full | 145 | 1.66 / 1.71 / 2.75 / 2.82 | 144-151 | same | same | per-reception records dominate | yes |
| `security.signature` (pq-capable OBU) | falcon-512 | 145 | 1.754 | 138 | 2 561 | 248 013 | larger frames, more airtime | yes |
| `security.signature` | ml-dsa-87 | 145 | 0.071 | 92 | **0** | 0 | every SPDU above the WSMP MTU | see 5 |
| `scale.traffic.model` | meso-micro, 300 m radio margin | 1 555 | 5.52 | 244 | 8 388 | 1 076 309 | far cheaper than all-micro | yes (16x faster than f1000) |
| `scale.traffic.model` | meso-micro, radio follows all | 1 555 | 19.8 | 265 | 18 348 | 2 331 912 | between the two | yes |

## Findings

1. **Cost is linear in reception attempts, and attempts are quadratic in fleet here.**
   - Run time per attempt per simulated second is 12-15 µs at every rung.
   - Cost grows with N² on this ladder, and that is correct. The line-of-sight reach at the 33 dBm reference EIRP (`CandidateRangePlan`) covers the whole 2.5 km Manhattan box, so every vehicle is a candidate receiver for every frame. The spatial index cannot prune a set that is all in range.
   - `radio.range.max_m` and the meso tier restore near-linear cost: 300 m cuts the run by 80% at 145 vehicles.
   - There is no super-linear term beyond attempts. Per-attempt cost rises only from 12 µs to 15 µs between 453 and 1 267 vehicles (cache pressure).
   - Memory is about 0.7 MB per vehicle at 1 267 vehicles. It stays flat over simulated time (2, 10 and 30 s).
2. **Hotspot: building geometry.**
   - A `sample` profile of the 453-vehicle run (one thread) puts about 50% of the CPU in `obstacle::ring_crossing` (27%), the rstar `SelectionIterator` walk (19%) and `FootprintBox::meets`. ChaCha draws take 6%.
   - Long links crossing dozens of footprints dominate. They need every wall counted for the Sommer β/γ loss, so they cannot exit early without changing the received energies, and with them the digests.
   - A digest-safe speed-up would need an exact bound. One option is to stop the walk once the accumulated loss puts the link below the energy that any consumer reads. That needs a check on how sub-floor energy enters SINR. Not attempted.
3. **`radio.range.margin_db` did nothing on the 1.2 km open grid** (expectation row DEAD). The reach exceeds the world's diagonal at both 5 and 10 dB, and open-air links all clear N − margin, so the same links are followed. The row is corrected to `same` with that evidence. The Manhattan margin runs (overlays `margin5`/`margin20`) were queued but did not get the lock in time.
4. **The abstract radio tier is only 11% cheaper.**
   - Its option label says "cheapest, for very large fleets", but it still walks every pair in reach. It also follows 49% more links, because the abstract law has no building blockage.
   - This is a real gap against the label. Not fixed this round: it needs either a distance cut in the abstract tier or a reworded label.
5. **ML-DSA-87 on DSRC/WSMP sends nothing.** All 2 850 SPDUs were refused at the MTU (`net_mtu_refusals`), so the run cost nothing.
   - This is the documented air-interface consequence (`signature.rs`), and the `pq-signatures` sweep relies on it.
   - The loader does not refuse it, though, so a user gets a silent empty run. Suggest a validator warning when a scheme's per-signature octets alone exceed the MTU with `fragmenter/none`.
6. **`radio.tiers.mac: high` was a silent no-op.** It ran frame for frame identically to `medium`: `build_mac` treats every non-abstract tier the same, and its only effect is to permit `phy: high`. **Fixed:** the settings label (`scenario/options.rs`) now says it runs as Standard and unlocks the Detailed PHY, as the `phy: abstract` label already did. Labels are not hashed, so digests are unchanged.

## Expectation rows

`crates/v2xw-engine/tests/expectations/run-cost.yaml` pins the deterministic work counters that wall time follows: attempts, frames and MAC grants. `v2xw sensitivity --expect` on that file gave 6/8 on the first pass:

- **`MacTimer`:** the row named the event-class count `MacTimer`, which is absent at the abstract tier. The row now uses `report.mac_grants` (1 006 → 0).
- **`margin_db`:** corrected as in finding 3.

Neither correction has been re-run.

## Not measured (lock contention)

The 30-minute soak and the 3-minute reference were not run, and neither were the 453-vehicle abstract and 300 m variants, 1 000 vehicles with 300 m and with 4 threads, and the Manhattan margin pair. Overlays and the batch list are in place: `scenarios/scale/cost/*.yaml`.
