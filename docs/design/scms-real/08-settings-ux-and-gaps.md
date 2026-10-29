# SCMS settings: a menu for every setting, and the four gap specs

Date: 2026-10-28. Base: `main` at `0570929c`. Status: plan, not built.

The owner asked for two things. First, the SCMS part of the settings window should give a
menu for every setting, so nobody has to know what to type. Second, the four gaps from the
last report should be added: exact server locations, server capacity, a pluggable MA policy
with a family sweep, and deeper CA hierarchies. The owner left every choice to "whatever
makes the engine most realistic". This page records each choice made on that rule (§9).

This page has six parts:

* §1: why `butterfly` and its neighbours are bare text boxes today.
* §2: every SCMS setting, with today's control and the control proposed for it.
* §3: the grouping, order and help text of the SCMS panel.
* §4: the engine and UI changes the menus need.
* §5–§8: specs for the four gaps.
* §10–§11: the check that keeps settings from going menu-less again, and the build schedule.

---

## 1. How a setting gets a menu today, and why many SCMS ones have none

The engine publishes the form. The UI draws whatever the engine publishes.

* `publish.rs` `Walk::node` (`crates/v2xw-engine/src/scenario/publish.rs:880`) marks a
  string leaf as `kind: "enum"` (a drop-down) only when `with_meta` added an `enum` or an
  `x-options` to it. Otherwise the leaf stays `kind: "string"`, and the UI draws a text box.
* `with_meta` (`publish.rs:1025`) adds `enum` only from `validate::CHOICES`
  (`validate.rs:2158`). `with_options` (`publish.rs:1113`) adds `x-options`, meaning a label
  and a sentence for each value, from one of three sources. The first is the `enum` it was
  given. The second is a registry slot (`options::slot_values`, `options.rs:1025`). The third
  is `options::extra_values` (`options.rs:1177`).
* `options::option_row` (`options.rs:962`) takes its words from `words()` (`options.rs:34`).
  When `words()` has no row for a value, the label is the humanised id and the description
  is the fallback text. For a `CHOICES` row the fallback is empty, because only a reflected
  enum carries `x-enum-descriptions`. So a value can appear in a menu with no explanation.

**Root cause:** most of the deployment code validates its strings in hand-written `match`
arms with their own "allowed: …" lists. None of those lists is a `CHOICES` row or an
`extra_values` arm, so the schema never learns the set of allowed values:

| Setting | Where the allowed values live today |
|---|---|
| `security.scms.deployment.options.butterfly` | `Deployment::butterfly` (`crates/v2xw-proto/src/scms/deployment.rs:622`), called from `phase2::deployment_of` (`crates/v2xw-engine/src/phase2.rs:8881`) |
| `security.scms.deployment.preset` | `Preset::IDS` (`deployment.rs:105`), parsed in `deployment_of` (`phase2.rs:8735`) |
| `…providers[].region` | `REGIONS` (`deployment.rs:190`), `phase2.rs:8801` |
| `…providers[].ma_mode` | a literal `match` in `phase2.rs` (after 8826) |
| `…providers[].crl_publication` | `CrlPublication::IDS` (`deployment.rs:149`), `phase2.rs:8859` |
| `…deployment.latency` | a literal `match` in `phase2.rs` (after 8886) |
| `security.scms.chain_file` | a literal `match` in `home_chain_file_of` (`phase2.rs:8672`) |
| `security.threshold.protocol`, `.umbrella_inner`, `.roles[]` | `THRESHOLD_IDS`, `THRESHOLD_ROLES` (`crypto_config.rs:20`, `v2xw-proto/src/threshold/mod.rs:359`) |
| `actors.backend.failures[].mode`, `.node` | `FAILURE_MODES` (`crypto_config.rs:22`) and the role list in the `FailureNodeError::Role` message |
| `actors.backend.entities` keys, `links[].from/to` | `SCMS_ENTITIES` (`phase2.rs:9067`) |
| `actors.backend.entities.*.service_model` | a literal `match` in `apply_backend_topology` (`phase2.rs:9139`); the slot is open and lists the whole service-model registry |

**The UI is mostly ready for this.** `ChoiceSelect` (`ui/apps/studio/src/settings/widgets.tsx:40`)
already draws a drop-down from `x-options`, puts each option's sentence under the chosen
value, and disables refused values (`available: false`). Numbers already show their unit, range
(`x-range`) and presets (`x-presets`). Rows inside a list (`providers[]`, `failures[]`) get a
row editor whose fields carry their own choices (`ui/apps/studio/src/lib/schema.ts:438–468`).
The UI has five real gaps (§4.2):

1. A **map** (`actors.backend.entities`, `providers[].components`) can only be edited as
   JSON (`schema.ts:449`: "a nested list or map stays inside the JSON"). So even the
   `entities.*.profile` slot menu that the engine already publishes never appears.
2. The reason a value is refused exists only as an `<option title>` tooltip. Chrome and
   Safari on macOS do not show those tooltips.
3. The only description shown is the chosen value's. You can't read what the other values
   do before you pick one.
4. Nothing hides a setting that the current choices make irrelevant, such as `ma_review_s`
   under `automatic` or `extra_ms` when the failure mode is not `slow`.
5. Sub-headings come from the path (`subKeyOf`, `ui/apps/studio/src/settings/model.ts:410`),
   so all 30 `lifecycle.*` keys land under one heading, "lifecycle".

There are two more gaps on the engine side:

* Several numbers have no bound row in `validate::BOUNDS`. Those controls show no range.
  The list: `electors`, `quorum`, `fibre_fraction`, `per_link_ms`, `fleet_share`,
  `providers[].ma_review_s`, `signer_id_policy.full_cert_every_ms`, every
  `security.threshold.*` number, and `failures[].from_s/to_s/extra_ms`.
* Ten SCMS settings exist only inside the free-form `security.protocol.params` JSON. They
  are in `phase2::LIFECYCLE_KEYS` (`phase2.rs:1213`) but are not fields of `LifecycleSpec`:
  `ee_retry_timeout_s`, `ee_retry_max_per_hour`, `crl_background_entries`, `clock_skew_s`,
  `clock_skew_share`, `root_rollover_at_s`, `root_removal_at_s`, `peer_root_removal_at_s`,
  `modelled_reports` and `etsi_butterfly`. The window does not show them at all.

---

## 2. The settings audit: every SCMS setting

Key to the table:

* **Today** is what the window draws on `main` now: *text* (a free text box), *select*
  (a drop-down with words), *select, no words* (a drop-down whose options show a bare id),
  *number* (with unit and range unless the cell says otherwise), *toggle*, *JSON*, or
  *hidden* (reachable only through `security.protocol.params`).
* **Proposed** gives each option as `value` "Label": what it does. ~~Struck~~ options are
  listed but disabled, with the reason shown. "US" marks the value the deployed US system
  uses; a run with any value marked "not US" is flagged in its manifest
  (`Deployment::is_us`).
* **Level** is B for the Basic page and A for Advanced (`guide::level_of`).
* **Group** is the §3 group the setting appears under.
* Paths below `security.scms.` are shortened to start with `scms.`. Paths below
  `security.scms.deployment.providers[]` start with `prov[].`.

### 2.1 Deployment

| Path | Type | Today | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `security.protocol.id` | string (slot) | select | select. `protocol/scms/camp` "US SCMS": butterfly keys, two linkage authorities, linkage-value CRL (US). `protocol/etsi/ts102941` "European C-ITS PKI (TS 102 941)": authorisation tickets with no linkage and passive revocation; needs envelope `etsi103097` (not US). | absent (no credential lifecycle) | B |
| `actors.backend.protocol` | string (slot) | select | the same list and words, plus a warning when it disagrees with `security.protocol.id` | absent | A |
| `scms.deployment.preset` | string | **text** | select. `us-production-2025` "US system (2025)": two providers on one trust list, neutral labels a and b (US). `us-test-ctl-2025` "US test trust list (17 Jul 2025)": six roots, five approved; provider-f's root ships but is not trusted. `camp-reference` "CAMP reference design": one SCMS with LOP, CRL Store, RSU broadcast and a central MA, flat latency; the pre-2025 topology (not US). `custom` "Custom providers": you list the providers below. | `us-production-2025` on an SCMS run | B |
| `scms.deployment.electors` | u32 | number, no unit or range | select. `5` "5 electors" (US), or "Other…": a number from 1 to 32, step 1 (not US) | 5 | A |
| `scms.deployment.quorum` | u32 | number, no unit or range | select. `3` "Any 3 of the electors" (US), or "Other…": 1 to `electors` (not US) | 3 | A |
| `scms.deployment.options.lop` | bool | toggle | toggle "Location Obscurer Proxy". Off (US): requests go straight to the RA. On: one proxy hop with its own latency and queue. Neither 1609.2.1 nor the US profile has a LOP. | off; on under `camp-reference` | A |
| `scms.deployment.options.butterfly` | string | **text** | select. `original` "Original butterfly key expansion" (US): the device sends signing and encryption caterpillar keys, the RA expands them into cocoon keys, and the ACA encrypts each certificate to the device. ~~`unified`~~ "Unified butterfly": not deployed in the US; this build refuses it. ~~`cubk`~~ "Compact unified butterfly": the same reason. | `original` | A |
| `scms.chain_file` | string | **text** | select. `all-providers` "Every provider's CA certificates" (US global chain file). `home-provider` "Only my provider's": a device learns other providers' ICA and ACA over the air by P2PCD before it can verify their certificates (ASSUMPTION). | `all-providers` | A |
| `scms.deployment.latency` | string | **text** | select. `region` "Distance-based": great-circle distance at fibre speed plus a per-link overhead (ASSUMPTION). `flat` "Fixed latency": every backend link takes `net.backend_net`'s fixed latency. | `region`; `flat` under `camp-reference` or with `net.backend_net.params.latency_ms` set | A |
| `scms.deployment.fibre_fraction` | f64 | number, no unit or range | number in units of c, more than 0 up to 1, step 0.01. Presets: 0.667 "2/3 c, glass fibre" (default). Shown only under `region`. | 0.667 | A |
| `scms.deployment.per_link_ms` | f64 | number in ms, no range | number in ms, 0 to 100, step 0.5. Preset: 1 ms (ASSUMPTION). Shown only under `region`. | 1 | A |
| `scms.pki_view` | bool | toggle | toggle "Publish the PKI view" (pki.state, pki.flow and related channels). Off keeps long recordings small. | on | A |

### 2.2 Providers (a row editor per provider)

| Path | Type | Today | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `prov[].name` | string | **text** (row field) | open select of the preset's providers (`provider-a`, `provider-b`; `provider-a`…`provider-f` under the test CTL), plus "New provider…" under `custom` | — | A |
| `prov[].components` | map role → u32 | **hidden** (JSON only, nested map) | map editor (§4.2). The key is a role select: `root` Root CA, `ica` Intermediate CA, `eca` Enrolment CA, `ra` Registration Authority, `aca` Authorization (pseudonym) CA, `la1`/`la2` Linkage Authorities, `ma` Misbehaviour Authority, `crlg` CRL Generator, `pg` Policy Generator, and three marked "CAMP design only, not US": `lop`, `crl_store`, `crl_broadcast`. The count is a select from 0 to 8. For `root` and `ica`, ~~2–8~~ are disabled ("one per provider"). For `root`, `ica`, `ra` and `aca`, ~~0~~ is disabled ("every provider needs one"). | the preset's | A |
| `prov[].region` | string | **text** | select. `us-east` "us-east, Ashburn VA", `us-east-2` "Columbus OH", `us-central` "Council Bluffs IA", `us-south` "San Antonio TX", `us-west` "Boardman OR", `us-west-1` "Santa Clara CA", each labelled "assumed: no provider publishes its hosting". §5 replaces this with `site`. | `us-east` | A |
| `prov[].fleet_share` | f64 | number, no range | number as a fraction, 0 to 1, step 0.05, with a "Split equally" button. The form shows the running total, which must be 1 (ASSUMPTION: no market share is published). | equal shares | A |
| `prov[].on_ctl` | bool | toggle | toggle "Root on the trust list". Off models an untrusted or rogue root: nothing it certifies verifies. Disabled for the first (home) provider, with that reason. | on (`provider-f` off under the test CTL) | A |

### 2.3 Certificate lifecycle

Every row is a number with its unit and the range from `BOUNDS`. Durations get a
human-readable readout beside the number ("604,800 s = 1 week") and duration presets.

| Path | Type | Today | Proposed control (unit, min to max, step, presets) | Default | Lvl |
|---|---|---|---|---|---|
| `scms.lifecycle.i_period_s` | f64 | number | s, 1 to 31,536,000, step 1. Presets: 1 week (US, CAMP-EE §2.1.5.3.2); 1 day and 60 s (both need `time.calendar: compressed`). | 604,800 | A |
| `scms.lifecycle.cert_lifetime_s` | f64 | number | s, 1 to 31,536,000. Presets: "i-period + 1 h" (US 10,140 min); "same as i-period" (no overlap). | 608,400 | A |
| `scms.lifecycle.certs_per_period` | u32 | number | count, 1 to 60, step 1. Presets: 20 (US, CAMP-EE Table 2.1.2.6.2); 60 (NYC pilot taxis, CVP-LOG). | 20 | B |
| `scms.lifecycle.pool_periods` | u32 | number | weeks, 1 to 64. Presets: 2 (default); 1 (NYC pilot: one week ahead). | 2 | A |
| `scms.lifecycle.topup_below_periods` | u32 | number | weeks, 0 to 64. Presets: 1 (default); 0 "Never top up". | 1 | A |
| `scms.lifecycle.max_periods_ahead` | u32 | number | weeks, 1 to 520. Presets: 156 "3 years" (US, CAMP Primer p.7; THEA); 1 "1 week" (NYC pilot). | 156 | A |
| `scms.lifecycle.cert_shuffle_threshold` | u32 | number | requests, 1 to 10⁶. Presets: 1,000 (CAMP 1.2.2 Table 67); 10,000 (Brecht 2018). | 1,000 | A |
| `scms.lifecycle.cert_shuffle_window_s` | f64 | number | s, 0 to 604,800. Presets: 2 days (CAMP 1.2.2); 1 day (Brecht). | 172,800 | A |
| `scms.lifecycle.first_batch_delay_s` | f64 | number | s, 0 to 86,400. Preset: 60. | 60 | A |
| `scms.lifecycle.download_poll_interval_s` | f64 | number | s, 0.1 to 86,400. Preset: 300 "download-minWait" (SCMS Manager EE profile Table 3). | 300 | B |
| `scms.lifecycle.max_download_polls` | u32 | number | polls, 1 to 10,000. Preset: 100 "download-maxReqs" (same table). | 100 | A |
| `scms.lifecycle.download_request_tolerance_s` | f64 | number | s, 0 to 3,600. Preset: 5 (CAMP SCMS-1203). | 5 | A |
| `scms.lifecycle.enrolment_lifetime_s` | f64 | number | s, 1 to 315,360,000. Presets: 6 years (CAMP 1.2.2 Table 3); 7 years (R1.1); 3 years (5GAA draft). | 189,216,000 | A |
| `scms.lifecycle.reenrol_lead_s` | f64 | number | s, 0 to 315,360,000. Presets: 548 days (default, the middle of SM-REENR's 12–24 months); 12 months; 24 months. | 47,347,200 | A |
| `scms.lifecycle.reenrol_download_delay_s` | f64 | number | s, 0 to 86,400. Preset: 300. | 300 | A |
| `scms.lifecycle.ee_retry_timeout_s` (**new typed field**) | f64 | **hidden** | s, 0 to 3,600. Presets: 10 (CAMP SCMS-522); 0 "No retry". | 10 | A |
| `scms.lifecycle.ee_retry_max_per_hour` (**new typed field**) | u32 | **hidden** | retries, 0 to 1,000. Preset: 10 (CAMP SCMS-523). | 10 | A |
| `scms.tls` | string | select, no words | select. `1.3` "TLS 1.3" (US default). `1.2` "TLS 1.2": one more round trip before the first request. Both are allowed by the SCMS Manager EE profile §2.1.18. | `1.3` | A |
| `scms.lifecycle.clock_skew_s` (**new typed field**) | f64 | **hidden** | s, −3,600 to 3,600, step 0.1. Preset: 0. Shown with `clock_skew_share`. | 0 | A |
| `scms.lifecycle.clock_skew_share` (**new typed field**) | f64 | **hidden** | fraction, 0 to 1, step 0.05 | 0 | A |
| `scms.lifecycle.root_rollover_at_s`, `root_removal_at_s`, `peer_root_removal_at_s` (**new typed fields**) | f64 | **hidden** | s into the run, 0 to `time.duration_s`. Empty means never. | never | A |
| `security.pseudonym_change.strategy` | string | select | select (words already exist): `time`, `distance`, `j2945-1`, `nyc-pilot`, `c2c-cc`, `mix-zone`, `silent` | `time` | B |
| `security.pseudonym_change.period_s` | f64 | number | s, 1 to 86,400. Presets exist (300 J2945/1). Shown only under `time`. | 300 | B |
| `security.pseudonym_change.distance_m` | f64 | number | m, 1 to 100,000. Preset: 2,000 (NYC pilot). Shown only under `distance`. | — | A |
| `security.pseudonym_change.silent_period_s` | [f64; 2] | **JSON** | range pair: "from" and "to", in s, 0 to 60. Presets: "None"; "3–13 s" (PRESERVE). | none | A |
| `security.pseudonym_change.mix_zone_radius_m` | f64 | number, no bound | m, 5 to 500. Preset: 30. Shown only under `mix-zone`. | 30 | A |
| `security.pseudonym_change.mix_zone_min_age_s` | f64 | number, no bound | s, 0 to 3,600. Preset: 60. Shown only under `mix-zone`. | 60 | A |
| `security.signer_id_policy.full_cert_every_ms` | u64 | number in ms, no range | select. `1000` "Once a second" (J2945/1 reading). `450` "Every 450 ms" (NYC pilot reading). "Other…": 0 to 10,000 ms. | 1000 | A |
| `security.signer_id_policy.digest_otherwise` | bool | toggle | toggle "Send an 8-byte digest between full certificates" | on | A |
| `security.signer_id_policy.on_new_neighbour` | bool | toggle | toggle "Also attach the certificate after hearing a new station" (ETSI CAM rule, not US) | off | A |

### 2.4 Revocation

| Path | Type | Today | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `prov[].crl_publication` | string | **text** | select. `sla-1-business-day` "Within one business day" (US; SCMS Manager SLA). `cadence` "On the fixed cadence below". `on-decision` "As soon as decided". | `sla-1-business-day`; `cadence` under `camp-reference` | A |
| `scms.lifecycle.crl_cadence_s` | f64 | number (empty means the provider rule) | select. "Each provider's rule" (removes the key; default). `0` "Publish on decision". "Fixed cadence…": s, 1 to 2,592,000, presets 1 h, 1 day, 1 week. | provider rule | B |
| `scms.lifecycle.crl_fetch_interval_s` | f64 | number | s, 0.1 to 1,209,600. Presets: 1 h (ASSUMPTION inside CAMP's weekly bound); 1 week "the CAMP upper bound". | 3,600 | B |
| `scms.lifecycle.crl_broadcast_interval_s` | f64 | number | s, 0.1 to 3,600. Preset: 5 (USDOT RSU v4.1 Store and Repeat). Greyed out unless a unit has the `crl` role. | 5 | A |
| `scms.crl_overdue_tolerance_s` | u32 | number | s, 0 to 31,536,000. Presets: 2 weeks (CAMP 1.2.2 Table 67); 0 "Refuse as soon as overdue". | 1,209,600 | A |
| `scms.crl_overdue_scope` | string | select, no words | select. `craca` "Only certificates under the overdue list" (default). `all` "Every message" while any series-1 list is overdue. CAMP 1.2.2 Table 67 allows either reading. | `craca` | A |
| `scms.lifecycle.crl_background_entries` (**new typed field**) | u32 | **hidden** | count of earlier revocations already on the CRL, 0 to 10⁷. Presets: 0; 10,000; 1,000,000. | 0 | A |

### 2.5 Misbehaviour authority

| Path | Type | Today | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `prov[].ma_mode` | string | **text** | select. `manual-review` "An operator reviews each case" (CV-pilot "minimum viable" MA; CAMP122 §4.2–4.3). `automatic` "Acted on at once". | `manual-review` | B |
| `prov[].ma_review_s` | f64 | number, no range | s, more than 0 up to 2,592,000. Presets: 1 h; 8 h "one working shift" (ASSUMPTION); 1 business day. Shown only under `manual-review`. | 28,800 | A |
| `scms.lifecycle.ma_review_s` | f64 | number | select. `0` "Automatic". "Operator review…": the same presets. Applies to every provider without its own `ma_mode`. | 8 h on the real calendar; 0 on a compressed one | B |
| `scms.lifecycle.ma_max_open_cases` | u32 | number | cases, 1 to 10⁶. Preset: 1,000 (ASSUMPTION). | 1,000 | A |
| `scms.deployment.ma_report_sharing` | bool | toggle | toggle "Forward reports about another provider's certificates to that provider's MA" | on; off under `camp-reference` | A |
| `detection.ma.id` | string (slot) | select | select. `threat/ma/corroborated` "Independent evidence over time" (default). `threat/ma/legacy-window` "Legacy persistence gate" (kept for legacy-corpus parity). §7 supersedes this with `ma_policy`. | corroborated | A |
| `detection.ma.params` | JSON | **JSON** | typed number rows from `MA_CORROBORATED_PARAM_KEYS` (`phase2.rs:452`), each with its unit and bound: `event_window_s` 5, `event_min_reporters` 2, `min_events` 3, `window_s` 60, `min_reporters` 3, toggle `defence` on, `reputation_max` 40, `report_budget` 30. This piece is replaced by the §7 policy fields once those land. | card values | A |
| `scms.lifecycle.report_shuffle_threshold` | u32 | number | reports, 1 to 10⁶. Preset: 10,000 (CAMP-EE SCMS-765). | 10,000 | A |
| `scms.lifecycle.report_shuffle_window_s` | f64 | number | s, 0 to 604,800. Preset: 1 day (SCMS-765). | 86,400 | A |
| `scms.lifecycle.report_store_quota_bytes` | u32 | number | bytes, 0 to 10⁹. Presets: 0 "Keep none"; 16 KiB; 64 KiB (ASSUMPTION; MBR v1.0 §2.2 makes it device-specific); 1 MiB. | 65,536 | A |
| `scms.lifecycle.report_daily_multiplier` | f64 | number | factor, 0 to 1, step 0.05. Preset: 0.9 (MBR v1.0 §2.3.1). | 0.9 | A |
| `scms.lifecycle.report_decay_hour_et` | u32 | number | select of the 24 hours, "00:00 ET" to "23:00 ET"; 05:00 is "MBR v1.0 §2.2" | 5 | A |
| `scms.lifecycle.report_weight_{random_position, constant_position, max_speed, max_acceleration, brake_acceleration}` | f64 | number | weight, 0 to 1,000, step 0.1. Preset: 1.0 (MBR v1.0 §2.3.1.x). | 1.0 each | A |
| `scms.lifecycle.modelled_reports` (**new typed field**) | bool | **hidden** | toggle "Model report sizes instead of building real report PDUs" | off | A |
| `scms.report_records` | bool | toggle | toggle "Record every report's fate", for `v2xw ma-replay` and the §7 sweep | off | A |

### 2.6 Receiver checks

| Path | Type | Today | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `scms.rx_beacon_max_age_s` | f64 | number | s, 0 to 31,536,000, step 0.1. Preset: 2 s (C2C-CC RS_BSP_168). | 2 | A |
| `scms.rx_max_age_s` | f64 | number | s. Preset: 600 (RS_BSP_168). | 600 | A |
| `scms.rx_max_future_s` | f64 | number | s, step 0.01. Preset: 0.04 (RS_BSP_168). | 0.04 | A |
| `security.verification_policy` | string | select | select (words already exist): `verify-all`, `on-demand`, `prioritized` | `verify-all` | B |
| `security.on_demand_threshold` | f64 | number | score, more than 0 up to 1, step 0.05. Preset: 0.5. Shown only under `on-demand`. | 0.5 | A |
| `security.prioritized_horizon_m` | f64 | number | m, 10 to 2,000. Preset: 300. Shown only under `prioritized`. | 300 | A |

### 2.7 Cryptography

| Path | Type | Today | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `security.envelope` | string | select | select (words exist). `ieee1609.2` (US); `etsi103097` (not US). | `ieee1609.2` | A |
| `security.signature` | string | select; **8 of 13 values have no words** | select, with the label and `standard` taken from `v2xw_sec::scheme::SIGNATURE_SCHEMES` for every scheme. The `ml-dsa-*`, `falcon-*`, `slh-dsa-*` and `hybrid-mldsa65` values are marked not US. Under `crypto_mode: real`, every scheme except `ecdsa-p256` is disabled with the reason "real mode does the mathematics for P-256 only". | `ecdsa-p256` | B |
| `security.kem` | string | select, **no words** | select with labels and notes from `KEM_SCHEMES`. `ecies-p256` "ECIES on P-256" (US, IEEE 1609.2); `ml-kem-512/768/1024`; `hybrid-ecies-p256-ml-kem-768` (not US). | `ecies-p256` | A |
| `security.crypto_mode` | enum | select | select (words exist): `modeled`, `real` | `modeled` | A |
| `security.threshold` (whole block) | struct | (fields only) | toggle "Use authority committees" (not US). On adds the block with its defaults; off removes it. | absent | A |
| `security.threshold.protocol` | string | **text** | select from `THRESHOLD_PROTOCOLS` labels: `frost-p256` "FROST threshold Schnorr (RFC 9591)"; `cggmp21-ecdsa` "Threshold ECDSA (CGGMP21), 1609.2-compatible output"; `umbrella` "Umbrella credentials over derived leaves"; `shamir-feldman` "t-of-n secret release": for linkage seeds, so it is disabled for the PCA and CRL roles | `frost-p256` | A |
| `security.threshold.members` / `.quorum` | u16 | number in servers, **no range** | servers, 1 to 64 (`MAX_MEMBERS`); the quorum runs from 1 to `members`. Presets: 5 and 3. | 5 / 3 | A |
| `security.threshold.roles[]` | [string] | **JSON** | checkboxes: `pca` "Pseudonym CA"; `crl` "CRL Generator"; `linkage` "Linkage authorities". | all three | A |
| `security.threshold.linkage_members` / `linkage_quorum` | u16 | number, no range | the same ranges. Empty means the same as `members`/`quorum`. Shown only with `linkage`. | — | A |
| `security.threshold.umbrella_leaves` | u32 | number, no range | pseudonyms, 1 to 1,000. Preset: 20. Shown only under `umbrella`. | 20 | A |
| `security.threshold.umbrella_inner` | string | **text** | select of the signing protocols; ~~`umbrella`~~ is disabled ("cannot sign its own umbrellas"). Shown only under `umbrella`. | `frost-p256` | A |
| `security.threshold.preprocessed` | bool | toggle | toggle "Do the offline round ahead of time" | off | A |
| `security.threshold.round_timeout_ms` | f64 | number in ms, no range | ms, 1 to 60,000. Preset: 500. | 500 | A |
| `security.threshold.retry_after_s` | f64 | number in s, no range | s, 0 to 3,600. Preset: 30. | 30 | A |
| `security.threshold.ask_all` | bool | toggle | toggle "Ask every live member and keep the first quorum" | on | A |
| `security.threshold.member_latency_ms` | f64 | number in ms, no range | ms, 0 to 1,000. Preset: 20 (WAN between organisations, ASSUMPTION). | 20 | A |
| `security.protocol.params` | JSON | JSON | still shown as JSON, with the note "Legacy spelling: use the typed fields above; a typed field wins". `etsi_butterfly` becomes a toggle shown only under the ETSI protocol. | — | A |

### 2.8 Failures and servers

| Path | Type | Today | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `actors.backend.failures[].node` | string | **text** (row field) | composite picker (§4.2): **Provider** (the home provider, or any provider of the chosen preset or custom list) × **Server** (`ra`, `pca` "Pseudonym CA (ACA)", `la1`, `la2`, `ma`, `crlg`, `eca`, `dcm`, `lop`, `crl_store`, `crl_broadcast`, plus `<role>-member-<k>` for every committee member while `security.threshold` is set). It writes `provider-b/crlg`, or a bare role for the home provider. | — | A |
| `actors.backend.failures[].mode` | string | **text** | select. `offline` "Down": deliveries are dropped. `slow` "Slow": `extra_ms` more service time per message. `malicious` "Lying": invalid threshold shares; disabled unless the server is a committee member, with that reason. | — | A |
| `actors.backend.failures[].from_s` / `to_s` | f64 | number in s, **no range** | s, 0 to `time.duration_s`; `to_s` empty means "until the end" | 0 / end | A |
| `actors.backend.failures[].extra_ms` | f64 | number in ms, **no range** | ms, more than 0 up to 600,000. Presets: 10, 100, 1,000. Shown only under `slow`. | — | A |
| `actors.backend.entities` (map key) | role | **JSON** | map editor: the key is a role select from `SCMS_ENTITIES` | — | A |
| `actors.backend.entities.*.profile` | string (slot) | **hidden** (inside JSON) | select of the `backend/` and `bs/` hardware profiles (the slot already publishes this list) | `p.backend_profile` | A |
| `actors.backend.entities.*.service_model` | string (slot) | **hidden**; the slot is open and lists the whole registry | closed select. `service/mmc` "c parallel workers" (default). `service/mm1` "One worker". §6 moves the worker count to `capacity`. | `service/mmc` | A |
| `actors.backend.entities.*.net` | string (slot) | **hidden** | shown disabled with its status note: "not read; every entity sits on `net.backend_net`" | — | A |
| `actors.backend.links[].from` / `to` | string | **text** | role selects from `SCMS_ENTITIES` | — | A |
| `actors.backend.links[].latency_ms` / `capacity_mbps` | f64 | number | ms from 0, and Mbit/s above 0. Presets: 10 ms; 1,000 Mbit/s (engine defaults). | link default | A |

### 2.9 Access and relay (WP-19, in the running wave, not on `main` yet)

| Path | Type | Today (branch) | Proposed control | Default | Lvl |
|---|---|---|---|---|---|
| `net.rsu_ip.air` | string | select (it has a `CHOICES` row on the branch) | select. `constant` "Fixed air link": any relaying unit in range. `sch` "Service channel with WSA" (IEEE 1609.3): a vehicle uses a unit only after verifying its signed WSA. | `constant` | A |
| `net.rsu_ip.wsa_interval_ms` | u32 | number, 20–5,000 ms | ms. Preset: 100 (USDOT RSU Spec v4.1 App. D). Shown only under `sch`. | 100 | A |
| `actors.rsus[].roles[]` | [string] | checkboxes, open set | closed checkboxes with words: `provisioning-proxy` "Relays certificate downloads"; `report-forward` "Relays misbehaviour reports"; `crl` "Repeats the CRL"; `spat`, `map`, `spat-map`, `cpm` | — | A |

The fields that §5–§8 add are listed with their menus in those sections.

---

## 3. The SCMS panel: groups, order and help

The engine publishes `x-subgroup` with an id, a label and an order. It comes from one table
in `options.rs` (`SCMS_SUBGROUPS`, matched on the longest path prefix and then on the leaf
name). `subKeyOf` uses it when it is present, so the order lives with the engine, like
everything else in the form. Within a group, Basic fields come first and then the order of
§2.

| # | Group | Help shown under the heading |
|---|---|---|
| 1 | **Deployment** | Which certificate system runs and how it is laid out: the providers on the trust list, the electors who sign that list, and the parts of the design the US does not deploy. Leave it on "US system (2025)" for the real deployment. Any change the US does not make is marked *not US* and recorded in the run's manifest. |
| 2 | **Providers** | The companies that run an SCMS. Each runs its own root, intermediate CA, RA, pseudonym CA, linkage authorities and MA. Names are neutral labels. Where each one's servers sit, and its share of vehicles, are assumptions, because no provider publishes them. |
| 3 | **Certificate lifecycle** | How a vehicle gets and uses its certificates. It enrols once, receives 20 pseudonym certificates per week in batches from the RA, tops up before it runs out, and changes pseudonym while driving so it cannot be followed. Defaults are the US deployment's, with the source on each field. |
| 4 | **Revocation** | What happens after a device is judged misbehaving: how fast its provider publishes the revocation list (CRL), how often vehicles fetch it, and how a receiver treats a list that is overdue. |
| 5 | **Misbehaviour authority** | How reports of bad messages reach the MA and how the MA decides. Covers what a vehicle stores and uploads, how the RA shuffles reports, the MA's decision rule, and whether a person reviews each case. The US pilots used manual review. |
| 6 | **Receiver checks** | What a receiving vehicle accepts: how old or how far in the future a message may be, and which messages it verifies when it cannot verify all of them. |
| 7 | **Cryptography** | The signature and encryption schemes, and whether the mathematics is done (real) or only charged for (modelled). Committees, where t-of-n servers share one authority's key, are a research option and not the US system. |
| 8 | **Failures & servers** | Servers that go down, slow down or lie during the run, and each server's hardware, capacity and links. |

The `actors.backend.*` keys move from the "Backend & Cellular" section to group 8. In
`researchSectionOf` (`model.ts:349`) the SCMS rule already wins for `actors.backend`. Only
the sub-heading changes.

---

## 4. The changes the menus need

### 4.1 Engine (`options.rs`, `publish.rs`, `validate.rs`, `guide.rs`)

1. **One source per closed set.** Each set that today lives in a `match` (§1 table) becomes
   a `pub const …_IDS` next to its parser. The parser builds its "allowed:" message from
   that constant, and `extra_values` publishes the same constant. The loader and the form
   then cannot disagree. Sets that are only refused (`unified`, `cubk`) go into a sibling
   `Deployment::REFUSED_BUTTERFLIES` and are published with `available: false`.
2. **Words for every value.** Add `words()` rows for every set in §2. For signature, KEM and
   threshold schemes, take the label, `standard` and `notes` from the scheme registry in
   `option_row`, so a newly registered scheme gets words without an edit. Each option row
   gains `us: true | false`, which the UI shows as a "not US" badge.
3. **Bounds.** Add `BOUNDS` rows for every number marked "no range" in §2.
4. **Presets.** Add `PRESETS` rows for every number in §2 that lists presets, each with its
   source in `note`.
5. **Map keys.** For a map leaf (`…entities`, `prov[].components`), publish
   `x-key-options`: the role rows, with `available: false` and a reason where §2 disables one.
6. **Dependent sets.** For `failures[].node` and `prov[].name`, publish
   `x-options-compose: {"qualifier": "provider", "separator": "/"}` and a top-level
   `scms.providers_by_preset` map from preset to provider names. The UI fills the provider
   half from the document: the preset's names, or the custom list.
7. **Conditional relevance.** Publish `x-relevant-when: {path, in: [...]}` for every field
   §2 marks "shown only …". The loader already refuses most of these combinations. The UI
   greys such fields and says why, rather than hiding them, so nothing is ever invisible.
8. **Promote the params-only keys** (§1, last bullet) to typed `LifecycleSpec` fields. The
   rows go into `LifecycleSpec::entries`. A field left absent serialises to nothing, so
   scenario hashes do not move.
9. **Sub-groups** (§3): `x-subgroup` and `x-subgroup-blurb`.

### 4.2 UI (`ui/apps/studio/src/settings/*`, `lib/schema.ts`)

1. **Map editor** (`widgets.tsx`, `MapRows`): one row per key, with a key select from
   `x-key-options` and the value's own control. A map of structs (`entities`) gets a nested
   row of fields. It replaces the JSON fallback at `schema.ts:449` for maps whose values are
   leaves or one level of struct.
2. **Composite picker** (`ProviderRolePicker`): the provider select and the role select
   side by side. It reads `x-options-compose`.
3. **Range pair** (`RangePair`) for `[min, max]` lists that publish `x-pair: true`.
4. **Option explainer**: an "i" button beside every select. It opens a list of every
   option: label, sentence, US badge and, for a disabled option, the reason. This replaces
   reliance on the `<option title>` tooltip. Selecting a value still shows its sentence
   under the control.
5. **Relevance**: a field whose `x-relevant-when` is not met is drawn greyed, with
   "Applies only when *Failure mode* is *Slow*". Its value is kept.
6. **Duration readout**: next to any `s` field, show "= 1 week 1 h".
7. **US banner** at the top of the SCMS section: "This run is the US system", or "Not the US
   system: electors 7, 2 ICA levels". The banner reads the `is_us` result that
   `scenario.check` returns. Build piece A2 adds `is_us` to that result if it is missing.
8. **Sub-headings** from `x-subgroup` (`subKeyOf`, `model.ts:410`).
9. Every control carries `data-kind` (`select`, `text`, `number`, `toggle`, `json`, `map`,
   `pair`, `compose`) on its `data-testid="setting"` element. This is for the check in §10.

---

## 5. Gap 1: exact server locations

**Today.** `Provider.region: Region`
(`crates/v2xw-proto/src/scms/deployment.rs:298–322`) is one of six catalogue points,
`REGIONS` (`deployment.rs:190–229`). Every one is marked ASSUMPTION. All of a provider's
roles share that point, and `intra_region_latency` is `over_km(0.0)`, which leaves only the
per-link overhead (`deployment.rs:754`). The city leg is `city_latency`
(`deployment.rs:771`).

**What can be known.** No SCMS provider publishes where its servers run
(01-deployment-and-governance §2.3: "UNCONFIRMED for every provider"). That stays true, so
*which* site a provider uses remains an assumption. What can be cited is *where each public
cloud region is*:

| Cloud | Published location granularity | Source |
|---|---|---|
| Google Cloud | city: us-central1 Council Bluffs IA; us-east1 Moncks Corner SC; us-east4 Ashburn VA; us-east5 Columbus OH; us-south1 Dallas TX; us-west1 The Dalles OR; us-west2 Los Angeles CA; us-west3 Salt Lake City UT; us-west4 Las Vegas NV | https://docs.cloud.google.com/compute/docs/regions-zones (read 2026-10-28) |
| Azure | state or city: eastus and eastus2 Virginia; centralus Iowa; northcentralus Illinois; southcentralus Texas; westcentralus Wyoming; westus California; westus2 Washington; westus3 Phoenix | https://learn.microsoft.com/en-us/azure/reliability/regions-list (read 2026-10-28) |
| AWS | state only: us-east-1 N. Virginia; us-east-2 Ohio; us-west-1 N. California; us-west-2 Oregon | https://docs.aws.amazon.com/global-infrastructure/latest/regions/aws-regions.html (read 2026-10-28) |

**Design.**

* A `SITES` catalogue in `deployment.rs`. Each entry has an id, a label, a cloud, the
  published location string, its source URL, and a point with `point_source`:
  * `published-city`: the US Census Gazetteer place's internal point (`INTPTLAT` and
    `INTPTLONG`, 2024 Gazetteer places file) for the city the cloud names.
  * `assumed-metro`: for a state-only listing, a named metro marked ASSUMPTION. For example,
    AWS us-east-1 → Ashburn VA, the same point as today's `us-east`.

  The six existing `REGIONS` stay as catalogue entries with their present coordinates, so
  every existing scenario is byte-identical.
* **Error bound, stated in the manifest.** A city point is within about 30 km of any campus
  in that city. At 2/3 c that is 0.15 ms one way, well under the 1 ms per-link overhead.
* New keys:
  * `prov[].site`: a site id. It cannot be set together with `region`.
  * `prov[].sites.<role>`: a per-role site override, for example the MA in one region and the
    RA in another.
  * `prov[].lat_deg` and `lon_deg`: custom coordinates for an on-premises site. These
    require `site: custom` and are labelled "your assumption".
* Latency between two roles of one provider becomes `over_km(great_circle(site_a, site_b))`.
  When both sites are the same this is still `over_km(0)`. The manifest's `assumptions()`
  lists each provider's sites and their `point_source`.

**Settings and menus.**

* `prov[].site`: a select grouped by cloud, labelled for example "Google us-east4: Ashburn,
  VA (published city)". It also offers "Custom coordinates…". The "Engine default" option
  means "the preset's region".
* `prov[].sites`: a map editor whose key is a role select and whose value is the site select.
* `lat_deg`: degrees, −90 to 90, step 0.0001. `lon_deg`: degrees, −180 to 180. Both are
  relevant only with `site: custom`.

**Accept tests**, in `crates/v2xw-engine/tests/scms_sites.rs`. Each test names the fault
that must turn it red:

1. Provider-b on `gcp-us-west1`: `inter_provider_latency(0, 1)` equals
   `great_circle(Ashburn, The Dalles) / (2/3 c) + 1 ms` within 1 µs. *Fault: the parser reads
   `site` but leaves `region` in place.* The latency stays at the us-east value and the test
   goes red.
2. `sites: {ma: us-west}` on provider-a: the RA→MA report upload in `pki.flow` is later than
   in the base run by the computed leg, ±1 µs. *Fault: intra-provider links still use
   `over_km(0)`.*
3. `site` together with `region`, an unknown site, or `lat_deg` without `site: custom`: each
   is refused with a path-precise message. *Fault: drop the exclusivity check.*
4. The manifest names every site's `point_source`. *Fault: omit the sites from
   `assumptions()`.*

**Goldens.** Every new key is off by default. All shipped scenarios must keep their digests.
The `scms_deployment` goldens are re-run, with no re-bless.

**Pieces (about 3 h each).**

* C1a: the catalogue, keys, parser, latency and tests 1, 3 and 4.
* C1b: per-role sites, test 2, guide and option words.

---

## 6. Gap 2: server capacity as settings

**Today.** Every backend entity is a `ServiceModelSpec { servers, fixed_overhead, batching }`
(`crates/v2xw-proto/src/service.rs:49`). Defaults are `backend_servers: 4` and
`backend_overhead: 1 ms` (`crates/v2xw-proto/src/scms/params.rs:594–595`), unsourced. The
only per-entity control is `service_model` `mmc`/`mm1`, which means 4 servers or 1
(`phase2.rs:9139`). There is no queue limit, so an overloaded server queues forever. The
other way to slow a server is the `slow` failure mode. Peer providers' entities cannot be
configured at all.

**What can be known.** No SCMS provider or CAMP document reviewed in 03 or constants.md
publishes worker counts, queue limits or per-request service times for production servers.
Every capacity default is therefore an ASSUMPTION and is labelled so. The cryptographic part
of the service time stays sourced: it comes from the hardware profile (`backend/…`) through
`profile` and the operation-cost tables. What a device does when its request is turned away
is sourced too: the retry rules of CAMP SCMS-522/523, already implemented as
`ee_retry_timeout`/`ee_retry_max_per_hour`.

**Design.**

New keys:

* `security.scms.deployment.capacity.<role>`, which applies to every provider.
* `prov[].capacity.<role>`, which overrides it for one provider.

Each role takes:

| Field | Meaning | Default (ASSUMPTION, today's values) |
|---|---|---|
| `workers` | parallel servers, the c of M/M/c | 4 |
| `queue_limit` | requests waiting beyond the workers; above it a request is refused with "busy". Empty means unlimited. | unlimited |
| `overhead_ms` | per-request time beyond the cryptography | 1 |
| `service` | `mmc` (workers in parallel) or `fifo-1` (`mm1`) | `mmc` |

* `ServiceQueue` gains an optional `limit`. A refused request is a protocol-visible
  failure, so the device's existing retry path runs. It is counted in `backend.state` as
  `refused_busy`.
* `actors.backend.entities.*.service_model` keeps working. Setting both it and
  `capacity.<role>.service` for the same role is refused, naming both paths.
* `nodes.backend_tier: abstract` still overrides everything with 64 workers and no overhead
  (`phase2.rs:9107`). The form greys the capacity fields under it and says so.

**Settings and menus.**

* `workers`: workers, 1 to 1,024, step 1. Presets: 1; 4 (engine default); 16; 64 ("the
  abstract tier").
* `queue_limit`: a select. "Unlimited" (default), or a number from 1 to 10⁷ with presets of
  100, 1,000 and 10,000.
* `overhead_ms`: ms, 0 to 10,000, step 0.1. Presets: 0; 1 (default); 5; 20.
* `service`: a select. `mmc` "Workers in parallel"; `fifo-1` "One at a time".
* The role key: the same role select as `components`.

**Accept tests**, in `tests/scms_capacity.rs`:

1. The RA with `workers: 1` against `workers: 4`, at the same arrival burst: the mean RA
   sojourn in `backend.state` matches the Erlang-C expectation for the measured arrivals,
   within 5 %. *Fault: `workers` is parsed and ignored.*
2. `queue_limit: 10` under a 500-request burst: `refused_busy > 0`. Every refused device
   retries after `ee_retry_timeout_s`, and none exceeds `ee_retry_max_per_hour`. *Fault:
   the limit is checked after enqueueing, so nothing is refused.*
3. Provider-b's `capacity.crlg.overhead_ms: 5000` delays only provider-b's CRL publication.
   *Fault: the override is applied to the home provider.*
4. `entities.ra.service_model` together with `capacity.ra.service` is refused.
5. All defaults absent: shipped digests are unchanged.

**Pieces (about 3 h each).**

* C2a: keys, `ServiceQueue` limit, refusal and retry path, tests 2 and 4.
* C2b: per-provider wiring for peer entities, tests 1, 3 and 5, and the form words.

---

## 7. Gap 3: a declarative, pluggable MA policy and a family sweep

**Today.**

* Two decision pipelines exist, chosen by `detection.ma`: `threat/ma/corroborated` (the
  default; `crates/v2xw-threat/src/ma_corroborate.rs:83`, parameters at `:87–123`) and
  `threat/ma/legacy-window` (`ma.rs`). Their parameters are free JSON in
  `detection.ma.params`.
* Per provider, `ma_mode` chooses manual review or automatic (`params.rs:378`).
* An offline rule family already exists: `v2xw ma-replay`
  (`crates/v2xw-cli/src/ma_replay.rs`). Its rules are `pseudonym-k`, `device-k`, `catch` and
  `cluster`, run over a run's recorded `ma.received` rows, and scored against ground truth
  only after every rule has decided.
* The hard limit today: nothing outside code can define a new rule, and the in-run MA cannot
  be swapped for one of the replay rules.

**What the sources say.** The US deployment publishes the report format and the device-side
store weights (SM-MBR v1.0), not the MA's decision rule. The pilots ran a manual "minimum
viable" MA: "an operator reviews reports and adds devices to the CRL" (CAMP122 §4.2–4.3;
PRIMER pp. 6, 8; 03 §8). TS 103 759 and SAE J3287 leave the reaction out of scope. So **no
single policy can be called the deployed one**, and the owner's paper is right to present a
family. The default preset reproduces today's engine default exactly. It is labelled for
what it is: an operator-reviewed MA whose evidence rule is an ASSUMPTION.

**Design: `security.scms.ma_policy`** (in a new module, `crates/v2xw-threat/src/ma_policy.rs`)

```yaml
security:
  scms:
    ma_policy:
      preset: cv-pilot-manual        # or a name from ma_policies, or custom
      identity: pseudonym            # pseudonym | device (by the LAs' link token)
      evidence:
        rules:                       # per report subtype: does it count, and its score
          RandomPosition:   {score: 1.0}
          ConstantPosition: {score: 1.0}
          MaxSpeed:         {score: 1.0}
          MaxAcceleration:  {score: 1.0}
          BrakeAcceleration:{score: 1.0}
        accuracy_weighting: none     # none | catch (tau_m)
        event_window_s: 5
        event_min_reporters: 2
      threshold:
        min_events: 3
        min_score: 0                 # 0 = not used
        min_reports: 0
        min_distinct_reporters: 3
        window_s: 60
      decay: none                    # none | daily-multiplier (factor, hour_et) | half-life (half_life_s)
      population_guard: off          # off | cluster (c, m)
      defence: {trusted_reporters: on, reputation_max: 40, report_budget: 30}
      decision: {mode: manual-review, review_s: 28800}   # or {mode: automatic}
    ma_policies:                     # named family members, for providers and sweeps
      strict-k5: {preset: pseudonym-k, threshold: {min_distinct_reporters: 5}}
    deployment:
      providers:
        - {name: provider-b, ma_policy: strict-k5}
```

* **Presets.** A preset fills every field. A field written beside the preset overrides it.

  | Preset id | Label | What it is |
  |---|---|---|
  | `cv-pilot-manual` | CV-pilot operator review | Default. Exactly `CorroborationParams::default()`, plus manual review for 8 h. |
  | `corroborated-auto` | Corroborated, automatic | The same rule with no operator. |
  | `legacy-window` | Legacy persistence gate | Parity with the legacy corpus. |
  | `pseudonym-k` | k reporters per certificate | Unsound at k = 1. |
  | `device-k` | k reporters per device over a window | |
  | `catch` | Accuracy-aware (CaTch-style) | |
  | `cluster` | Population guard | |
  | `custom` | Custom | Every field yours. |

  The last five come from `ma_replay.rs`, with the same parameters and words.
* **One evaluator.** `MaPolicy` implements the existing `MaPipeline` trait
  (`v2xw_threat::ma`). The in-run MA (`phase2::MaHost` gains a `Policy` arm) and
  `v2xw ma-replay` both call the same `MaPolicy::decide`. Online and offline cannot drift
  apart.
* **Conflicts.** `detection.ma` together with `ma_policy` is refused. So are
  `prov[].ma_mode` together with that provider's `ma_policy.decision`, and a provider's
  `ma_policy` naming nothing in `ma_policies`.
* **Python hook (optional, piece B4).** v2xw-py already turns a Python object into a Rust
  trait object (`crates/v2xw-py/src/plugins.rs`), with a batched family for detectors and a
  determinism kit (`conformance`). A `PyMaPolicy` implements `MaPipeline` in the same way:
  * It receives one Arrow batch of the `MaView` columns per MA evaluation tick. Only ticks
    with new reports produce a batch.
  * It returns `(subject, revoke)` rows.
  * It works only for runs started from Python (`v2xw.run(..., ma_policy=obj)`), because a
    scenario file cannot name Python code.
  * The manifest records `ma_policy: python:<module>:<sha256 of source>`.
  * It obeys the plug-in rules (no own RNG, wall clock or platform libm). The two-run digest
    check applies.
* **Sweep mode.** `experiment.sweep` already expands any dotted path
  (`schema.rs` `Experiment`, KEY_STATUS `experiment` wired). Sweeping
  `security.scms.ma_policy.preset` over a list of names (presets or `ma_policies` keys) runs
  one full run per policy per seed. That is the paper-grade answer, because it captures
  feedback: a revoked device stops sending, CRL delays apply and reporters' reputations
  change.
  * A new experiment report, `ma-policy-family.csv`, plus a Markdown table, has one row per
    (policy, seed) and a mean with a 95 % CI per policy. Its columns:
    * attackers revoked;
    * **honest devices revoked**, and honest devices blocked per 10⁶ device-hours;
    * time to revoke, as median and p90, in three stages: attack start → MA decision → CRL
      published → first receiver refusing;
    * reports processed and peak open cases.

    Ground truth (`gt.scms_device`) is joined only in this report, never in the policy.
  * A fast preview: `v2xw ma-replay --policies <scenario.yaml>` runs the same named family
    over one recorded run.
  * The report states the difference: an offline replay cannot show feedback.
  * Both modes require `report_records: true`. The sweep sets it for every point and says
    so in the report header.

**Settings and menus.**

* `ma_policy.preset`: a select of the eight presets, with the words above, plus the names in
  `ma_policies`.
* `identity`: a select. `pseudonym` "Per certificate"; `device` "Per device, through the
  linkage authorities".
* `evidence.rules`: a map editor. The key is a subtype select (the SM-MBR subtypes and the
  `detect/*` check ids). The value is `score`: 0 to 100, step 0.1, and 0 means "ignored".
* `accuracy_weighting`: a select. `none`; `catch` "Discount what the accused's own
  accuracy explains". The `tau_m` field is in m, 0.1 to 10, preset 1 (SM-MBR gpsDrift).
* `event_window_s`: s, 0.1 to 600, preset 5 (F2MD `DELTA_REPORT_TIME`).
* `event_min_reporters`: 1 to 100, preset 2.
* `min_events`: 1 to 100, preset 3.
* `min_score` and `min_reports`: 0 to 10⁶.
* `min_distinct_reporters`: 1 to 1,000. Preset 3 (the legacy k). Values below 2 are marked
  "a single report can revoke (unsound)".
* `window_s`: s, 1 to 2,592,000. Presets: 60, 1 h, 1 day, 1 week.
* `decay`: a select. `none`. `daily-multiplier`: factor 0 to 1, preset 0.9, and an hour
  select (MBR v1.0). `half-life`: `half_life_s` with presets of 1 h and 1 day.
* `population_guard`: a select. `off`; `cluster`, with `c` from 2 to 1,000 and `m` from 1 to
  10.
* `defence`: a toggle and two numbers, with the existing defaults.
* `decision.mode`: a select. `manual-review`, with `review_s` presets as in §2.5; or
  `automatic`.
* `prov[].ma_policy`: an open select of the preset ids and the `ma_policies` names.
* The sweep: in the Experiment panel, "Compare MA policies" offers a checkbox list of the
  same names and writes `experiment.sweep."security.scms.ma_policy.preset"`.

**Accept tests**, in `tests/ma_policy.rs`:

1. **Default identity.** `ma_policy: {preset: cv-pilot-manual}` produces the same event-log
   digest as no `ma_policy` on `credential-lifecycle` and `revocation-latency`. *Fault: the
   preset's `event_min_reporters` is 3.* The digest differs and the test goes red.
2. **Online and offline agree before the first revocation.** Until the first revocation
   decision, the online MA and `ma-replay` see identical `ma.received` rows. So for each of
   the eight presets, the time of the first decision and its subject match between the
   online run and the replay of that run's records. *Fault: the replay's window is
   right-open while the online one is closed.*
3. **Per-provider policy.** Provider-b on `pseudonym-k`, k = 1, and provider-a on the
   default: only provider-b's certificates are revoked on a single report. *Fault: the
   provider's policy falls back to the deployment-wide one.*
4. **Blindness.** A `custom` policy passes `check_blind` (`ma_replay.rs`): renaming every
   identifier renames its decisions in the same way. *Fault: a rule reads `gt.*`.*
5. **Sweep report.** A sweep over three presets × 2 seeds writes 6 rows plus 3 summaries. An
   honest-only scenario reports 0 attackers revoked, and every revoked device is counted as
   honest. *Fault: the honest join uses the pseudonym rather than the device.* The count
   drops to 0 and the test goes red.
6. **Refusals.** `detection.ma` together with `ma_policy`; an unknown `ma_policies` name;
   `min_distinct_reporters: 0`.
7. **Python (B4).** A Python port of `pseudonym-k` produces the same decisions as the Rust
   preset on the recorded run. *Fault: the adapter drops the last row of each batch.*

**Goldens.** `ma_policy` is absent by default and the `MaHost` default path is untouched, so
every digest must stay the same.

**Pieces (about 3 h each).**

* B1: `MaPolicy` in v2xw-threat with all presets, test 1 at the unit level, and
  `cv-pilot-manual` equal to `CorroboratedMa`.
* B2: engine wiring through the `MaHost::Policy` arm, `ma_policies`, per-provider policy,
  the decision mode, and tests 1, 3 and 6.
* B3: the sweep report in v2xw-experiment, `ma-replay --policies`, and tests 2, 4 and 5.
* B4 (optional): `PyMaPolicy` and test 7.

---

## 8. Gap 4: deeper CA hierarchies (several ICA levels), marked not US

**Today.**

* Each provider has exactly one root and one ICA. `validate` refuses `count("ica") > 1`
  (`deployment.rs:592`).
* The issuing chain is `root → ica → {eca, ra, aca, la, …}`, with some components
  root-issued (`ROLE_SPECS`, `crates/v2xw-proto/src/scms/governance.rs:294`).
* The ICA's issuing permissions are `{minChainLength 2, chainLengthRange 0}` and the root's
  are `{3, −1}` (`cert_issue_permissions`, `governance.rs:777–810`).
* The receiver accepts at most `MAX_CHAIN_LENGTH = 8` certificates
  (`crates/v2xw-sec/src/chain.rs:58`).

**Design.**

* New key `prov[].ica_levels` (1 to 5; 1 is the US). It is independent of
  `components.ica`, which still counts instances and stays at 1.
* Level 1 is the root-issued ICA and keeps the name `ica`, so today's names and bytes are
  unchanged. Levels 2…n are `ica-2`…`ica-n`. Each is issued by the level above, and level n
  issues everything the ICA issues today. Root-issued components such as `pg` stay
  root-issued, as in the published chains.
* **Permissions.** Level k carries `{minChainLength 2 + (n − k), chainLengthRange 0}`, so
  every chain is exact and a certificate issued by the wrong level fails the chain check.
  The root's `{3, −1}` already admits any depth.
* **Depth limit.** Root + n ICAs + ACA + pseudonym ≤ 8, so n ≤ 5. n = 6 is refused, naming
  the limit.
* **Behaviour.**
  * Every level is offline (`is_offline`).
  * The chain file holds every level.
  * Under `chain_file: home-provider`, P2PCD learns each missing level, one exchange per
    certificate.
  * A receiver's first verification of a foreign chain pays one extra verification per level
    until the validated-certificate cache holds it.
  * A revoked ICA level is revoked by its issuer (CRACA = the issuer, series
    `SCMS_COMPONENTS`).
* **Manifest.** `Deployment::is_us` returns false when any provider has `ica_levels > 1`,
  and the manifest says "not the US system: provider-a has 3 ICA levels".
* `failures[].node` and `prov[].sites` accept `ica-2`…`ica-n` (they are offline, so
  `offline` is the only failure mode with any effect).

**Settings and menus.**

* `prov[].ica_levels`: a select. `1` "One intermediate CA" (US). `2`, `3`, `4`, `5` "N
  intermediate CAs" (not US). ~~`6`+~~ disabled: "a receiver accepts at most 8 certificates
  in a chain (IEEE 1609.2)".
* The role selects (§2.2, §2.8) list `ica-2`… once a level exists.

**Accept tests**, in `tests/scms_hierarchy.rs`:

1. With `ica_levels: 3`, a pseudonym certificate's chain is 6 long. `v2xw_sec::chain`
   validates it with zero chain errors, and the run has zero chain-related verification
   failures. *Fault: every level carries `{2, 0}`.* The chain check refuses, so failures
   are above 0 and the test goes red.
2. `is_us` is false and the manifest names the level count. *Fault: `is_us` ignores
   `ica_levels`.*
3. `ica_levels: 6` is refused with the limit in the message.
4. Under `home-provider`, a device's first verification of provider-b's certificate makes
   n + 1 P2PCD exchanges, for the ICA levels and the ACA. *Fault: the chain file omits the
   middle level.* Verification fails and the test goes red.
5. `ica_levels: 1`, or the key absent, keeps every certificate byte and digest the same as
   `main`.

**Pieces (about 3 h each).**

* C3a: governance issuance at depth n, the permissions, tests 1, 3 and 5.
* C3b: the chain file, P2PCD, `is_us`, manifest, failure-node names, tests 2 and 4.

---

## 9. Choices made under "most realistic" (the owner delegated these)

1. The US values stay the defaults everywhere. Every non-US value is selectable but
   badged. Refused values are shown disabled with the reason, never hidden.
2. Provider hosting stays an ASSUMPTION. The site catalogue cites where clouds are, never
   which cloud a provider uses. City points come from the Census Gazetteer. AWS's
   state-only regions get a named metro marked ASSUMPTION.
3. Capacity defaults stay today's 4 workers, 1 ms and an unlimited queue, labelled
   ASSUMPTION. No figure is invented.
4. The MA default is `cv-pilot-manual`, which equals today's behaviour. The paper's
   family runs online (feedback included), with `ma-replay` as a preview only.
5. Python MA policies only from Python-launched runs. A scenario file never names code.
6. Deeper hierarchies are capped at 5 ICA levels by the receiver's chain limit, not by a
   number chosen here.

---

## 10. The check that keeps every SCMS setting on a menu

**Unit test**, `every_scms_string_setting_offers_a_menu`, in the `publish.rs` tests (same
module as `slots_report_what_they_can_offer`):

* For every published leaf whose path starts with `security.`, `actors.backend.`,
  `detection.ma` or `net.rsu_ip.`:
  * A `string` leaf, a list of strings or a map key must publish a non-empty `x-options` (or
    `x-key-options`).
  * Every row must have a non-empty `label` and `description`.
  * Every `available: false` row must have a description, which is its reason.
* Every `number`/`integer` leaf under the same paths must publish a `unit` and an `x-range`.
* There is **no allow-list**. Free names such as `prov[].name` publish open options (the
  preset names), so they pass on their own terms. An allow-list is the usual way a check
  quietly stops being able to fail.
* **Injected faults that must turn it red.** The PR records each run.
  1. Delete the `options.butterfly` arm from `extra_values`. The test fails on
     `security.scms.deployment.options.butterfly`.
  2. Remove the KEM label lookup. The test fails on `security.kem` / `ml-kem-512` with
     "no description".
  3. Remove the `BOUNDS` row for `deployment.electors`. The test fails with "no range".

**Playwright**, in `ui/apps/studio/e2e-engine/scms-settings.spec.ts`. It runs against the
real engine's published schema, in the `playwright.engine.config.ts` project:

* It opens Settings → Certificates & SCMS → Advanced and visits all eight groups.
* It asserts that no `[data-testid=setting][data-pointer^="/security"]`,
  `[data-pointer^="/actors/backend"]` or `[data-pointer^="/detection/ma"]` row has
  `data-kind="text"` or `data-kind="json"`, except `/security/protocol/params`, which is
  asserted to carry the "legacy spelling" note.
* **A built-in twin proves it can fail.** The same assertion runs a second time with
  `page.route` deleting `x-options` from the butterfly row of the schema response. That
  twin is marked `test.fail()`. If the check stops being able to fail, the twin passes and
  the suite goes red.

---

## 11. Build schedule: three agents, no hot file edited in parallel

**Pre-condition.** The two waves running now edit `options.rs`, `guide.rs`, `schema.rs`,
`validate.rs` and `backend.rs` (worktrees `wf_6212b431-de5-1…3`, WP-19 and others). This
work starts after they merge, from the `main` they leave.

Each agent runs one heavy command at a time through `heavy.sh`. Each agent uses its own
worktree and the shared target directory (see the lean-agent and parallel-Rust notes).

**File ownership.**

| Agent | Owns (only it edits these) |
|---|---|
| **A: menus** | `scenario/schema.rs`, `scenario/validate.rs`, `scenario/options.rs`, `scenario/guide.rs`, `scenario/publish.rs`, every touch to `phase2.rs`, and `ui/apps/studio/**` |
| **B: MA policy** | `crates/v2xw-threat/src/ma_policy.rs` (new), `ma.rs`, `ma_corroborate.rs`, `crates/v2xw-engine/src/ma_policy.rs` (new), `crates/v2xw-cli/src/ma_replay.rs`, `crates/v2xw-experiment/**`, `crates/v2xw-py/src/plugins_ma.rs` (new) |
| **C: topology** | `crates/v2xw-proto/src/scms/deployment.rs`, `scms/governance.rs`, `service.rs`, `kernel.rs`, `crates/v2xw-engine/src/scms_topology.rs` (new) |

**How hot files stay single-owner.** Piece **A0** goes first, alone (about 3 h):

* It adds every new schema struct and key from §5–§8.
* It adds stub modules `ma_policy.rs` and `scms_topology.rs`, holding the `…_IDS` constants
  and a `KEY_STATUS` slice. `validate::status_of` falls back to those slices.
* It adds one call site each in `phase2.rs`: `MaHost::Policy`, `scms_topology::apply_sites`,
  `apply_capacity` and `ica_levels`.
* Each stub refuses any non-default value with "not built yet", so nothing half-works.

**Status, A0 (2026-10-29): built.** Every key of §5-§8 is in `schema.rs` (`MaPolicySpec` and
its seven blocks, `CapacitySpec`, and `site`, `sites`, `lat_deg`, `lon_deg`, `capacity`,
`ma_policy`, `ica_levels` on a provider), all absent by default, so every scenario hash is
unchanged. The engine modules `ma_policy.rs` and `scms_topology.rs` hold each key's
`_IDS`, `KEY_STATUS` (`refused`), range, title, unit, step and menu (`Opt` rows with
`available: false` and the reason for every unbuilt value), read through
`scenario/module_keys.rs` by `validate::status_of`/`bound_of`, `options::title`/`extra_values`
and `publish` (`unit`, new `x-step`). B and C edit only those two files' tables. The loader
and the run refuse every non-default value with "not built yet" (the run through
`MaHost::from_scenario`, `deployment_of` and the `apply_capacity` call after the peer runs);
only `ma_policy.preset: cv-pilot-manual` and `ica_levels: 1` load. Choices made here (§9 rule):

* `decay` and `population_guard` are blocks with a `mode` select (`{mode: daily-multiplier,
  factor, hour_et}`) rather than the scalar shorthand of the §7 sketch, so each field is its
  own control with its own range.
* `decay.half_life_s` 1 s to 1 year, `defence.reputation_max`/`report_budget` 1 to 10⁶ and
  `decision.review_s` (0, 30 days]: §7 gives no range; these span every cited horizon.
  `review_s` keeps `lifecycle.ma_review_s`'s 30-day top but excludes 0, because
  `decision.mode: automatic` is the no-operator case.
* Map keys are closed: `evidence.rules` takes the five SM-MBR subtypes; `sites` and
  `capacity` take `deployment::ROLES`. The `detect/*` rule keys and `ica-2…` roles come with
  B1 and C3b. `ma_policy` together with `detection.ma` is refused now (§7 Conflicts).
* Left for A1-A3: `x-key-options` for the map keys, presets (`x-presets`), relevance, guide
  rows, and the UI reading `x-step`.

Checks (all shown red under a named fault, then reverted): `ma_policy::tests` and
`scms_topology::tests` (each non-default value refused at its path, the run refusing
without the loader, malformed values named, menus equal to their `_IDS`) and
`publish::tests::module_keys_publish_menus_ranges_and_status` (62 leaves, each with a
title, a menu or unit/range/step, and status `refused`). The `phase2` and `bsm_timing`
goldens are unchanged.

**Status, A1 (2026-10-29): built.** Engine menus, §4.1 items 1-5, 8 and 9, and the §10 unit
test. Every SCMS string leaf, list of strings and map key now publishes a menu with a label and
a sentence per value; every SCMS number a unit and a range.

* Closed sets (item 1): `phase2::{CHAIN_FILE_IDS, MA_MODE_IDS, LATENCY_IDS, BUTTERFLY_IDS,
  REFUSED_BUTTERFLIES, SERVICE_MODEL_IDS}` and `crypto_config::FAILURE_ROLES` feed both the
  parsers' "allowed:" messages and `options::extra_values`; preset, region and CRL rule read
  the existing `deployment.rs` constants. The refused butterflies sit in `phase2`, not
  `Deployment::REFUSED_BUTTERFLIES`, because `deployment.rs` is agent C's file; a test ties
  them to `Deployment::butterfly`. `entities.*.service_model` is closed to the two queues the
  run builds (it listed the whole registry). `failures[].node` and `prov[].name` are open menus.
* Words (item 2): rows for every §2 set; signature, KEM and committee words come from the
  scheme registries (`option_row` → `scheme_words`), so a new scheme needs no edit. Rows carry
  `us: true|false` (`options::us_of`). Map keys (item 5): `x-key-options` for
  `entities`, `prov[].components`, `prov[].sites`, `capacity` (both), `ma_policies`
  (open) and `ma_policy.evidence.rules`.
* Bounds (item 3): 31 `BOUNDS` rows, each enforced by the loader (`validate::scms_menu_bounds`,
  or `scms_lifecycle` for the lifecycle keys), so the form's range is the loader's. Choices made
  under §9: `full_cert_every_ms` tops at 3,600,000 ms, not §2's 10,000, because
  `radio-validation.yaml` uses one hour for "digests only"; `clock_skew_s` is 0 to 3,600 s, not
  ±3,600, because the lifecycle loader refuses a negative key; `crl_background_entries` tops at
  10⁶, the clamp the model applies. The SCMS numbers that were "dimensionless" now name a unit.
* Presets (item 4): `options::SCMS_PRESETS`, 90 rows with their sources; a test keeps each
  inside its range. `report_decay_hour_et` is a 24-hour menu.
* Typed keys (item 8): nine of the ten params-only keys are `LifecycleSpec` fields with
  defaults, guide rows and reference lines; absent they serialise to nothing, so no scenario
  hash moves. `etsi_butterfly` stays in `security.protocol.params`: `security.scms` is refused
  on the ETSI protocol, so a typed field there could never apply. `crl_background_entries` now
  reads the typed field too (`lifecycle_param`).
* Sub-groups (item 9): `options::SCMS_SUBGROUPS` (the eight §3 groups, help text and order) and
  `x-subgroup`/`x-subgroup-blurb` on every SCMS node.

Checks, each shown red under its named fault and reverted: `publish::tests::
every_scms_string_setting_offers_a_menu` (the three §10 faults: butterfly arm, KEM words,
electors bound), `scms_presets_lie_inside_their_ranges` (a preset moved out of range),
`the_scms_menu_ranges_are_enforced` (the loader call removed),
`phase2::settings_menu_tests::the_published_sets_are_the_parsers_sets` (a published latency id
the parser refuses) and `the_promoted_lifecycle_keys_act_like_their_params_spelling` (a key
dropped from `entries()`). No golden moved.

**Status, A2 (2026-10-29): built.** The UI of §4.2 items 1, 3, 4, 5, 6, 8 and 9, the engine
facts they read, and the §10 Playwright spec with its twin. Items 2 (composite picker) and 7
(US banner) are A4's.

* Engine: `x-pair` on `pseudonym_change.silent_period_s`; `x-relevant-when` from
  `options::relevant_when` (fixed-path conditions only: latency, pseudonym strategy,
  verification policy, umbrella protocol, RSU air link, the MA pipeline); the twelve
  `detection.ma.params` keys of both pipelines are typed fields with unit, range, guide row
  and their pipeline (`x-relevant-when` on `detection.ma.id`), the block carries
  `x-typed-children` and the form drops its JSON box. The loader (`MaHost::from_scenario`)
  now enforces those ranges. `detection.ma.id` publishes its default, the corroborating
  authority an absent key runs; keys only the legacy pipeline takes publish their value as
  `x-pipeline-value`, not as a default, because writing nothing never runs that pipeline.
* UI: `MapRows` (a key menu from `x-key-options` and the value's control, or a row of
  fields for a struct value) at the top level and inside a list row (a provider's
  `components`, `sites`, `capacity`); `RangePair`; an "i" button beside every menu listing
  each option with its sentence, US / not-US badge and, for a disabled one, the reason; a
  "not US" badge on the chosen value; "= 1 week 1 h" beside every `s` number; greyed rows
  with "Applies only when …" (value kept); the SCMS page's sub-headings, order and help
  from `x-subgroup`; `data-kind` on every row.
* Choice under §9: the Security & Crypto page keeps signature, KEM, envelope and
  verification policy, as the owner's layout has it; their `x-subgroup` files them only
  when they are shown on the SCMS page. Conditions inside a list element (`failures[].extra_ms`
  under `slow`, `prov[].ma_review_s` under `manual-review`) are left to the loader's refusal.

Checks, each shown red under its named fault and reverted: `publish::tests::
the_authority_parameters_are_fields_and_relevance_names_real_values` (the corroborating
pipeline's range check removed), vitest `test/scms-menus.test.ts` (the `x-typed-children`
skip removed), and `e2e-engine/scms-settings.spec.ts` against the real engine: the main
test (a keyed map reported as `json`) and its built-in twin (`check()` made a no-op: "expected
to fail, but passed"). No golden moved.

**Status, A3 (2026-10-29): built.** Words, presets and relevance for the §5-§8 keys.

* Words: the modules' `Opt` rows (A0) already give every value a label and a sentence; A3 adds
  a guide row (`x-explain`, `x-recommended`) for every §5-§8 key. A key that repeats under
  several roots (`ma_policies.*.…`, `providers[].capacity.*.…`) has one row, written at its
  first root, which `guide::hint` finds for the others.
* Presets: `options::MODULE_PRESETS`, keyed by the path below the root
  (`options::module_key`), so the same named values serve every root: the §6 capacity values
  and the §7 MA policy values, each with its source or ASSUMPTION. `decay.hour_et` is the
  same 24-hour menu as `report_decay_hour_et`.
* Relevance: the MA policy's dependent fields (`decay.factor`/`hour_et` under
  `daily-multiplier`, `half_life_s` under `half-life`, `population_guard.c`/`m` under
  `cluster`, `decision.review_s` under `manual-review`, `evidence.tau_m` under `catch`) at the
  fixed `ma_policy` root. Choice under §9: an absent mode is the preset's, which may be either,
  so it does not grey the field (`null` in `x-relevant-when`).
* Configuration-reference line added.

Check, shown red under three named faults and reverted: `publish::tests::
the_new_scms_keys_carry_help_presets_and_relevance` (the module presets not looked up; a key
under its second root finding no guide row; `decay.factor`'s preset moved to 1.9). The first
and third mask each other, so each ran in its own build. No golden moved.

**Status, A4 (2026-10-29): built.** Dependent sets and the US banner (§4.1 item 6, §4.2
items 2 and 7).

* Engine: `options::provider_sets()` gives each preset's providers, home provider first, as
  `Deployment::preset` builds them, plus where a document names its preset and its custom
  list. `actors.backend.failures[].node` carries it in `x-options-compose` (`separator: "/"`),
  `prov[].name` in `x-options-by-preset`. Choice: the sets ride on the two fields rather than
  a top-level `scms.providers_by_preset`, so the form needs nothing outside the field.
* `phase2::us_system`: on an SCMS run, `is_us` and each departure in words. The deployment's
  part is `Deployment::is_us` (preset, electors, quorum, required roles); the rest are the
  values the menus badge "not US" (envelope, signature, KEM, home-provider chain file, LOP,
  committees). Choice under §9: the banner is stricter than the manifest's `is_us`, because a
  run on a post-quantum signature is not the US system however its deployment is laid out.
  `scenario.validate` returns it as `us_system` (null off an SCMS run), and the OpenRPC
  document lists the field.
* UI: `ProviderRolePicker` (the deployment's providers beside the role menu; the home
  provider's server written bare, another's as `provider-b/crlg`; "Other…" still types a
  committee member), provider names filtered by the document's preset (any name under
  `custom`), and the banner at the top of Certificates & SCMS from the last Check ("This run
  is the US system", "Not the US system: electors 7, signature ecdsa-p384", or that the run
  has no SCMS).

Checks, each shown red under its named fault and reverted:
`phase2::settings_menu_tests::the_us_verdict_names_each_departure` (the electors reason
dropped), `publish::tests::the_failure_server_and_provider_name_follow_the_deployment` (the
compose insertion dropped), vitest `test/scms-compose.test.ts` (the home provider's server
written qualified; the verdict not carried to the store). `v2xw-server --test rpc` passes
with the new field in the OpenRPC document. No golden moved.

**Status, A5 (2026-10-29): built.** The final pass over the SCMS help. 57 SCMS settings had no
guide row (deployment, providers, the committee block, the pseudonym and signer-identifier
numbers, the backend entities, links and failures); each now has a real-world sentence and a
recommended value with its source, so the one line under every SCMS control is the guide's.
`publish::tests::every_scms_setting_explains_itself_under_its_heading` keeps it so: every SCMS
leaf has `x-explain`, every one but the relay's `net.rsu_ip` sits under one of the eight §3
sub-headings, and each heading has its help. Faults, each in its own build because the first
masks the second: the `security.kem` guide row removed, and `security.kem` filed under no
sub-heading. No golden moved.

After A0, B and C edit only their own files. They make a key live by flipping their own
module's `KEY_STATUS` and refusal, never by editing `validate.rs`.

**Waves (three agents each).**

| Wave | A: menus | B: MA policy | C: topology |
|---|---|---|---|
| 0 | A0 scaffolding (alone) | — | — |
| 1 | A1: engine menus (§4.1 items 1–5, 8, 9) and the §10 unit test with its faults | B1 | C1a |
| 2 | A2: UI (§4.2) and the §10 Playwright spec with its twin | B2 | C1b |
| 3 | A3: words, presets and relevance for the §5–§8 keys, and configuration-reference lines | B3 | C2a |
| 4 | A4: dependent sets (`x-options-compose`) and the US banner | B4 (optional) | C2b |
| 5 | A5: final pass over every group's help, plus guide rows | — | C3a |
| 6 | — | — | C3b |

There are 18 pieces of about 3 h each, 17 without B4. Each agent merges through the
`main-merge` lock in its own column order. After each merge, the shipped-scenario digest
suite must show no change.
