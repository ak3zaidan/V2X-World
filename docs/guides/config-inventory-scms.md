# SCMS and security parameter inventory

The 2026-10-18 audit of every certificate-system and security knob a researcher or a
deployment engineer would change. Each row says where the number lives and how a scenario
sets it, or why it stays fixed. The same-seed runs that show each knob moving what it
should are in `docs/design/scms-real/testing/scms-knobs.md`.

The rule used to classify:

- **Exposed** means a validated scenario field. It has a unit, a range, a plain title, an
  explanation, a cited default and a Basic or Advanced level, and the model reads it.
- **Model parameter** means a key of a model's free-form `params` block (`detection.local`,
  `detection.ma`, `net.uu`). The model validates the key and refuses a misspelling. The
  settings window edits the block as JSON.
- **Fixed** means a constant that a standard or the deployment profile fixes, or an
  implementation detail with no protocol effect. A simulator that let such a constant vary
  would no longer simulate the US system.

All of these settings sit under **Certificates & SCMS** in the settings window:
`security.scms.*`, `security.protocol`, `security.pseudonym_change`,
`security.signer_id_policy`, `security.threshold`, `actors.backend` and `detection.ma`.
Signature schemes, verification policy, crypto hardware and caches stay under
**Security & Crypto**.

## Exposed by this audit: `security.scms.lifecycle` and `security.scms.tls`

Before this audit the credential lifecycle's numbers could only be written as untyped
keys of `security.protocol.params`. The settings window showed that block as raw JSON
with no defaults, ranges or words. Each key is now a field of its own. The model reads the
typed field and the old key through one function (`phase2::lifecycle_map`), so both
spellings produce the same parameters; the typed field wins when a scenario sets both.
`tests/scms_knobs.rs` proves this key by key. Three knobs that the model held but no
scenario could reach are new: `max_download_polls`, `download_request_tolerance_s` and
`ma_max_open_cases`. The TLS version could only be set from a unit test before.

| Field | Level | Default (source) | Range | What moves (measured) |
|---|---|---|---|---|
| `i_period_s` | Advanced | 604,800 s (CAMP-EE §2.1.5.3.2) | 1 s to 1 y; anything other than a week needs `time.calendar: compressed` | the whole calendar |
| `cert_lifetime_s` | Advanced | 608,400 s: the week plus a 1 h overlap (CAMP-EE §2.1.5.3.2) | at least the i-period | overlap at the change of week |
| `certs_per_period` | Basic | 20 (CAMP-EE Table 2.1.2.6.2) | 1 to 60 | certificates per top-up, ×2 exactly |
| `pool_periods` | Advanced | 2 (engine) | 1 to 64 | the preload |
| `topup_below_periods` | Advanced | 1 (engine); 0 turns top-ups off | 0 to 64 | 0 starves the fleet after its preload |
| `max_periods_ahead` | Advanced | 156 (CAMP Primer p.7: 3,120 = 156 × 20) | 1 to 520 | the RA clips requests |
| `cert_shuffle_threshold` | Advanced, partial | 1,000 (CAMP 1.2.2 Table 67) | 1 to 10⁶ | no in-run effect: it acts on a device's first request, made before the vehicle joins; weekly top-ups are pre-generated and skip the shuffle |
| `cert_shuffle_window_s` | Advanced, partial | 172,800 s (CAMP 1.2.2 Table 67) | 0 to 604,800 s | as the threshold |
| `report_shuffle_threshold` | Advanced | 10,000 (CAMP-EE SCMS-765) | 1 to 10⁶ | report release |
| `report_shuffle_window_s` | Advanced | 86,400 s (SCMS-765) | 0 to 604,800 s | 5 s revokes sooner than 30 s |
| `first_batch_delay_s` | Advanced | 60 s (engine) | 0 to 86,400 s | the first poll |
| `download_poll_interval_s` | Basic | 300 s, `download-minWait` (SCMS Manager EE profile Table 3) | 0.1 to 86,400 s | a late batch only (ACA 4 s slow): mean top-up 10.2 → 15.6 s at 20 s |
| `max_download_polls` | Advanced | 100, `download-maxReqs` (EE profile Table 3) | 1 to 10,000 | a late batch only: 1 cuts completed top-ups 188 → 104 |
| `download_request_tolerance_s` | Advanced | 5 s (CAMP 1.2.2 SCMS-1203) | 0 to 3,600 s | the RA's replay window |
| `enrolment_lifetime_s` | Advanced | 6 y (CAMP 1.2.2 Table 3; owner decision) | 1 s to 10 y | re-enrolment |
| `reenrol_lead_s` | Advanced | 548 d (SM-REENR: 12 to 24 months) | below the lifetime | when re-enrolment starts |
| `reenrol_download_delay_s` | Advanced | 300 s (`download-minWait`) | 0 to 86,400 s | the re-enrolment download |
| `crl_cadence_s` | Basic | none: each provider's rule (US: within 1 business day, SM-PROV §3.4.9) | 0 (on decision) to 30 d | 0 publishes at once |
| `crl_fetch_interval_s` | Basic | 3,600 s (an ASSUMPTION inside CAMP's weekly bound) | 0.1 s to 2 weeks | 120 s gives fewer downloads |
| `crl_broadcast_interval_s` | Advanced | 5 s (USDOT RSU v4.1 Store and Repeat) | 0.1 to 3,600 s | roadside CRL repeat |
| `ma_review_s` | Basic | 8 h on the real calendar (pilots' manual MA; an ASSUMPTION: no time is published); automatic on a compressed one | 0 to 30 d | 60 s delays the revocation |
| `ma_max_open_cases` | Advanced | 1,000 (an ASSUMPTION) | 1 to 10⁶ | the MA's concurrent cases (before this audit the engine ran one at a time whatever the cap; fixed) |
| `security.scms.tls` | Advanced | `1.3` (SCMS Manager EE profile §2.1.18 admits 1.2 and 1.3) | `1.3`, `1.2` | 1.2 means fewer handshake bytes and a slower top-up |

The defaults the settings window shows come from the model's own code
(`phase2::lifecycle_defaults`), so they cannot drift from what a run uses.
`tests/scms_knobs.rs::the_published_defaults_are_the_models` writes every published default
back and requires the parameters to be unchanged.

The loader refuses:

- every field outside its range;
- an `i_period_s` other than one week on the real calendar;
- a `cert_lifetime_s` shorter than the i-period, which would leave a vehicle a gap with no
  valid certificate;
- a `reenrol_lead_s` at or above `enrolment_lifetime_s`;
- a TLS version other than 1.2 and 1.3;
- the whole `security.scms` block on any protocol other than `protocol/scms/camp`.

## Already configurable before this audit

| Knob | Path | Notes |
|---|---|---|
| Deployment preset and providers | `security.scms.deployment.preset` (`us-production-2025`, `us-test-ctl-2025`, `camp-reference`, `custom`), `providers[]` (`components`, `region`, `fleet_share`, `on_ctl`, `ma_mode`, `ma_review_s`, `crl_publication`) | the fleet split is `fleet_share`, and regions are labelled as assumptions |
| Electors and quorum | `security.scms.deployment.electors`, `quorum` | 5 / 3; changing them flags the run as non-US |
| Backend latency model | `security.scms.deployment.latency` (`region`, `flat`), `fibre_fraction`, `per_link_ms` | the region model is an ASSUMPTION |
| MA report sharing | `security.scms.deployment.ma_report_sharing` | |
| LOP, butterfly | `security.scms.deployment.options.lop`, `butterfly` | only `original` is admitted (owner decision) |
| Chain file | `security.scms.chain_file` | `all-providers`, `home-provider` |
| Pseudonym change | `security.pseudonym_change.{strategy, period_s, distance_m, silent_period_s, mix_zone_radius_m, mix_zone_min_age_s}` | |
| Certificate or digest | `security.signer_id_policy.{full_cert_every_ms, digest_otherwise, on_new_neighbour}` | this also drives P2PCD demand |
| Verification policy | `security.verification_policy`, `on_demand_threshold`, `prioritized_horizon_m` | |
| Signature, KEM, PQ and hybrid | `security.signature`, `security.kem`, `security.crypto_mode` | |
| Threshold committees | `security.threshold.*` | |
| Crypto hardware | `nodes.default_obu`, `nodes.per_class`, `nodes.hsm_sign_lower_us`, `nodes.compute_tier`, `nodes.backend_tier` | |
| Peer certificate cache | `nodes.peer_cert_cache` | |
| Verification queue | `nodes.verify_queue_depth` | |
| Backend entities | `actors.backend.entities.<role>.{profile, service_model}`, `actors.backend.links`, `actors.backend.failures` | RA down, extra latency |
| Backend network | `net.backend_net` (`latency_ms`, `capacity_mbps`) | |
| Cellular access and eMBMS CRL | `net.uu` (`cellular/uu/*`; the `mbms_*` params) | the eMBMS bearer delivers CRLs |
| RSU roles and backhaul | `actors.rsus[].roles` (`crl`, `report-forward`, `provisioning-proxy`), `backhaul`, `backhaul_params` | the application-certificate cadence follows the i-period |
| Root rollover | `security.protocol.params.root_rollover_at_s`, `root_removal_at_s` | governance events, kept as protocol params |
| ETSI butterfly | `security.protocol.params.etsi_butterfly` | ETSI PKI only |

## Model parameters (edited as the model's `params`)

| Knob | Path | Keys |
|---|---|---|
| Detector suite and thresholds | `detection.local[].params` | `consistency_threshold_m`, `heading_threshold_deg`, `detector_lag_s`, `z_threshold`, `min_consecutive`, `sybil_min_certs`, `sybil_cell_m`, `art_max_m`, `max_accel_mps2`, `stale_max_s`, the `heading_*` keys, `offroad_confidence_bound`; host keys `report_interval_s` (the per-subject report rate limit, 1 s), `use_stated_accuracy`, `use_map` |
| MA corroboration | `detection.ma.params` (`threat/ma/corroborated`) | `event_window_s`, `event_min_reporters`, `min_events`, `window_s`, `min_reporters`, `defence`, `reputation_max`, `report_budget` |
| Legacy MA | `detection.ma.params` (`threat/ma/legacy-window`) | `report_threshold_k`, `revoke_min_seconds`, `revoke_persist_s`, `revoke_window_s`, `defence`, `reputation_max`, `report_budget` |

## Fixed, with the reason

| Constant | Where | Value | Why it stays fixed |
|---|---|---|---|
| CRL overdue tolerance | `v2xw-node/src/rxval.rs` `OVERDUE_CRL_TOLERANCE_S` | 2 weeks | CAMP 1.2.2 Table 67 `overdue_CRL_tolerance`: a profile constant every device applies. OPEN: worth exposing for attack studies that delay CRLs; it needs a path through the node runtime. |
| `nextCrl` | `params.rs` `CrlIssuance::next_crl` | 1 week | CAMP 1.2.2 §3 item 10 |
| Business-day CRL slot | `CrlIssuance::slot_et` | 10:00 ET | ASSUMPTION: the SLA fixes the deadline, not the hour |
| Validated-certificate cache | `rxval.rs` `EE_CACHE` | 4,096 | an implementation choice with no protocol effect (constants.md). OPEN: expose it with `nodes.peer_cert_cache` if a cache-exhaustion attack needs it |
| Report store quota | `phase2.rs` `REPORT_STORE_QUOTA_BYTES` | 64 KiB | SM-MBR §2.2 leaves it device-specific; ASSUMPTION. OPEN |
| Report maximum age | `v2xw-node/src/stores.rs` `REPORT_MAX_AGE` | 1 week | SM-MBR |
| LA-ACA key rotation | `PduLayer::la_aca_key_rotation_s` | 1 week | ASSUMPTION; key event only, with no per-request traffic |
| Seed retry | `ScmsParams::seed_retry` | 30 s | uncited model parameter, kept equal for the committee comparison |
| Backend servers and overheads | `ScmsParams::backend_servers`, `backend_overhead`, `device_overhead` | 4, 1 ms, 1 ms | set per entity through `actors.backend.entities.<role>.service_model` |
| RSU application-certificate backhaul | `v2xw-proto/src/scms/run/appcert.rs` `RSU_BACKHAUL_LATENCY`, `RSU_BACKHAUL_BPS` | 10 ms, 1 Gbit/s | the engine's access model carries the RSU's real backhaul; these constants only price the kernel's own leg |
| P2PCD backoff | `v2xw-net/src/frag/cert_cycle.rs` `P2PCD_MAX_BACKOFF_MS` | 250 ms | IEEE 1609.2 §8.4 |
| PSIDs, ASN.1 bounds, header sizes | `v2xw-msg`, `v2xw-sec` | | fixed by the standards |
