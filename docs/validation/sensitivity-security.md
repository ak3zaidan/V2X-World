# Sensitivity: security, message and backend settings

Each row runs a baseline and one setting with the same seed and the same small fleet, and
asserts the direction the cause predicts. A setting that changed nothing fails its row.
Tests: `crates/v2xw-engine/tests/sensitivity_security.rs`. The 17 tests take about 110 s
together in a dev build with `opt-level=2`.

**Fleet.** A procedural 5 x 8 grid, about 1.1 x 0.6 km, in open air. 20 to 60 vehicles
spawn at t = 0, all equipped, sending 802.11p BSMs at 10 Hz with `verify-all`.

**Backend rows** add the CAMP SCMS on a compressed calendar (20 s i-periods, 3
certificates per period, a top-up when only the current period is left) and a cellular
modem in every vehicle (`4g-east-coast`), unless the row says otherwise.

**Numbers** come from the run report or from the published metric samples
(`metric.sample`). A sample's value is weighted by its sample count and pooled over
windows; counts are summed.

## The table

| Setting | Values (baseline first) | What moved | Why |
|---|---|---|---|
| `security.signature` | ecdsa-p256 → hybrid-mldsa44-ecdsa-p256 → ml-dsa-44 | Security bytes 96 kB → 2.60 MB → 2.53 MB. Fragments 0 → 2,068 → 2,068. Security share of the air 0.52 → 0.95 → 0.95. CBR 0.073 → 0.77 → 0.74. PDR 0.96 → 0.59 → 0.62. E2E latency 3.9 → 28.3 → 27.0 ms. Unverified share 0.016 → 0.57 → 0.59 | The ML-DSA-44 signature is 2,420 B and the public key 1,312 B (FIPS 204 Table 2). With a certificate the message is larger than the MTU, so it is fragmented, and a lost fragment loses the whole message. Hybrid carries both signatures. |
| `security.signer_id_policy.full_cert_every_ms` | 1000 → 0 (every message) → 5000 | Certificate share 0.10 → 1.00 → 0.033. Security bytes 192 kB → 415 kB → 175 kB. CBR 0.074 → 0.119 → 0.074. Unverified share 0.010 → 0 → 0.041. P2PCD requests (`p2pcd_requests` **new**, 30 vehicles, 6 s) 491 → 0 → 2,029 | A certificate is about 120 B more than the 8-byte digest. The rarer it is, the longer a newcomer holds digests it cannot check. The default cadence is J2945/1's 1 s (05-protocols.md §2.4). |
| `security.signer_id_policy.on_new_neighbour` **new** | off → on (5 s cadence, 5 s pseudonym period) | Certificate share 0.020 → 0.060. Security bytes 379 kB → 401 kB. Unknown-signer fates 4,578 → 449. Unverified share 0.065 → 0.006. AoI 154 → 77 ms | This is the ETSI CAM rule (TS 103 097 V1.3.1 §7.1.1): a station that hears a station it does not know puts its certificate in its next CAM. The loader refuses the setting when every message already carries the certificate, or when no BSM or CAM runs. |
| `security.verification_policy` | verify-all → on-demand (software ECDSA) | Verifications 8,105/s → 813/s. Unverified share 0.020 → 0.97. Verify wait 0.26 → 0.13 ms. E2E latency 5.3 → 3.6 ms | On-demand verifies only strangers and signers an application marks relevant (06-node-models.md §2.1). |
| `nodes.default_obu` | Craton2 HSM → Craton2 software → SoC without HSM | Stages for HSM vs software: sign 9.0 vs 0.61 ms, verify 0.40 vs 1.29 ms. Verify wait 0.003 → 0.26 → 0.037 ms. E2E latency 12.3 → 5.3 → 3.9 ms | The eHSM signs in under 9 ms (R7 §A4). The line-rate engine verifies in at most 400 µs (derived from 2,500/s). In software the same unit signs in 613 µs and verifies in 1,290 µs (Pi 3B proxy, R7 §D8). The HSM unit is slower end to end because it signs slower. |
| `nodes.verify_queue_depth` | 64 → 1 (60 vehicles, software verify) | Verify-overflow fates 0 → 11,917. Verifications 29,240/s → 24,864/s. Unverified share 0.055 → 0.089. AoI 60 → 75 ms | A full queue refuses the newcomer (06-node-models.md §2.2). |
| `security.pseudonym_change.period_s` | 300 → 5 s (20 s run, 5 s cadence) | Pseudonym changes 0 → 60. Unknown-signer fates 1,275 → 4,578. Unverified share 0.018 → 0.065. AoI 68 → 154 ms | Each new pseudonym is heard first by digest, and receivers cannot check it until its certificate comes round (the P2PCD case). |
| `security.pseudonym_change.silent_period_s` (5 s period, 20 s run, 1 s cadence) | none → [2, 4] s | Linkability against the global observer 0.88 → 0.20 (53 → 12 of 60 changes bridged). Frames 3,955 → 2,086. Unknown signers 843 → 724 (was 4,051 before the fix below). P2PCD requests against the 300 s baseline: 377 → 843 at 5 s | The tracker (Wiedersheim et al., WONS 2010) predicts where a vehicle will be; a 2-4 s gap breaks the prediction (Huang et al., "silent cascade", SECON 2005). Silence costs awareness frames. |
| `messages.rate_hz` (50 ms step) | 5 → 10 → 20 Hz | Frames 450 → 899 → 1,790. Security bytes 54 → 96 → 179 kB. CBR (sampled) 0.003 → 0.077 → 0.152. AoI 115 → 65 → 39 ms | The load follows the rate (airtime ratio 2.0 for each doubling). The 5 Hz CBR is under **Limits**. |
| `messages.path_history_points` **new** (30 vehicles, 12 s) | 15 → 1 → 0 | Payload 202,734 → 202,580 → 166,610 B. Air bytes 741,606 → 741,452 → 705,482 B. Envelope/payload 1.895 → 1.897 → 2.306 | Part II's path history is about 10 B a message here. A 12 s drive on a straight grid keeps about one point, because a chord may run 300 m (J2945/1), so the cap binds at 1 and not between 1 and 15. |
| `security.envelope` (GN/BTP, CAM + DENM, 5 s cadence) | ieee1609.2 → etsi103097 | Security bytes 715,354 → 717,469 B. Certificate share 0.033 → 0.035. 19 DENMs in both | TS 103 097 is a profile of 1609.2, so a CAM is byte-identical under both (§5.1-5.2). What differs is that a DENM must carry a certificate (§7.1.2). |
| `net.uu.params.preset` / `penetration` | 5g-first-hop → 4g-west-coast; penetration 1 → 0.5 | Top-up exchange 1,136 → 1,259 ms. At half penetration: 10 vehicles with a modem instead of 20; 10 vehicles starved, 39 s without a valid certificate | The presets are the measured Uu latencies (`UuLatencyPreset`). A vehicle with no modem and no relay cannot top up. |
| `actors.rsus[].backhaul_params` (no modem, 8 provisioning proxies) | 200 → 600 ms; 200 ms at 64 kbit/s; 5 ms | Top-up 1,884 → 3,484 ms; 64 kbit/s 2,181 ms. 5 ms: 2,142 ms over 320 messages instead of 240 | The exchange is a request, an ack and download polls 1 s apart, so it crosses the line 4 times one way (600 ms adds 1.6 s). A 5 ms line is not the fastest: its first poll arrives before the batch is ready, so a second poll is needed a second later. |
| `security.protocol.params.crl_fetch_interval_s` / `crl_cadence_s` (20 % attackers) | fetch 5 → 30 s; cadence 10 → 30 s | Revocation latency (detect → first enforcement) 7.38 → 8.08 s for the fetch interval; 7.38 → 17.38 s for the cadence. Published at 7.23 s in both fetch runs | A list is published only when the authority revokes. A vehicle downloads only a version newer than its own, at its next poll. The cadence decides when the list is published, and the fetch interval decides the last leg. |
| CRL size (follows revocations; `threats.attackers[].fraction`, 60 s, cadence 10 s, fetch 5 s) | 0.1 → 0.3 | Revoked 3 → 7. CRL entries 3 → 7, CRL bytes 332 → 492 (40 B an entry). Downloads 30 → 30 | One entry per revoked certificate. Total cellular downlink (56.8 → 55.5 kB) does not show it, because revoked vehicles stop topping up, which outweighs 30 downloads of 160 B more. |
| `threats.attackers[].fraction` (ConstPos, legacy-12, corroborating MA) | 0 → 0.1 → 0.3 | Attackers 0 → 3 → 8. Verdicts 27 → 1,986 → 4,286. Report-level TP 0 → 6 → 15; FP 2 → 2 → 1. Revoked attackers 0 → 3 → 7, honest 0 | A pseudonym is a subject, so 3 attackers make 6 true-positive subjects. An honest fleet's 27 verdicts are the detector suite's false positives; the authority's persistence gate keeps them from revoking anyone. |

## What was fixed

**Certificate on a new neighbour** is a new validated setting:
`security.signer_id_policy.on_new_neighbour`.

- It is in the schema, `validate.rs` (two refusals and the key status), `guide.rs`
  (Advanced) and the label in `options.rs`.
- It is read in `v2xw-node` `NodeSecurity::signer_id_for`. The runtime notes a new
  neighbour before adding it to the neighbour table.
- It is off by default, so no digest moves.

The first wiring made things worse: unknown signers went from 843 to 3,948 when a fleet
changed pseudonym together.

- The cause: every vehicle heard newcomers just before its own change, spent the forced
  certificate on its old pseudonym, and the restarted cadence then held back the new
  certificate for a full period.
- The fix: a certificate sent only for a new neighbour no longer restarts the cadence.
  The two triggers are independent, as in the ETSI rule. Unknown signers then fell from
  843 to 105.

**The test reads node-level losses from `node.rx`.** The run report's `rx_losses` holds
only the PHY's causes, so `verify-overflow` and `unknown-signer` never appear there. That
is by design (invariant I-R3).

**A silent period no longer hides the new certificate.** The silence discards the safety
messages a node signs during it, but only after signing. The certificate cadence counted
those frames as sent, so the first frame after the silence named the new pseudonym by
digest, and no neighbour had heard its certificate. Unknown signers rose from 843 to 4,051.
The engine now calls `NodeSecurity::withdraw_certificate` for each discarded frame, so the
cadence restarts with the first frame on the air. Unknown signers fell to 724.

**Path history length is a new validated setting:** `messages.path_history_points`
(0 to 23, J2945/1's 15 unless set). It is in the schema, `validate.rs` (bounds, refused
without `bsm` or `cam`, key status), `publish.rs` (points), `guide.rs` (Advanced) and the
label in `options.rs`. The node reads it as the path-history builder's `max_points`
(`OwnVehicle::with_path_history_points`). Unset, nothing moves.

**P2PCD requests are a metric.** 08-measurement-and-data.md §2.2 specifies
`p2pcd_requests`, and the nodes already counted them in their telemetry window, but no
channel carried the count. `node.telemetry` now carries `p2pcd_requests` and
`metric/security/envelope-and-verification` publishes it in 1/s. The test sums the window
counts, because a count-weighted mean of rates over-weights a busy window.

## Limits found

**CBR at 5 Hz reads about 0 in a fleet spawned in one instant.**

- The MAC reports the CBR of its last 100 ms at each 1 s flush.
- A node's phase moves its generation inside a mobility step, not across the message
  period, so a fleet that starts together sends at 5 Hz only in the even 100 ms slots.
- The sample therefore misses every frame. Airtime and offered load are correct.
- Fixing it needs either a CBR averaged over the whole window, or a phase spread over the
  period. A wider phase window does not do it, because the phase is folded into the step.

**The pseudonym period's effect follows the certificate cadence's phase.** With a 5 s
cadence and a 5 s period, every change falls just before a certificate, and 100 changes
cost fewer P2PCD requests (7,932) than 20 expiry changes in the 300 s baseline (17,365),
because those fell just after one. The row therefore uses the 1 s cadence. A pseudonym
change does not force a certificate in this build. ETSI's rule (send one when an unknown
station appears) is `on_new_neighbour`.

**Pseudonym changes are counted only with a backend.** `phase2.pseudonym_changes` and
`sec.pseudonym` come from the backend's tracker. Without `security.protocol`, nodes do
change pseudonym (the AoI and the unknown signers move) but the count reads 0. The row
uses the backend.

**The verify load does not rise with the certificate cadence.** At 0 ms and at 1000 ms the
rate is about 8,280/s. A certificate is verified once and cached, so every message costs
one verification either way. Only first sightings would add work.

**Detection counts are cumulative.** `det_*` is the confusion matrix so far, so the test
reads the last window rather than a sum.

**Not configurable, so not a row:**
- BSM Part II beyond the path history: the events, lights and path prediction follow
  the vehicle's state, not a setting.
- CRL size: it follows the number of revocations, not a setting. Its row moves it through
  the attacker share.
- An explicit cellular "availability" model beyond `penetration` and coverage.
