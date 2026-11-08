# Dataset record contract

What the engine records for the `dataset` exporter (`crates/v2xw-record/src/dataset/`),
which reads it into the release. The release layout is the [release schema](schema.md).

Everything below is recorded **only when the scenario names the `dataset` exporter**
(`exporters: [{id: dataset}]`; the gate is `v2xw_engine::dataset_records::dataset_requested`).
A run that does not name it records exactly what it did before: the new fields are
`Option`s left out of the JSON, the new channels are never written. Naming `dataset` also
turns `security.scms.report_records` on (the global view needs `ma.received`).

Conventions:

- `t`-like fields are `SimTime`, integer ns since run start (`*_ns` in the schema).
- Floats are on their D9 grid: `_deg` 1e-7, `_m`/`_mps`/`_mps2` 1e-3, `cbr` 1e-4, other
  1e-6.
- Visibility is the channel's tag in `crates/v2xw-record/src/channels.rs`. **NODE** means
  the device (or the MA) that writes the record knows every field; **GT** is ground truth;
  **NODE+GT** carries a truth join beside node fields (the exporter splits it).
- Digests: `hex16` = the first 8 octets of SHA-256, 16 lower-case hex digits.
- `msg` is the engine's message (frame) id. It is the join key between `node.tx`,
  `phy.rx`, `node.rx_sec` and `gt.tx_truth`; an observer table must not carry it (it is
  `tx_id` in the schema, oracle only). Join observer rows by `spdu_digest` instead.

## 1. Transmission content: `node.tx` (NODE), one record per frame on the air

New fields live in `node.tx.content` (`v2xw_metrics::channels::MsgContentView`); the
existing top-level fields are reused.

| Contract item | Real field | Type | Unit / meaning |
|---|---|---|---|
| spdu_digest | `node.tx.content.spdu_digest` | hex16 | over the signed SPDU octets as transmitted (an attacker's forged signature included) |
| digest_basis | `node.tx.content.digest_basis` | `"bytes"` \| `"fields"` | `fields`: no SPDU was encoded (size-modelled message); the digest is over `v2xw/dataset/fields-digest/v1` ‖ msg type ‖ signer ‖ generation time ‖ the decoded `content` JSON (`dataset_records::fields_digest`) |
| cert_id | `node.tx.pseudonym` | hex16 | HashedId8 of the signing certificate (existing field) |
| psid | `node.tx.content.psid` | u64 | `headerInfo.psid` decoded from the SPDU (32 = 0x20 for a BSM); absent for `fields` |
| size_bytes | `node.tx.spdu_bytes` (SPDU), `node.tx.bytes_on_wire` (frame) | u64 | bytes (existing) |
| msg_cnt | `node.tx.content.msg_count` | u8 | J2735 `msgCnt` (existing) |
| temp_id | `node.tx.content.temp_id` | hex8 | J2735 `id` (existing) |
| sec_mark_ms | `node.tx.content.sec_mark_ms` | u16 | ms within the minute (existing) |
| generation_time_us | `node.tx.content.generation_time_us` | u64 | 1609.2 Time64, TAI µs since 2004-01-01, decoded from the SPDU header; for `fields`, the engine's Time64 of the claimed generation time |
| lat_deg, lon_deg | `node.tx.content.lat_deg`, `.lon_deg` | f64 | degrees (existing) |
| elevation_m | `node.tx.content.elev_m` | f64 | metres (existing) |
| pos_accuracy.semi_major_m | `node.tx.content.pos_semi_major_m` | f64 | metres (existing field, now also under the dataset gate) |
| pos_accuracy.semi_minor_m | `node.tx.content.pos_semi_minor_m` | f64 | metres (same) |
| pos_accuracy.orientation_deg | `node.tx.content.pos_orientation_deg` | f64 | degrees clockwise from north (same) |
| speed_mps | `node.tx.content.speed_mps` | f64 | m/s (existing) |
| heading_deg | `node.tx.content.heading_deg` | f64 | J2735, clockwise from north (existing) |
| accel.long_mps2 | `node.tx.content.accel_long_mps2` | f64 | m/s² (existing field, now also under the gate) |
| accel.lat_mps2 | `node.tx.content.accel_lat_mps2` | f64 | m/s² |
| accel.vert_mps2 | `node.tx.content.accel_vert_mps2` | f64 | m/s² (J2735 LSB 0.02 G × 9.80665) |
| accel.yaw_rate_dps | `node.tx.content.yaw_rate_dps` | f64 | deg/s (J2735 has no `unavailable`: 0 may mean unknown) |
| brakes | `node.tx.content.brakes` | u8 | raw `BrakeAppliedStatus` bits (bit 4 = unavailable) |
| event_flags | `node.tx.content.event_flags` | u16 | raw `VehicleEventFlags` bits; absent when Part II carried none |
| length_m, width_m | `node.tx.content.length_m`, `.width_m` | f64 | metres (LSB 1 cm) |

All BSM content is decoded from the payload octets the sender encoded, so an attacker's
row carries the falsified on-air values. Part I fields come only for BSMs (PSM: the
existing subset).

## 2. Reception security: `node.rx_sec` (NODE, new), one record per settled reception

| Field | Type | Meaning |
|---|---|---|
| `t` | SimTime | when the receiver settled it |
| `rx` | NodeId | the receiving OBU (its own record) |
| `msg` | u64 | join to `phy.rx.msg` / `node.tx.msg` (oracle key, see conventions) |
| `spdu_digest` | hex16 | as the receiver computes it from the octets it got (= the sender's `node.tx.content.spdu_digest`) |
| `cert_id` | hex16 | the SPDU's signer HashedId8 |
| `msg_type` | string | `bsm`, … |
| `verify_result` | string | `ok`, `not_checked` (accepted unverified), `bad_signature`, `on_crl`, `unknown_issuer`, `outside_validity` (1609.2 generation time outside the certificate's validity: covers expired and not-yet-valid; the receiver does not tell them apart), `stale`, `future` (generation-time window), `chain_invalid`, `psid_not_permitted`, `psid_not_registered`, `outside_region`, `crl_overdue`, `crl_rejected`, `dropped_unchecked` (queue overflow or policy skip before any check), `unsecured` (P2PCD response) |
| `verify_latency_us` | u64? | `verify_done − verify_start`; null when no check ran |
| `cbr` | f64? | the receiver's last closed `mac.cbr` window, ratio |

Only receptions the receiver's stack decoded produce a row (`phy.rx` losses do not).

## 3. `ma.received` (NODE) gains (`MaReceivedDataset`, flattened into the row)

Read from the report the deciding MA decrypted and opened (`OpenedReport`).

| Field | Type | Meaning |
|---|---|---|
| `report_digest` | hex16? | over the reporter-signed MBR SPDU (COER, the decrypted inner `Ieee1609Dot2Data`); null on the modelled path (no bytes) |
| `psid` | u64 | the application PSID the report covers: 32 (`Psid20Mbr`). The MBR SPDU itself is signed under PSID 0x26 |
| `mbr_type` | string? | `1VInconsistent`, `Implausible`, `ObsImplausible` |
| `subtype_codes` | `[{type, subtype, name, params}]` | SM-MBR v1.1 subtypes; `params` as `name=value` strings in the SM-MBR unit (`gpsDrift_mm=1000`, `threshold_mps=90`, …) |
| `nonstandard_triggers` | `[string]` | the reporter's fired checks with no SM-MBR (or TS 103 759) code. NOTE: today this is read from the engine's report object, which the MA would not see on the wire (the MBR carries only SM-MBR subtypes); keep it flagged as research metadata |
| `generation_time_us` | u64? | the signed report's 1609.2 `headerInfo.generationTime` (Time64, TAI µs since 2004) as the MA read it; null on the modelled path |
| `generation_location` | `{lat_deg, lon_deg, elevation_m?}`? | the report header's `generationLocation` |
| `evidence` | `[{spdu_digest, generation_time_us, sec_mark_ms, msg_cnt, lat_deg, lon_deg, speed_mps, heading_deg, accel_long_mps2, brakes, pos_accuracy_m}]` | each `signedBsmList` entry: digest over its COER re-encoding (equal to the sender's `node.tx.content.spdu_digest`, tested), fields decoded from its UPER BSM; `brakes` raw bits, `pos_accuracy_m` = semi-major |
| `la_link_t` | SimTime? | always null today, see §11 |

Existing fields kept: `reporter_cert`, `subject_cert`, `issuer`, `la_link`, `set_aside`,
`generation_time` (SimTime of the report), `evidence_start/_end`.

## 4. `pki.report_fate` (NODE+GT) gains (`ReportFateDataset`, flattened)

Present on every fate logged after the report was built and secured (`delivered` and
later ones); absent on `stored`, on `uploaded` and on modelled-path reports. The engine
builds a report's octets when its upload reaches the Location Obscurer Proxy, after the
`uploaded` step, so the build has a record of its own, `node.report_built` (§7a): a
report still in the RA's shuffle when the run ends has no later fate to carry it.

| Field | Type | Meaning |
|---|---|---|
| `report_digest` | hex16? | same digest as `ma.received.report_digest` (the device holds the same inner SPDU) |
| `size_bytes` | u64? | the secured (encrypted) report, bytes |
| `path` | string? | `cellular` or `rsu_relay` |
| `evidence_spdu_digests` | `[hex16]` | the evidence BSMs' digests |

The existing `truth` object is the GT part; the exporter must keep it out of `local/`.

## 5. `ma.decision` (NODE) gains (`MaDecisionDataset`, flattened)

| Field | Type | Meaning |
|---|---|---|
| `ma` | u32 | the deciding MA's provider index |
| `rule` | string | the scenario's `detection.ma.id` (e.g. `threat/ma/legacy-window`) |
| `subject_kind` | string | `device`: the US MA links the reported certificate to its chain through the LAs inside the case and revokes the device |
| `la_link` | hex16? | the MA's link token of the subject (the same token as `ma.received.la_link`) |
| `report_digests` | `[hex16]` | every report about the decided certificate this MA had received by the decision (`report_id` stands in for a modelled-path report with no bytes) |
| `n_reports`, `n_distinct_reporters` | u64 | their count, and their distinct reporter certificates |

`subject` (the certificate digest), `decision`, `t` are existing. Only the US-path
decision (case `decision` stage) carries the extension; ETSI-model decisions do not.

## 6. CRL publication: `ma.crl_entry` (NODE, new)

One row per linkage-seed entry a case put on the pseudonym CRL.

| Field | Type | Meaning |
|---|---|---|
| `t_issued` | SimTime | the case's `issued` stage (the CRL carrying the entry was signed) |
| `crl_series` | u64 | 1 (pseudonym CRL series) |
| `crl_number` | u64 | the CRL generator's list version carrying the entry |
| `entry_kind` | string | `linkage_seed` (`hash_id` entries are not produced by the MA path) |
| `la_link` | hex16 | the MA's token of the revoked device |
| `i_period` | u64 | the entry's `iRev` |
| `t_effective` | SimTime | = `t_issued` (a device applies a CRL on receipt; when each device got it is in `node.pki_event`) |
| `subject_cert_id` | hex16 | the certificate the decision named |

## 7. `node.pki_event` (NODE, new)

Derived from successive `node.security` windows of the device (so `t` has the telemetry
window's resolution, about 1 s).

| Field | Type | Meaning |
|---|---|---|
| `t`, `node` | | the window, the device (its own record) |
| `event` | string | `enrolled` (first window), `pseudonym_change`, `batch_downloaded` (stored certificates grew), `crl_downloaded` (installed CRL version grew) |
| `cert_id` | hex16? | the pseudonym it signs with after the event |
| `crl_series`, `crl_number`, `crl_entries` | u64? | for `crl_downloaded`: the newest held CRL's series and counter, the entries held |
| `bytes`, `latency_ms` | u64? | always null today, see §11 |

## 7a. `node.report_built` (NODE, new)

One record per report built and secured for upload, when its upload reaches the Location
Obscurer Proxy (`Phase2::report_at_proxy`). The reporter's own knowledge of its own report;
the field names are `pki.report_fate`'s, so the exporter reads both with one column map.

| Field | Type | Meaning |
|---|---|---|
| `t` | SimTime | the build was recorded (the upload's arrival at the proxy) |
| `reporter` | NodeId | the reporting device (its own record) |
| `report_id` | string | the report's id on its device, as on `pki.report_fate` |
| `subject` | hex16 | the accused certificate, as on `pki.report_fate` |
| `report_digest`, `size_bytes`, `path`, `evidence_spdu_digests` | | as in §4 |

## 8. `gt.tx_truth` (GT, new), one per `node.tx` row

`t` (transmit start), `msg`, `node`, `x_m`, `y_m` (true position at transmit start, world
ENU metres), `speed_mps`, `heading_deg` (clockwise from north), `accel_mps2` (true
longitudinal), `gnss_error_m` (the sender's own position estimate against the truth: the
honest error, also for an attacker), `fix_to_stamp_ms` (null, see §11).

## 9. `gt.device` (GT, new), once per device, at its first transmission

`t`, `node`, `class` (`vehicle` / `rsu` / `vru`), `provider` (SCMS provider name, null
without SCMS credentials), `gnss_class` (`bsm-timing-class-<k>` when the BSM timing model
runs, else null), `faulty` (bool), `fault_kind`, `attacker` (bool; also in
`gt.device_exposure`).

## 10. `app.warning` / `app.outcome` gain `msg`

Not recorded yet, see §11.

## Already present and sufficient

`phy.rx` (tx, rx, msg, rssi_dbm, outcome), `gt.attack.action` (msg, actor, attacker,
fields, magnitude), `gt.subject` (cert → actor), `gt.device_exposure`, `gt.kinematics`,
`sec.cert`, `node.credentials`.

## 11. Not recorded, and why

| Item | Reason |
|---|---|
| `ma.received.la_link_t` | the row is written when the report reaches the MA, before any case asks the LAs; the LA answer's time is the case's `resolved` stage on `proto.revocation` (stage stamps by subject digest), which the exporter can join |
| `gt.tx_truth.fix_to_stamp_ms` | the BSM timing model's fix instant is not handed to the engine with the transmission (`Transmission` has the claimed position, not its fix time) |
| `gt.device.faulty` / `fault_kind` | the engine has no out-of-specification device fault model yet; every honest device is written `faulty: false` |
| `node.pki_event` `download_refused`, `bytes`, `latency_ms` | not visible in `node.security`; the PDUs and their sizes are on `pki.flow` (public PDU log) |
| `app.warning.msg`, `app.outcome.msg` | the safety apps track neighbours by certificate, not by message; plumbing the triggering message id needs a change in `v2xw-node` safety apps (open) |
| `ma.decision` for the ETSI model | no case, no LA linking: left without the extension |
