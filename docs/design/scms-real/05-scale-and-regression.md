# 05 — Scale, regression and test time (WP-F3)

Status: written 2026-10-17 by the WP-F3 agent. Every number below was measured in this
worktree under `.claude/pm/heavy.sh` (2 test threads, the owner's 8 GB Mac), in the **test
profile** (no release build is allowed in agent worktrees). Wall times are for comparison
between runs here, not device or release costs.

## 1. Test time

### 1.1 Where the time went

`sample` on one Phase 2 case (`with_no_attacker_nothing_is_revoked`, opt-level 0) put the
CPU in P-256/P-384 field arithmetic and `crypto-bigint` (≈ 25 %), ChaCha (`rand_chacha`,
`ppv-lite86`, ≈ 9 %), `serde_json`, and the workspace's own per-reception work: obstacle
ray tests (`v2xw-radio::obstacle`), `point_in_ring` (`v2xw-world`), RNG streams
(`v2xw-core::rng`), metric views (`v2xw-metrics::channels`). Optimising only third-party
crates removed little (466 s → 339 s): RustCrypto's field operations are `#[inline]`, so
they are compiled into `v2xw-sec` at `v2xw-sec`'s own opt-level, and the rest is
workspace code.

### 1.2 The change (`Cargo.toml`)

- `[profile.dev.package."*"] opt-level = 2`: every third-party crate, in dev and test.
- `[profile.test.package.<crate>] opt-level = 1` for the ten hot workspace crates
  (`v2xw-core`, `-msg`, `-sec`, `-world`, `-radio`, `-metrics`, `-mobility`, `-node`,
  `-proto`, `-record`). `cargo build`/`check` keep them at 0; the engine crate, whose many
  test binaries would each pay the optimiser, stays at 0. opt-level 2 for these crates
  measured no faster than 1.
- `debug-assertions` and `overflow-checks` are untouched, so no assertion is weakened.
  Rust never contracts or reorders floating point, so opt-level moves no result. **Shown:**
  both pinned digests (§2) reproduce from a build with the old opt-level-0 profile.

### 1.3 Measured

| Run | Before | After |
|---|---|---|
| One Phase 2 case, 1 thread | 466 s | 93 s |
| `--test phase2` (the 11 cases before this package) | ≈ 25 min (owner) | 292 s |
| `--test phase2` with this package's 2 cases | — | 383 s |
| `--test backend_entities` | ≈ 6 min (owner) | 76 s |
| I-S1 pair, real + modelled, 60 s | 90 s | 12.5 s |
| Rebuild of the deps once | — | +219 s, one time in `target/agents` |

No fixture was shrunk and no case moved out of the default suite: with the profile change
none needed to be. The Phase 2 suite is now bound by its six full 300 s Manhattan runs
(the path, its rerun for determinism, three counterfactuals and the `camp-reference` run),
each 85–110 s on 2 threads.

### 1.4 The long-test convention

A slow measurement is `#[ignore = "long: …"]` and its name starts with `long_`. The default
suite shows it as ignored (never a silent pass); the merge step runs them with

```
cargo test -p v2xw-engine --test scms_scale -- --ignored long_
```

`--ignored` alone also runs the `diagnostic` and `measurement` cases of other files, so
the `long_` filter is part of the command.

## 2. Regression digests

`crates/v2xw-engine/tests/regression/` holds one JSON record per case: the record digest
(`DigestRecorder::digest_hex`), the frame digest and the record count. A missing record
fails; a deliberate change re-blesses with

```
V2XW_BLESS_SCMS_DIGESTS=1 cargo test -p v2xw-engine --test phase2 --test scms_scale
```

and the commit says what changed. Cases:

| Case | Run | Records |
|---|---|---|
| `phase2-path` | `tests/phase2.rs` path run (us-production-2025, 300 s), read from the run the other cases share, no extra run | 6,383,782 |
| `i-s1-phase2-60s` | the I-S1 pair (§3), 60 s | 256,250 |

Both reproduce at `RAYON_NUM_THREADS` 1 and 2, and from the opt-level-0 build.

## 3. I-S1 on the two-provider deployment

`scms_scale::real_and_modelled_crypto_give_the_same_run_on_the_two_provider_deployment`
runs the Phase 2 path (Manhattan, every third RSU, the attacker, `us-production-2025`) for
60 s in `real` and in `modeled` crypto. Equal: the whole `RunReport`, the record digest,
the frame digest, the record count. Not vacuous: 13 devices (provider-a 6, provider-b 7);
verifications by issuer and verifier a←a 340, a←b 968, b←a 921, b←b 529; 2 reports filed;
11,725 receptions; 0 credentials refused. The earlier I-S1 test
(`credential_handoff.rs`) ran the grid with no SCMS.

## 4. Honest revocation on the new topology

`phase2::the_honest_revocation_path_holds_on_the_old_and_the_new_topology` runs the same
300 s path on `camp-reference` (one SCMS) and reads the default run (two providers):

| | camp-reference | us-production-2025 |
|---|---|---|
| Devices by provider | a 52 | a 25, b 27 |
| Revoked attacker + honest | 1 + 0 | 1 + 0 |
| Revocation latency | 49.0 s | 49.0 s |
| Frames transmitted | 77,605 | 77,605 |

The topology changes who certifies whom and the backend's millisecond link latencies. It
does not change what the fleet transmits. The air-size change from implicit certificates
(the certificate on the air is 77 B where the old explicit stand-in was about 144 B,
BUILD-STATUS WP-C1) applies to both topologies equally. It is not measured again here
against the pre-C1 engine, because no pre-C1 build of this topology exists.

## 5. Fleet scale (modelled crypto)

`scms_scale::long_fleet_scale_run_cost` runs `scenarios/revocation-latency.yaml`
(Manhattan D7 box, buildings, one RSU, both providers, the scenario's attackers) at its
study rates. Peak memory is from `/usr/bin/time -l` around one rate per process
(`V2XW_SCALE_RATE`, `V2XW_SCALE_S`).

| veh/h | sim s | vehicles | frames | reception attempts | wall s | wall / sim s | max RSS | reports → at MA | revoked attackers / honest |
|---|---|---|---|---|---|---|---|---|---|
| 1,500 | 120 | 52 | 31,496 | 705,706 | 31.6 | 0.26 | 148 MB | 3 → 0 | 0 / 0 |
| 6,000 | 120 | 201 | 118,577 | 10,620,309 | 295 | 2.46 | 231 MB | 912 → 396 | 3 / 0 |
| 24,000 | 70 | 483 | 163,221 | 34,818,434 | 926 | 13.2 | 552 MB | 455 → 184 | 0 / 0 |

Reception attempts grow with the square of the fleet, as every frame is followed to every
receiver in reach: 4× the vehicles gave 15× the attempts and 9× the wall time (1,500 → 6,000
veh/h). Per simulated second, 24,000 veh/h costs 5.4× the 6,000 run. Memory stays under
0.6 GB at 483 vehicles, so CPU and not RAM bounds a city-scale SCMS run on this machine.
Within 70 s the 24,000 run's reports reach the MA (184) but no revocation has been issued
yet. The scenario's revocation horizon is 600 s, which would be about 2.5 h of wall time at
this rate in the test profile. The default `long_` run (90 s at all three rates) takes about
35 min. The test also asserts
what must hold at any size: no backend error, no honest revocation, both providers
certifying, frames growing with the rate.

## 6. Mutation proof

Each fault was injected, the named test went red, and the fault was reverted
(scratchpad `mutate.py`).

| Fault | Test | Red with |
|---|---|---|
| Modelled verify refuses digests whose first byte is 0 (`v2xw-sec/src/crypto.rs`) | I-S1 pair | "the run reports differ between modes" |
| Region latency per link 1 ms → 1.001 ms (`v2xw-proto/src/scms/deployment.rs`) | `the_phase_2_path_reproduces_its_regression_digest` | "`phase2-path` no longer reproduces its committed regression digest" |
| The same | I-S1 pair (pinned record) | "`i-s1-phase2-60s` no longer reproduces …" |
| With more than one provider the MA decides on another subject's report (`v2xw-engine/src/phase2.rs` `start_cases_of`) | topology test | "us-production-2025: the attacker was not revoked" |

## 7. Open

- Release-build costs at these fleet sizes: they need the integration step's release
  binary (`docs/validation/test-run-cost.md` holds the earlier release ladder).
- Cross-platform digest equality: the conformance gate's CI comparison has never run, so
  the pinned records are for macOS/aarch64 until it does.
- Under the new profile `v2xw-sec` and `v2xw-proto` pass, with one exception:
  `v2xw-proto/tests/threshold.rs::a_malicious_member_is_caught_by_its_share_excluded_and_the_session_restarts`
  (`invalid_shares` 2, expected 1; 2 sessions). It fails identically at 95c9b30 with the
  old opt-level-0 profile, so it predates this package. It most likely follows from the
  per-provider runs of the D1 MULTI merge. Not fixed here.
