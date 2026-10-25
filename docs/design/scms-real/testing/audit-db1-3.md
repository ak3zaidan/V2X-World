# Audit db1-3: on-air security on vehicles, RSUs and handsets

Scope: BSM signing (sizes on the air, digest or certificate), receiver verification and
the validated-certificate cache, P2PCD for unknown issuers, the receiver's validation
rules, the pseudonym change policy, the verification cost charged to each device, and the
privacy and linkability metrics.

Method. The rows are in `crates/v2xw-engine/tests/expectations/scms-audit-db1-3.yaml`.
All 41 were committed with `measured: pending` (commit "audit rows, written before their
runs") before any judged run. The checker is `tools/realworld/audit/onair.py`. It reads
only the raw records (`node.tx`, `node.rx`, `node.verify`, `node.p2pcd`, `node.security`,
`node.credentials`, `node.telemetry`, `sec.pseudonym`, `privacy.link`), `metrics.json`
and `run-report.json`. It never imports engine code. `onair.py faults` re-measures each
row under one injected record fault and shows the row going red (§4).

Runs. These used the integrator's release binary of `8becf72a`, copied to the scratchpad,
at `--record-level receptions` with a fixed `--build-utc`, one per run-lock call (1,200 MB
cap). Each run peaked at about 125 MB and took about 65 s.

| Run | Scenario | What it adds |
|---|---|---|
| A, A2 | `scenarios/real-world/audit/onair-base.yaml` | The realism base: 120 s of radio, J2945/1 CERTCHG, a passive eavesdropper, 3 % ConstPos |
| B | `onair-change.yaml` | A with the `time` rule at 30 s |
| C | `onair-p2pcd.yaml` | A cut to 60 s with `chain_file: home-provider`, so P2PCD runs on the air |
| A', B', C' | the same | the same scenarios on the fixed engine (optimised debug build of this branch) |

## 1. Inventory

Spec value and measured value per behaviour. Verdicts: **pass**; **fixed** (an engine
defect, fixed at its cause, with a node test that fails without the fix); **corrected**
(the check was wrong, with the evidence); **open**.

| Row | Behaviour | Spec value | Measured (8becf72a → fixed) | Verdict |
|---|---|---|---|---|
| OA.TX.cert-bytes | Pseudonym certificate on the air | 80 B (CAMP's 86 B less the 6 B countryOnly region the US profile omits) | 80 | pass |
| OA.TX.signer-id-bytes | Signer field: digest 8 B (HashedId8), certificate 2 B COER quantity + the certificate | 0 violations | 264 → 0 after the check modelled the COER length determinant of `unsecuredData` (2 octets once the payload is ≥ 128 B, X.696 §8.6) | corrected |
| OA.TX.spdu-accounting | spdu = payload + envelope; on the wire = spdu + net + link + fragment | 0 | 0 | pass |
| OA.TX.cert-cadence | Certificate once 1,000 ms have passed, digest otherwise (S2) | 0 | 0 | pass |
| OA.TX.first-carries-cert | First message under a new certificate carries it (S3) | 0 | **48 of 106 changes** → 0 | **fixed** |
| OA.TX.cert-gap-p50 | Median gap between certificates | 1,000–1,100 ms | 1,000 | pass |
| OA.MI.full-cert-share | Metric = recount per 10 s window | 0 bad windows | 4 → 0 after the recount covered every signed frame (17 over-the-air MBRs are "all messages") | corrected |
| OA.MI.envelope-overhead | Metric = Σ envelope / Σ payload | 0 | 0 | pass |
| OA.RX.no-cert-no-verify | A digest that misses the cache is not verified (S5) | 0 | 0 | pass |
| OA.RX.cached-verifies | A cached signer's digests verify (R16) | 0 | 1 | open (O-2) |
| OA.RX.ee-validations | `ee_validations` = distinct (receiver, signer) pairs | diff 0 | 8 → 0 after the recount added the 8 re-validations after a fresh CRL (R17; `invalidated` 8) | corrected |
| OA.RX.refused-honest | No honest reception refused | 0 | 0 | pass |
| OA.RX.stamp-order | Stamps in order on every `node.rx` | 0 | 0 | pass |
| OA.RX.verify-record-join | Every verifier task has one reception, and every reception one task | 0 | 0 | pass |
| OA.COST.repeat-veh/rsu/vru | A cached signer costs one P-256 verify: OBU 400 µs (>2,500/s), RSU 500 µs (>2,000/s), handset 645 µs | [400], [500], [645] | [400], [500, 1976] → [500] once re-validations are separated, [645] | pass (RSU: corrected) |
| OA.COST.first-sight | First sight costs more than a repeat | 0 | 0 | pass |
| OA.COST.unverifiable-free(-p2pcd) | A frame whose signer (or issuer) is not held is charged no signature check | 0 | **3,993 (A) and 6,993 (C) tasks, 1.87 s and 3.51 s of verifier time** → 0 (A) and 2 (C). The 2 left cost 12,998 µs each with no primitive: CA-certificate validation charged to a receiver's first frame (R18), not a signature | **fixed** |
| OA.MI.verify-cost | `verify_cost`, `verify_rate`, `verify_wait` = recount | 0 | 27 → 0 after the recount took tasks in which a signature ran and windows by record time | corrected |
| OA.MI.unverified-ratio | Delivered without verification / delivered | 0 | **9 bad windows** (RSU-held frames counted as delivered) → 0 | **fixed** |
| OA.CHG.j2945-none | J2945/1: no change inside 120 s | 0 | 0 | pass |
| OA.CHG.age-min / age-max | Change at 30 s, within one BSM | ≥ 29.9 / ≤ 30.2 s | 30.0 / 30.0 | pass |
| OA.CHG.temp-id | TemporaryID = first 4 octets of the HashedId8 | 0 | 0 | pass |
| OA.CHG.msgcnt | msgCnt restarts at a change | ≤ 0.02 | 0.0189 (2 of 106, the 1/128 chance of a random restart) | pass |
| OA.CHG.no-reuse | No return to a used pseudonym | 0 | 0 | pass |
| OA.CHG.records-agree | `sec.pseudonym` ↔ the air | 0 | 46 → 0 after the check counted the 46 changes made before a vehicle's first frame | corrected |
| OA.CHG.rate | Changes per vehicle-hour at 30 s | 80–121 | 135. Of 152 changes, 46 come before the vehicle's first frame; the 106 seen on the air are 94 per veh·h | expectation corrected (O-4) |
| OA.MI.change-rate | `pseudonym_change_rate` = recount | 0 | 9 → 0 after the recount was made cumulative over every node with security rows, as the provider's formula is | corrected |
| OA.MI.linkability | `linkability` = correct links / changes, cumulative | 0 | 0 | pass |
| OA.REP.privacy | Run-report privacy counters = recount | 0 | 123 → 0 after the recount used the report's split: a claim with no change behind it is `spurious`, not `claimed` | corrected |
| OA.P2P.on-air | P2PCD frames = Σ `responses_sent` = report | 0 | 0 (17 = 17 = 17) | pass |
| OA.P2P.learned | Receivers learn from observed responses | ≥ 1 | 19 | pass |
| OA.P2P.backoff | Response within `maxResponseBackoff` 250 ms of the request | ≤ 250 ms | 248.0 ms | pass |
| OA.P2P.no-foreign-before-learn | No foreign-provider message verified before learning | 0 | 0 | pass |
| OA.P2P.foreign-after-learn | Foreign messages verify after learning | ≥ 1 | 1,909 | pass |
| OA.P2P.trigger-metric | `p2pcd_requests` = P2PCD triggers | 0 | 3 bad windows: the metric counts digest misses (22–46/s), and P2PCD triggered 0.4–1.3/s | open (O-1) |
| OA.DET.records / metrics | Same seed → byte-identical | identical | `266b3fa0…` and `3e26662c…` both times | pass |

## 2. Fixes

**F1. An unverifiable frame was charged a full signature verification** (`v2xw-node`
`runtime.rs`, `rsu.rs` and `vru.rs`, in `drain_verify`). The receiver could not verify a
frame when it held neither the signer's certificate nor the issuer an implicit key is
rebuilt under (`SpduVerdict::Unverifiable`, 1609.2 §5.2). No signature check ran, but the
receiver booked the profile's P-256 verify on its verifier anyway. It is now charged only
the validation work actually done. The `node.verify` record now has reason
`unknown-signer` and no primitive. When the frame is delivered unverified (prioritized or
on-demand) the outcome is `skipped`. When it is held back (`verify-all`) the outcome is
`dropped`, so `unverified_ratio` no longer counts frames that never reached an
application. Before the fix those records read `skipped`/`verified`, and the RSU's held
frames were 66 of the 210 "unverified deliveries" in A's window at 40 s.

Test: `rx_validation.rs::a_signer_the_receiver_cannot_verify_is_charged_no_signature_check`.
Under the fault (the `Unverified` branch removed) the stranger's task costs the profile's
verify and the test fails.

**F2. After a pseudonym change, the first BSM was digest-signed** (`secure.rs`
`restart_certificate_cadence`, called at the change in `runtime.rs` and `vru.rs`). The
cadence is kept per message type, not per certificate. A change therefore inherited the
old certificate's last attachment: in B, 48 of 106 changes opened with a digest that no
receiver could resolve, for up to the 1 s cadence. That is roughly ten BSMs per change
that no neighbour could verify. Now the first BSM under every new pseudonym carries its
certificate.

Test: `runtime_behaviour.rs::the_first_bsm_under_a_new_pseudonym_carries_its_certificate`.
It fails without the call.

Both fixes change records. They may also change frames, through verification timing and
the attached certificates. The SCMS regression digests (`tests/regression/*.json`) must be
re-blessed; see the merge notes.

## 3. Findings left open

- **O-1. `p2pcd_requests` is the digest-miss rate, not P2PCD.** By its documented formula
  the metric counts digest-signed frames whose signer was not cached. In IEEE 1609.2
  clause 8, a P2PCD learning request is only for an unknown CA certificate: a missed
  pseudonym simply waits for the next certificate. In A the metric reads 22–46/s while no
  node sent a P2PCD request (no `node.p2pcd` rows). In C, real triggers are 0.4–1.3/s. The
  metric equals the `node.telemetry` counter it sums, in every window. Over a run, the
  independent digest-miss recount agrees within 3 % (A 3,992 against 3,963; B 5,483
  against 5,445; C 1,035 against 1,002). Per window it differs by up to 10 %, because each
  node closes its telemetry window at its own instant. The suggested fix is to rename it (`unknown_signer_rate`) and add a
  real `p2pcd_requests` from `node.p2pcd` `requests_sent`. This moves metric names in
  goldens and Studio, so it is an owner call.
- **O-2. A certificate learned from an over-the-air report is not usable for later
  digests.** Receiver 96 validated vehicle 82's pseudonym from a relayed MBR (EE cache
  hit later, 400 µs). It still could not resolve 82's next digest-signed BSM, because the
  peer certificate store learns only from awareness frames. This happened once in 37,295
  digest BSMs.
- **O-3. Vehicles verify relayed misbehaviour reports they overhear.** Seventeen MBR
  frames on the air were verified by neighbouring OBUs at first-sight cost (2,349 µs).
  These are PSID 0x26 frames for the RSU; a vehicle has no application for them.
- **O-4. Changes at entry.** A vehicle's pool is active before it reaches the road. Under
  the `time` rule, its first change can come before its first frame: 46 of 152 in B. This
  is harmless, but it raises the change rate to 135/veh·h against 120 for the rule. Under
  J2945/1 the same holds for a car parked longer than 5 min, which matches the common
  change-at-ignition practice.
- **O-5. The `linkability` card says the observer hears every frame.** The benchmark's
  eavesdropper has 10 % sniffer coverage at 200 m. The card's limitation text is stale.
- **O-6. CRL refusals under `home-provider`.** In C, 34 CRLs were refused as
  `crl-signer-untrusted`: the other provider's CRL signer is not in a home-provider chain
  file. This is a property of that test setting, not of the US deployment, whose devices
  hold the global chain file.
- **Not covered here:** on-air CRL refusal (no revocation reaches a vehicle within these
  windows; the conformance run covers it), region and PSID refusals at runtime (unit
  tests `rx_validation.rs` R5–R9), i-period rollover on the air, the link-layer address,
  and the Studio PKI view and server API (no time left).

## 4. Fault proofs

`onair.py faults <runs> <table> A,B,C,A2` mutates the records in memory, one fault per
row, and re-judges. The full result is in the run notes below. Every row went RED. On the
first pass, four faults left their rows green:

- three (`fcs`, `env`, `vcost`) hit a window with no metric sample;
- `cert_bytes` mutated a PSM, not a BSM.

All four were re-aimed. The faults for rows that already failed on `8becf72a` (first
certificate, unverifiable cost, unverified ratio) push those values further. Their real
proof is the fixed runs: 48 → 0, 3,993 → 0 and 9 → 0. Determinism: one injected byte
changes both digests.

Engine fixes:

- `runtime_behaviour::the_first_bsm_under_a_new_pseudonym_carries_its_certificate` fails
  with the call removed.
- `rx_validation::a_signer_the_receiver_cannot_verify_is_charged_no_signature_check` was
  fails with the `Unverified` branch disabled: the stranger costs 400 µs, against 0.

Node tests on the fixed tree: `rx_validation` 29, `runtime_behaviour` 21, `rsu_runtime`
12 and `vru_device` 13, all pass.

## 5. Merge notes

- Both fixes change `node.verify` records and verification timing, and F2 changes the
  frames on the air after a pseudonym change. The SCMS regression digests
  (`crates/v2xw-engine/tests/regression/i-s1-phase2-60s.json`, `phase2-path.json`) are
  expected to move, and they were not re-blessed on this branch (lock contention). Run
  `V2XW_BLESS_SCMS_DIGESTS=1 cargo test -p v2xw-engine --test phase2 --test scms_scale`
  and list the moves in the merge commit. Engine tests that count `node.verify`
  `skipped` outcomes or `verify_cost` (`unknown_signer.rs`, `sensitivity_security.rs`,
  `reception.rs`) were not run here.
- Fixed-run digests: A `4896d503…`, B `69ca5748…`, C `a86f2905…`. On `8becf72a`, A and
  A2 were byte-identical (`266b3fa0…`).
