# Security, certificates, attacks and backend: does each setting move what it should?

Security track, 2026-10-13, release `v2xw` built from e94e43c. Method: one setting (or two,
as a 2 x 2) changed on the same seed, against an expectation written from theory or a
cited source before the run. Rows: `crates/v2xw-engine/tests/expectations/security-track.yaml`
(one setting, routed by the families file) and
`crates/v2xw-engine/tests/expectations/interactions/security-track.yaml` (attackers,
revocation, pseudonyms and load, which need a longer or heavier base). Rerun:

    v2xw sensitivity --expect \
      --expectations crates/v2xw-engine/tests/expectations/security-track.yaml \
      --interactions crates/v2xw-engine/tests/expectations/interactions/security-track.yaml

Families: `security` is the CAMP SCMS lifecycle (`credential-lifecycle.yaml`, 30 s, 9
vehicles, one RSU, no attacker); attacker and revocation checks run it 90 s with 2 s report
and 5 s CRL windows. `dsrc` is 40 vehicles at 10 Hz; "load" is 160 vehicles at 20 Hz on the
software-only OBU (`obu/generic-automotive-soc-no-hsm`).

## Results (first run: 36 of 41 pass)

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| `security.signature` | falcon-512 | security bytes on air | up | 207.7 k -> 1.66 M | yes |
| `security.signature` | falcon-512 | CBR | up | 0.0024 -> 0.0088 | yes |
| `security.signature` | ml-dsa-65 | SDUs fragmented | up | 0 -> 1941 | yes |
| `security.signature` | ml-dsa-65 | e2e latency | up | 3.77 -> 14.8 ms | yes |
| `security.signature` | slh-dsa-sha2-128s | sign stage | up | 0.244 -> 6788 ms | yes |
| `security.signature` | slh-dsa-sha2-128s | PSDU | up | 208 -> 1370 B | yes |
| `security.signature` | hybrid Falcon+ECDSA | security bytes | up | 207.7 k -> 1.81 M (> Falcon alone) | yes |
| `security.signature` | brainpoolP256r1 | PSDU | same | 208.3 = 208.3 | yes |
| `security.envelope` | etsi103097 | PSDU | same | identical | yes |
| `signer_id_policy.full_cert_every_ms` | 500 | full-cert share | up | 0.101 -> 0.201 | yes |
| `signer_id_policy.full_cert_every_ms` | 2000 | P2PCD requests | up | 9.63 -> 14.86 | yes |
| `signer_id_policy.digest_otherwise` | false | full-cert share / P2PCD | up / down | 0.10 -> 1.0 / 9.6 -> 0 | yes |
| `signer_id_policy.on_new_neighbour` | true | full-cert share | up | 0.101 -> 0.112 | yes |
| `pseudonym_change.period_s` | 1 s | P2PCD / changes | up / up | 9.6 -> 12.4 / 0 -> 195 | yes |
| `verification_policy` | on-demand | verify rate | down | 111 -> 14.6 /s | yes |
| `verification_policy` | prioritized | verify rate | same | identical | yes |
| `on_demand_threshold` | 1.0 / 0.05 | verify rate | down / up | 14.6 -> 14.0 / 56.1 | yes |
| `nodes.default_obu` | software SoC | verify stage | up | 0.40 -> 0.645 ms | yes |
| `nodes.default_obu` | software SoC | sign stage | up | 9.0 -> 0.244 ms | **no: expectation corrected** |
| `nodes.peer_cert_cache` | 16 / 1024 | P2PCD | up / same | 1510 -> 7195 / identical | yes |
| `nodes.verify_queue_depth` | 256 (light load) | unverified share | same | identical | yes |
| attackers ConstPos x3 | vs none | detections TP | up | 0 -> 485 (483 under legacy-window MA) | yes |
| attackers ConstPos x3 | vs none | attackers revoked | up | 0 -> 3 | yes |
| attacker count 1 -> 4 | ConstPos | detections TP | up | +390 (+413) | yes |
| DataReplay x3 | vs none | detections TP | up | +502 | yes |
| Sybil x2 | vs none | ghost frames | up | 0 -> 6840 | yes |
| DoS x2 | vs none | frames on air | up | refused by the loader | **no: not modelled** |
| constant jammer 23 dBm | vs none | PRR | down | -0.081 | yes |
| `crl_cadence_s` 5 -> 30 | ConstPos x3 | revocation latency | up | +5.0 s (+15 s under legacy-window) | yes |
| `crl_fetch_interval_s` 5 -> 30, x MBMS | ConstPos x3 | revocation latency | up, MBMS dampens | 3.10 s in all four runs | **no: see below** |
| `pseudonym_change.period_s` 5 s | x peer cache 16 | P2PCD | up | +5.05 | yes |
| strategy silent | period 5 s | silenced frames | up | 0 | **no: wrong setting** |
| strategy silent | x eavesdropper | correct links | down | -33 | yes (for the wrong reason, see below) |
| `period_s` 5 s | x eavesdropper | changes | up | +38 | yes |
| `verify_queue_depth` 6 | load, x on-demand | unverified share | up | 0.437 -> 0.438 | **no: expectation corrected** |
| software crypto | x load | verify wait | up, amplified | +0.11 ms light, +26.4 ms loaded | yes |
| `prioritized_horizon_m` 50 | load | e2e latency | changes | +1.19 ms (+3.18 at depth 256) | yes |

## What the failures were

- **Software signing is faster than the reference HSM (expectation wrong).** The Craton2
  profile signs at the vendor's published "<9 ms" ceiling (R7 A4); a Cortex-A72 signs P-256
  in 244 us (R5 B.2). The row now expects `down`. Note for the profile owner: 9 ms is an
  upper bound used as the latency, so it likely overstates signing time on the reference OBU.
- **DoS:** the loader refuses `threat/attacker/legacy/DoS` by name (a flood needs more than
  one frame per generation interval). Correct refusal; row removed, listed as not modelled.
- **CRL fetch interval and MBMS changed nothing:** the metric is detection to *first*
  vehicle enforcing, and the RSU's `crl` role broadcasts the list over the air before any
  cellular poll. The row now removes the RSU's `crl` role so the poll path decides.
- **`strategy: silent`** means no scheduled change (validate.rs note), not a silent period.
  The silent period is `silent_period_s`; both silent rows now set `[3, 13]` s (PRESERVE).
- **Verify queue depth under overload:** M/M/1/K with rho > 1 serves at the service rate
  whatever K, so the loss share barely moves; the depth bounds the wait. The row now expects
  `stage_verify_wait_ms` down, dampened under on-demand.

## Settings that moved nothing in the catalogue sweep, and why

| Setting | Verdict |
|---|---|
| `threats.compromised_rsu_attack` | **Fixed**: accepted and ignored on any run without the credential lifecycle (only phase2 refused it). The loader now refuses it when `compromised_rsus` is empty; test `a_compromised_rsu_attack_without_units_is_refused`. |
| `pseudonym_change.distance_m` | **Fixed (sweep)**: the family runs the time strategy, which ignores it. Families-file context now sweeps it (and `mix_zone_*`) under its own strategy. |
| `pseudonym_change.period_s` / `distance_m` under another strategy | Open: accepted and ignored. Refusing would break the Studio's strategy quick setting, which changes only the strategy on scenarios that set `period_s`. Needs the quick setting to drop the other strategy's fields first. |
| `security.kem` | Live, but the 30 s family downloads no certificate batch; 150 s interaction added. |
| `security.crypto_mode: real`, eavesdropper default, `envelope`, brainpool | Same by design (rows above). |
| `net.backhaul`, `net.backend_net` | Routed to the GN family, which has no backend; 150 s relay-only interaction added on the lifecycle family. |
| `nodes.compute_tier: high`, `nodes.backend_tier` | `partial` in the catalogue: `high` equals `medium` in this build. Open. |
| `security.prioritized_horizon_m`, `verify_queue_depth` | Dead at light load (the queue is never more than one deep); live under load (rows above). |
| `mix_zone_radius_m`, `mix_zone_min_age_s`, `silent_period_s` | Not sweepable (no recommended range in guide.rs; a list). Covered by interactions. |
