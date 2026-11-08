# SCMS constants and their sources

Every default the SCMS code runs on, with where it lives and why it has the value it has.

**Status** is one of four:
- **CITED**: a primary source prints the value. A unit conversion of a printed value, such as 6 years to seconds, still counts as cited.
- **DERIVED**: computed from cited inputs, or produced by the engine's own real encoder and pinned by a test.
- **MEASURED**: measured by a script in this repository that anyone can rerun.
- **ASSUMPTION**: no source prints the value. The row gives the reason for the value chosen, and every such value can be changed in the scenario or the library.

**How this file is checked.** The tables are machine-read, so a value changed in the code but not here (or here but not in the code) fails a test:
- `crates/v2xw-proto/tests/model_card.rs` (`constants_md_matches_the_scms_card`) checks every parameter on the `protocol/scms/camp` model card. Each one must have a row here with the same default. A row is ASSUMPTION exactly when the card marks it `todo-calibrate`, and each such row carries a plan.
- `crates/v2xw-engine/tests/scms_constants.rs` does the same for the engine, node and security constants the SCMS path uses.
- Rows that neither test reads are listed for completeness. Their values are pinned by the tests named in the row.

**Sources.** The tags in the tables are:
- **[CAMP122]** CAMP VSC5, "SCMS PoC Implementation — EE Requirements and Specifications Supporting SCMS Software Release 1.2.2", 2016-11-15 (posted 2023-07-19), 653 pp. https://pronto-core-cdn.prontomarketing.com/2/wp-content/uploads/sites/2896/2023/07/SCMS-CV-Pilots-Documentation-20230719.pdf
- **[SMP]** SCMS Manager, "IEEE 1609.2.1 Profile", v1.0.3, 2023-03-15. Profiles IEEE 1609.2.1-2022. https://www.scmsmanager.org/wp-content/uploads/2023/07/IEEE1609Dot2Dot1-SCMS_Manager_Profile_1_0_3.pdf
- **[SM-MBR]** SCMS Manager, "Misbehavior Report and Application Specification for Connected Vehicle Pilot Deployment", v1.0, 2019-02-14. https://scmsmanager.org/wp-content/uploads/2020/01/Misbehavior-Report-and-Application-Specification-v1.0.pdf
- **[PRIMER]** USDOT ITS JPO, "SCMS Technical Primer", FHWA-JPO-19-775, 2019. https://rosap.ntl.bts.gov/view/dot/43635
- **[SM-PROV]** SCMS Manager, "Provider Requirements", v1.1, 2024-08-09. https://www.scmsmanager.org/wp-content/uploads/2024/10/SCMS-Manager-Provider-Requirements-v1_1.pdf
- **[CVP-P2]** USDOT ITS JPO, "CV Pilot Deployment Program: Driving Towards Deployment — Lessons Learned from the Design/Build/Test Phase", 2018. https://rosap.ntl.bts.gov/view/dot/37681
- **[BRECHT]** Brecht et al., "A Security Credential Management System for V2X Communications", IEEE T-ITS 2018, arXiv:1802.05323
- **[SM-REENR]** SCMS Manager, "EE Certificate Rollover (Re-Enrollment) Technical Standard", v1.0, 2020-10-08. https://www.scmsmanager.org/wp-content/uploads/2020/10/SCMS-Manager-EE-Re-Enrollment-Technical-Specification-v1.0.pdf
- **[SM-MBR-ASN]** SCMS Manager, "ASN.1 for Misbehavior Reporting", v1.1. https://scmsmanager.org/wp-content/uploads/2020/01/ASN.1-for-Misbehavior-Reporting-v1.1.pdf
- **[SM-EESEC]** SCMS Manager, "End-Entity Security Requirements, Design Guidance, and Validation Approach", v2.04, 2023-10-12. https://www.scmsmanager.org/wp-content/uploads/2023/10/SCMS-Manager-End-Entity-Security-Requirements-v2_04.pdf
- **[CVP-LOG]** USDOT ITS JPO, "CV Pilot Deployment Lessons Learned Logbook Synthesis", 2022. https://rosap.ntl.bts.gov/view/dot/66286
- **[RSU41]** USDOT, *Specifications for DSRC Roadside Unit v4.1*, 2016-10-31 (FHWA-JPO-17-589), §2.1, §2.2 and Appendix C. The PDF is not committed: it is a federal work, but its licence was not confirmed.
- **[ISART05]** R. Roebuck (Sirit), *Emerging DSRC Technology*, ISART 2005 slides, p.8: https://its.ntia.gov/media/33194/fre_j_slides.pdf. A vendor figure, so a secondary source.

---

## 1. The model card (`protocol/scms/camp`, `crates/v2xw-proto/src/scms/mod.rs`)

### 1.1 Calendar and provisioning

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `i_period_minutes` | 10080 | min | CITED | `params.rs` `i_period` | One week. [SMP] §2.1.17 Table 2; [CAMP122] Table 3; CAMP-EE R1.1 §2.1.5.3.2 |
| `cert_lifetime_minutes` | 10140 | min | CITED | `params.rs` `cert_lifetime` | One week plus the 1 h overlap. [SMP] Table 2 overlap = 1 h; CAMP-EE R1.1 §2.1.5.3.2 |
| `calendar_epoch_unix_s` | 1420534800 | s | CITED | `governance.rs` `US_IPERIOD_EPOCH_UNIX_S` | iPeriodEpoch, 2015-01-06 04:00 EST = 09:00 UTC. [SMP] §2.1.17 Table 2. Gives i = 614 on 2026-10-16 (`tests/calendar.rs`) |
| `epoch_ns` | 0 | ns | ASSUMPTION | `params.rs` `epoch` | Used only by library runs and the compressed calendar, where i-period 0 starts at the run's origin. The real calendar ignores it |
| `certs_per_period` | 20 | - | CITED | `params.rs` | [CAMP122] Table 67 certs_per_i_period; [PRIMER] p.8. NYC used 60 ([CVP-P2] item 24) |
| `initial_batch_certs` | 3120 | - | CITED | `params.rs` | 20 × 52 × 3 = 3,120. [PRIMER] p.7 |
| `max_periods_ahead` | 156 | - | CITED | `params.rs` | Three years of weekly batches. [PRIMER] p.7; [CAMP122] Table 67 max_available_cert_supply |
| `ra_min_preload_periods` | 2 | - | CITED | `params.rs` `RA_MIN_PRELOAD_PERIODS` | ra-maxPreloadTime ≥ 2 weeks. [SMP] §2.1.21 Table 3 |
| `ra_max_preload_periods` | 156 | - | CITED | `params.rs` `RA_MAX_PRELOAD_PERIODS` | ra-maxPreloadTime ≤ 3 years. [SMP] Table 3 |
| `device_pool_periods` | 2 | week | ASSUMPTION | `scms/mod.rs` `DEVICE_POOL_PERIODS`; engine `pool_periods` | The device's own download strategy, configured by the OEM. The pilots bracket it: NYC kept 1 week ahead ([CVP-P2] items 21, 24) and THEA took 156 weeks at once ([CVP-LOG] p.16). `params::PreloadPolicy` selects either one. It replaces the card-only `download_horizon_weeks` (4), which no code read |
| `first_batch_delay_s` | 60 | s | ASSUMPTION | `params.rs` | [CAMP122] §5.2.7.6.7.1 allows "some delay" for shuffling and generation but prints no number. Take the value from the RA being modelled |
| `download_poll_interval_s` | 300 | s | CITED | `params.rs` | download-minWait 5 min. [SMP] Table 3, §4.3.1 |
| `max_download_polls` | 100 | - | CITED | `params.rs` | download-maxReqs 100. [SMP] Table 3 |
| `ee_retry_timeout_s` | 10 | s | CITED | `params.rs` `EE_RETRY_TIMEOUT_S` | The EE retries a request with no response in 10 s. [CAMP122] §5.2.8.3 SCMS-522. Applies to the butterfly request (sent again byte for byte), the `.info` top-off and each batch download. Scenario key `ee_retry_timeout_s`; 0 turns it off |
| `ee_retry_max_per_hour` | 10 | - | CITED | `params.rs` `EE_RETRY_MAX_PER_HOUR` | At most 10 retries per 60 min. [CAMP122] §5.2.8.3 SCMS-523. The RA re-acks a byte-identical retransmission (ASSUMPTION: CAMP states the retry, not the answer) |
| `download_request_tolerance_s` | 5 | s | CITED | `params.rs` | ra.client-signature-ttl-sec. [CAMP122] SCMS-1203 |

### 1.2 Enrolment and re-enrolment

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `enrolment_lifetime_s` | 189216000 | s | CITED | `params.rs` | 6 years. [CAMP122] Table 3, SCMS-1307. Other values in the sources: 7 y (CAMP R1.1) and 3 y (5GAA draft CP (419)). |
| `reenrol_lead_s` | 47347200 | s | DERIVED | `params.rs` | 548 days, the midpoint of the "12–24 months before expiry" that [SM-REENR] §1–3 recommends |
| `reenrol_download_delay_s` | 300 | s | CITED | `params.rs` | The RA's nextDlTime. download-minWait 5 min, [SMP] Table 3. The flow is two-phase per [SM-REENR] |
| `electors` | 5 | - | CITED | `params.rs` (= `deployment::US_ELECTORS`) | Elector Tech Spec v1.1 §2.1; [SMP] §2.1.15; every published CTL lists five |
| `elector_quorum` | 3 | - | CITED | `params.rs` (= `deployment::US_QUORUM`) | Any 3 of 5. Elector Tech Spec v1.1 §2.4 ("any three (3) are necessary and sufficient"); the published CTLs since sequence 1 carry quorum 3 (decoded) |

### 1.3 Shuffles (privacy)

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `shuffle_threshold_requests` | 1000 | - | CITED | `service.rs` `CAMP_122_CERT_SHUFFLE` | [CAMP122] §5.2.15.5 Table 67 shuffle_threshold, SCMS-631. The conflict with [BRECHT] is settled in favour of CAMP 1.2.2. [BRECHT]'s 10,000 per day is available as a setting |
| `shuffle_window_s` | 172800 | s | CITED | `service.rs` `CAMP_122_CERT_SHUFFLE` | [CAMP122] Table 67 max_cert_request_age, 2 days |
| `report_shuffle_threshold` | 10000 | - | CITED | `service.rs` `CAMP_SHUFFLE` | [CAMP122] §5.2.8.4, SCMS-765 |
| `report_shuffle_window_s` | 86400 | s | CITED | `service.rs` `CAMP_SHUFFLE` | One day. [CAMP122] SCMS-765 |
| `pseudonym_change_interval_s` | 300 | s | CITED | card | SAE J2945/1 6.5.3, Table 21 `vCertChangeInterval` 5 min (primary); also [PRIMER] pp.8–9, [CVP-P2] item 25 |
| `pseudonym_change_distance_m` | 2000 | m | CITED | card | NYC rule: every 2 km or 5 min. [PRIMER] pp.8–9. The J2945/1 rule is 2,100 m ± 100 m of straight-line distance (Table 21 `vCertChangeDistance`) |
| `cert_attach_interval_ms` | 450 | ms | CITED | card | SAE J2945/1 6.5.2 `BSMSIGN-003`, Table 21 `vMaxCertDigestInterval` 450 ms (primary). The scenario default is 1,000 ms |

### 1.4 Revocation

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `crl_cadence_s` | 86400 | s | CITED | `params.rs` | Publish within 1 business day of a change, [SM-PROV] §3.4.10. The US presets run that rule exactly (`crl.business_day_sla`). The 5GAA draft CP (255) says within 24 h |
| `next_crl_s` | 604800 | s | CITED | `params.rs` `CrlIssuance::next_crl` | Devices fetch "no later than once a week", [CAMP122] §3 item 10. The draft CP's ceiling is monthly ([5GAA] (254)) |
| `crl_slot_et_hour` | 10 | h | ASSUMPTION | `params.rs` `CrlIssuance::slot_et` | [SM-PROV] §3.4.10 fixes the deadline, not the hour |
| `crl_entry_bytes` | 40 | B | CITED | `sizes.rs` `CRL_LINKAGE_ENTRY_BYTES` | [BRECHT] §VI-F: 32 B of seeds plus group overhead. Used only on the modelled path: since the edge-case package a device's CRL download (cellular, RSU backhaul, MBMS) is charged the encoded `CompositeCrlSpdu` |
| `crl_max_forward_periods` | 156 | - | CITED | `v2xw-sec` `linkage::DEFAULT_MAX_FORWARD_PERIODS` | The CAMP end-entity lifetime of 156 weekly periods |

### 1.5 Misbehaviour Authority and Linkage Authorities

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `ma_max_open_cases` | 1000 | - | ASSUMPTION | `params.rs` `MaPolicy` | No MA capacity is published. Sweep it in a saturation study |
| `ma_review_s` | 28800 | s | ASSUMPTION | `params.rs` `MaMode::ManualReview` | The default is the pilots' manual MA ([CAMP122] §4.2–4.3; [PRIMER] pp.6, 8), which prints no review time. 8 h is one working shift. 0 selects the automatic MA |
| `la_aca_key_rotation_s` | 604800 | s | ASSUMPTION | `params.rs` `PduLayer` | [CAMP122] Table 68 shared_key_update_interval is a local option with no published value. It is set to one i-period |
| `seed_retry_s` | 30 | s | ASSUMPTION | `params.rs` | CAMP describes no MA-to-LA retry. The value keeps a two-of-two LA retry comparable to a committee session |

### 1.6 Backend service and links

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `backend_hw_profile` | `hw-profile/i9-11950h-wolfssl` | - | CITED | `params.rs` | the published wolfSSL benchmark. Appliance transaction rates are NOT PUBLISHED |
| `device_hw_profile` | `hw-profile/cohda-mk6-botan` | - | CITED | `params.rs` | Cohda MK6 with Botan (published benchmark) |
| `hybrid_backend_pq_profile` | `hw-profile/cortex-a76-pi5-liboqs` | - | CITED | `hybrid.rs` | arXiv 2503.10238 Table 8 |
| `hybrid_device_pq_keygen_profile` | `hw-profile/cortex-a72-pi4-liboqs` | - | CITED | `hybrid.rs` | arXiv 2503.10238 Table 8 |
| `backend_servers` | 4 | - | ASSUMPTION | `params.rs` | The c of M/M/c. No provider publishes its server count. Record it per scenario |
| `backend_overhead_us` | 1000 | us | ASSUMPTION | `params.rs` | Per-request cost on top of the cryptography. RA, PCA and MA transaction rates are NOT PUBLISHED |
| `device_overhead_us` | 1000 | us | ASSUMPTION | `params.rs` | Per-request cost on the OBU outside the crypto engine. Not published |
| `backend_link_latency_ms` | 10 | ms | ASSUMPTION | `params.rs` | Used only by library runs that have no deployment. Under `security.scms.deployment` (every engine run) it is replaced by the deployment's intra-region latency (§3) |
| `backend_link_bandwidth_bps` | 1000000000 | bit/s | ASSUMPTION | `params.rs` | No provider publishes its provisioned capacity |
| `uu_link_latency_ms` | 50 | ms | ASSUMPTION | `params.rs` | Used only by library runs. An engine run carries every device leg over `net.uu` (cited presets) |
| `uu_link_bandwidth_bps` | 10000000 | bit/s | ASSUMPTION | `params.rs` | As `uu_link_latency_ms` |
| `v2x_air_bandwidth_bps` | 6000000 | bit/s | CITED | `params.rs` | QPSK rate 1/2 on the 10 MHz OFDM PHY. EN 302 663 V1.3.1 Annex C.3 Table C.1 |
| `v2x_air_latency_ms` | 1 | ms | ASSUMPTION | `params.rs` | Library-only channel-access delay. The engine puts roadside broadcasts through its MAC instead |
| `availability.model` | `always-up` | - | ASSUMPTION | `service.rs` `AvailabilityModel`, `kernel.rs` `availability_windows` | Off by default. `two-state`: up times exponential with mean MTBF, down times exponential with mean MTTR (`todo-calibrate`), each interval a keyed hash of (seed, entity, k, kind). Every entity starts up at 0. No provider publishes its availability; the values are for the sensitivity sweeps |
| `availability.mtbf_h` | 720 | h | ASSUMPTION | `AvailabilityModel::mtbf` | Mean time between failures. Not published. Menu 24, 168, 720, 8,760 h |
| `availability.mttr_min` | 30 | min | ASSUMPTION | `AvailabilityModel::mttr` | Mean time to repair. Not published. Menu 5, 30, 60, 240 min. A drawn outage is an `offline` fault, so the backend retry (`BACKEND_RETRY`) and the EE timeout handle it as a `failures[]` window |
| `service_time.model` | `deterministic` | - | ASSUMPTION | `service.rs` `ServiceTimeModel`, `ServiceQueue::admit` | Off by default. `log-normal`: each request's `work + fixed_overhead` times `exp(σ·z)`, `μ = 0`, `σ = sqrt(ln(1 + cv²))`, so the median is the cost table's time; `z` is a Box-Muller normal from two keyed hashes of (seed, entity, request index, kind), integer ns after rounding. The shape is the `high` node tier's, not measured |
| `service_time.cv` | 0.25 | - | ASSUMPTION | `ServiceTimeModel::LogNormal::cv_milli` (thousandths) | Spread of service times (coefficient of variation). Not published. Menu 0.1, 0.25, 0.5, 1.0 (range 0-3, step 0.05) |

### 1.7 Wire sizes

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `report_payload_bytes` | 524 | B | DERIVED | `sizes.rs`; `mbr.rs` `REFERENCE_REPORT_COER_BYTES` | The real COER `EndEntityMaInterfacePDU` of the reference US report: 1VInconsistent/ConstantPosition, two signed BSMs (the engine's evidence depth) and the accused's certificate. Format per [SM-MBR] v1.0 and [SM-MBR-ASN] v1.1. `tests/mbr.rs` (`the_reference_report_sizes_are_the_real_encoders`) pins it |
| `repo_url_bytes` | 64 | B | ASSUMPTION | `sizes.rs` | The CAMP RA acknowledgement's repository URL. No sample has been measured |
| `batch_container_bytes` | 76 | B | ASSUMPTION | `sizes.rs` | Framing of the `X_Y.zip` around one i-period. No real batch file has been measured |
| `linkage_chain_identifier_bytes` | 16 | B | ASSUMPTION | `sizes.rs` | CAMP names the LCI but does not publish its encoding (1609.2.1 leaves LA–RA as NULL) |
| `etsi_subject_attributes_bytes` | 64 | B | ASSUMPTION | `sizes.rs` | ETSI InnerEcRequest block. It waits on the ETSI PKI ASN.1 defect |

---

## 2. Engine, node and security constants on the SCMS path

### 2.1 Roadside relay and backhaul (`crates/v2xw-engine/src/backend.rs`)

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `relay_range_m` | 300 | m | ASSUMPTION | `RELAY_RANGE_M` | Nominal 5.9 GHz DSRC range: "300 m nominal, up to 1000 m" ([ISART05] p.8, a secondary vendor figure). [RSU41] §2.2 confirms that vehicles reach the SCMS through the RSU's IPv6 service, and may request at one RSU and receive at another, but prints no range. This radius gates only the batch download. A relayed report is a real frame, and the radio decides whether it arrives |
| `backhaul_latency_ms` | 10 | ms | ASSUMPTION | `BACKHAUL_LATENCY_MS` | No source gives the latency of an RSU's field network to the internet. The value reuses the library backend figure so the two are not separate invented numbers. The deployment's city-to-region leg is added on top. `backhaul/fibre`, `dsl` or `microwave` with a distance replaces it |
| `backhaul_capacity_mbps` | 1000 | Mbit/s | ASSUMPTION | `BACKHAUL_CAPACITY_MBPS` | A gigabit Ethernet port. [RSU41] §2.1 requires a PoE (IEEE 802.3at) Ethernet interface with IPv4 and IPv6 but gives no rate |
| `internet_one_way_ms` | 14.4 | ms | DERIVED | `INTERNET_ONE_WAY_MS` | (58.0 − 29.2) / 2, from Narayanan et al. WWW'20 Table 2 |
| `wired_rto_ms` | 200 | ms | CITED | `WIRED_RTO_MS` | Linux `TCP_RTO_MIN` (include/net/tcp.h) |
| `reconnect_s` | 1 | s | CITED | `RECONNECT_S` | RFC 6298 §2.1 initial RTO |
| `mbms_carousel_s` | 10 | s | ASSUMPTION | `MBMS_CAROUSEL_S` | TS 26.346 §7 leaves the FLUTE carousel interval to the operator |

### 2.2 TLS sessions (`backend.rs`, measured by `tools/scms_constants/tls_handshake_sizes.py`)

The script runs a real handshake between two OpenSSL 3.0.15 endpoints over memory BIOs, restricted to the US profile: server-only X.509 authentication, ECDHE-ECDSA suites and no session tickets ([SMP] §2.1.18, §3.2; [SM-EESEC] §5.2.3). It counts the bytes of each TLS-record flight. Nothing goes on the network.

The results rest on three assumptions:
- The server sends a two-certificate ECDSA P-256 chain: a leaf of 367 B DER and an issuing CA of 323 B. No provider publishes its TLS chain.
- The client sends OpenSSL's ClientHello restricted to the profile's suites. No OBU publishes its TLS stack.
- The host name is a neutral 26-character one.

Rerun it with `python -I tools/scms_constants/tls_handshake_sizes.py`. It needs `cryptography`.

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `tls_session_max_s` | 300 | s | CITED | `TLS_SESSION_MAX` | "The max lifetime of a session shall be 5 minutes", and resumption is not supported. [SMP] §3.2 |
| `tls13_client_hello_bytes` | 256 | B | MEASURED | `SCMS_TLS13_FLIGHTS.0` | The script above |
| `tls13_server_flight_bytes` | 1066 | B | MEASURED | `SCMS_TLS13_FLIGHTS.1` | ServerHello through server Finished, with the 2-certificate chain |
| `tls13_client_finish_bytes` | 80 | B | MEASURED | `SCMS_TLS13_FLIGHTS.2` | Rides with the first request. Not charged separately (open item 4) |
| `tls12_client_hello_bytes` | 173 | B | MEASURED | `SCMS_TLS12_FLIGHTS.0` | ECDHE-ECDSA-AES128-GCM-SHA256 |
| `tls12_server_flight_bytes` | 934 | B | MEASURED | `SCMS_TLS12_FLIGHTS.1` | ServerHello through ServerHelloDone |
| `tls12_client_finish_bytes` | 43 | B | MEASURED | `SCMS_TLS12_FLIGHTS.2` | The measured flight is 93 B. The engine adds 50 B for ChangeCipherSpec and record headers. The server's last flight of 51 B matches the transport's fixed 51 |

The engine's earlier defaults were a 512 B ClientHello and a 4,000 B server flight, both `todo-calibrate` and sized for an RSA chain. The SCMS carrier no longer uses them. The other transport figures (MSS, CUBIC, RTO, HTTP headers) stay on the generic `v2xw-radio` card with their own status.

### 2.3 CRL distribution and reports (`crates/v2xw-engine/src/phase2.rs`)

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `crl_fetch_interval_s` | 3600 | s | ASSUMPTION | `CRL_FETCH_INTERVAL_S` | Sits inside a cited bound. Devices fetch "as often as possible but no later than once a week" ([CAMP122] §3 item 10) and "on every RA connection" ([PRIMER] pp.5, 8), but no interval is printed. One hour is far inside the weekly bound and matches the profile's RA polling granularity (ra-minWait 1 h, [SMP] Table 3) |
| `crl_background_entries` | 0 | - | ASSUMPTION | engine `phase2.rs` `LifecycleParams::background_entries`; `run/crl.rs` `ScmsRun::seed_standing_crl` | The revocations already on the home provider's CRL at t0 (a standing list of earlier years). No deployment publishes its CRL length; 0 keeps the clean-slate run. Set it to size the list a vehicle downloads: every download (cellular, RSU backhaul, MBMS) is charged the encoded `CompositeCrlSpdu` (`ctl::encode_composite_crl_coer`), 33.08 B per linkage entry at 10,000 (`edge_cases.rs`) |
| `clock_skew_s` | 0 | s | ASSUMPTION | engine `phase2.rs` `Phase2::clock_skew_for`; node `secure.rs` `NodeSecurity::set_clock_offset` | A scenario setting; the default is the no-fault case. A vehicle clock's believed-minus-true offset (a GNSS time error held while the fix holds, or a spoofed time source). The node signs (`generationTime`, IEEE 1609.2 §6.3.9), rotates and judges validity (§5.2) on its own clock. 0 is a true clock |
| `clock_skew_share` | 1 | - | ASSUMPTION | engine `phase2.rs` | A scenario setting; the default is the no-fault case. The share of vehicles that carry `clock_skew_s`, taken by a fixed hash of the node index so a base and a variant skew the same vehicles |
| `peer_root_removal_at_s` | unset | s | ASSUMPTION | engine `phase2.rs` `governance_due`, `take_ctl_install` | A scenario setting; the default is the no-fault case. The EAC removes the second provider's root; the CTL is published 48 h later (Elector Policy v1.0 §2.2.4). Devices install a newer CTL with their next composite CRL download (the file carries the home CTL, IEEE 1609.2.1) and drop the root; RAs stop serving that provider's CRL |
| `crl_not_modified_bytes` | 92 | B | ASSUMPTION | `CRL_NOT_MODIFIED_BYTES` | The RA's HTTP 412 answer to a conditional CRL poll whose copy is current ([SMP] §4.3.1, §4.3.2: `If-Modified-Since` on 6.3.5.8 composite CRL and 6.3.5.14 information status). A minimal HTTP/1.1 response: status line 34 B, `Date:` 37 B, `Content-Length: 0` 19 B, blank line 2 B. Every due poll now sends its request; before, a poll that would find nothing new put nothing on the air |
| `http_header_set` | Host; Content-Type, Content-Length with a body; Date on answers | - | ASSUMPTION | `scms/http.rs` (`post_bytes`, `download_bytes`, `conditional_get_bytes`, `upload_bytes`, `answer_bytes`) | Every EE-RA request is charged its request line, `Host: ra.<provider>:8892` (the RA certificate's name, which its TLS certificate must match, [SMP] §3.2), and `Content-Type: application/octet-stream` and `Content-Length` when it has a body ([CAMP122] §6.1); a `GET` download carries its `SecuredAuthenticatedDownloadRequest` Base64 in `Download-Req`, padded `4·⌈n/3⌉` (RFC 4648 §4), never also as a body; the file check is two conditional `GET`s with `If-None-Match` (`"local_policy_<g>_<l>.oer"`, SCMS-544; the LCCF's tag `"local_certificate_chain_<v>.oer"` is unpublished), answered `200` or `304`. Answers carry `Date` (29 B IMF-fixdate, RFC 9110 §6.6.1) and `Content-Length`, plus `Content-Type` with a body; `304` carries `Date` only; a resumed batch is `206` with `Content-Range`; refusals are `500` around the modelled refusal body. No `User-Agent`, `Accept`, `Connection` or `Server`. Paths are [CAMP122] Table 79's; the 1609.2.1 v3 paths are paywalled (UNCONFIRMED). The 412 answer this set gives is `crl_not_modified_bytes` (92 B) |
| `crl_store_path` | `/download-crl/` on `ma.<provider>:8894` | - | CITED | `phase2::CRL_STORE_PATH`, `Phase2::crl_poll_request_bytes` | CAMP; the IEEE 1609.2.1 path string is UNCONFIRMED. A vehicle's composite CRL poll is an unauthenticated conditional `GET` ([CAMP122] §6.2.1, §6.2.3; [SMP] §4.3.2 `If-Modified-Since`), no body: 107 B with the 29 B IMF-fixdate. Before B9b it was charged the size model's signed request (`crl_request`, 97 B). The answer is `http::answer_bytes`: `200` with the composite, or the 92 B `412`. The roadside unit's download answer is framed the same way; its request is still not charged. |
| `info_status_path` | `/download/info-status` on `ra.<provider>:8892` | - | ASSUMPTION | `phase2::INFO_STATUS_PATH`, `Phase2::info_status_request_bytes` | The information-status poll (1609.2.1 §6.3.5.14, `If-Modified-Since`). No CAMP table names its path and the 1609.2.1 string is UNCONFIRMED; a path beside Table 79's `/download/info`. |
| `backend_retry_s` | 10 | s | ASSUMPTION | `v2xw-proto/src/fault.rs` `BACKEND_RETRY` | How often an SCMS component retries a message to a component that is down (an LA's PLVs or the ACA's certificates to a down RA). No document specifies inter-component retry: 1609.2.1 leaves RA↔LA `NULL`, and CAMP gives only the EE's 10 s (SCMS-522), reused here. A down component's own timers run when it restarts |
| `crl_broadcast_interval_s` | 5 | s | CITED | `CRL_BROADCAST_INTERVAL_S` | The interval is cited; the mechanism is an assumption. No document specifies a roadside CRL broadcast: [PRIMER] p.5 names the path, and TS 102 941 Annex D.3 gives no rate. A US RSU repeats back-office content as Store and Repeat, whose TxInterval "1 to 5" seconds ([RSU41] Appendix C, p.117) bounds this. 5 s is the longest allowed and costs the least channel time |
| `report_bytes` | 1119 | B | DERIVED | `report_bytes()`; `mbr.rs` `REFERENCE_UPLOAD_BYTES` | The upload of the reference report: 804 B signed and encrypted plus a 315 B Download-Req header, from the real encoder (`tests/mbr.rs`). It is charged only where a run has no real report to send; a real report is charged its own length |
| `report_store_quota_bytes` | 65536 | B | ASSUMPTION | `REPORT_STORE_QUOTA_BYTES` | [SM-MBR] §2.2 leaves the quota to the device. 64 KiB holds 58 reference reports |
| `report_daily_multiplier` | 0.9 | - | CITED | `ReportStorePolicy`; `mbr.rs` `SubtypeWeights` | [SM-MBR] §2.3.1 `DailyMultiplier`, every PSID-0x20 subtype. The spec gives each subtype its own; one knob moves all |
| `report_decay_hour_et` | 5 | h | CITED | `ReportStorePolicy`; `mbr.rs` `DECAY_HOUR_ET` | [SM-MBR] §2.2: "Every day, at 5 am Eastern time" |
| `report_weight_*` | 1.0 | - | CITED | `ReportStorePolicy`; `mbr.rs` `SubtypeWeights` | [SM-MBR] §2.3.1.2.2–§2.3.1.3.4 `initialWeight` per subtype. A report listing several subtypes takes the largest (ASSUMPTION: §2.2 does not say) |
| `report_interval_s` | 1 | s | ASSUMPTION | `REPORT_INTERVAL_S` | A one-second reporting step. [SM-MBR] prints no re-report interval |
| `topup_below_periods` | 1 | - | ASSUMPTION | `LifecycleParams` | Top up when only the current week remains. A device strategy, as `device_pool_periods` |
| `evidence_bsms` | 2 | - | DERIVED | `EVIDENCE_BSMS` | The fewest BSMs a 1VInconsistent report needs ([SM-MBR] §2.3). An Implausible report keeps the last one |

### 2.4 Receiver validation and P2PCD (`v2xw-node/src/rxval.rs`, `v2xw-sec/src/p2pcd.rs`)

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `overdue_crl_tolerance_s` | 1209600 | s | CITED | `rxval.rs` `OVERDUE_CRL_TOLERANCE_S` | Two weeks. [CAMP122] Table 67 overdue_CRL_tolerance |
| `p2pcd_max_response_backoff_ms` | 250 | ms | CITED | `p2pcd.rs` `P2pcdParams::RECOMMENDED` | IEEE 1609.2a-2017 Annex C.2.1.3.1 |
| `p2pcd_response_count_threshold` | 3 | - | CITED | `p2pcd.rs` | 1609.2a Annex C.2.1.3.1 |
| `p2pcd_max_active_responses` | 8 | - | ASSUMPTION | `p2pcd.rs` | The standard says "implementation-specific" (§8.2.4.2.2, PICS A.2.3.3) |
| `rxval_ee_cache` | 4096 | - | ASSUMPTION | `rxval.rs` `EE_CACHE` | A bounded verdict cache. An implementation choice that has no protocol effect |

---

## 3. Topology, governance and certificates (listed; pinned by their own tests)

| Name | Default | Unit | Status | Where | Source or reason |
|---|---|---|---|---|---|
| `us_electors` | 5 | - | CITED | `deployment.rs` `US_ELECTORS` | Elector Tech Spec §2.1 |
| `us_quorum` | 3 | - | CITED | `deployment.rs` `US_QUORUM` | Elector Tech Spec v1.1 §2.4 |
| `elector_years` | `12,15,18,21,24` | y | CITED | `governance.rs` `ELECTOR_YEARS` | Elector Policy v1.0 §2.3.1 ("the first 5 Electors' validity shall be staggered … 12, 15, 18, 21 and 24 years"); the published production and test elector certificates decode to exactly these |
| `elector_default_years` | 15 | y | CITED | `governance.rs` `ELECTOR_DEFAULT_YEARS` | Elector Policy v1.0 §2.3.1: "By default, an Elector certificate shall be valid for 15 years". Every replacement elector takes it. It took the fifth elector's 24 y until the governance validation |
| `ctl_add_root_business_days` | 3 | business day | CITED | `governance.rs` `CTL_ADD_ROOT_BUSINESS_DAYS` | Provider Req. v1.1 §3.4.11 ("published by the Provider within three (3) business days"). Counted on the Eastern date, weekends and federal holidays skipped (`calendar.rs`) |
| `ctl_add_elector_business_days` | 5 | business day | CITED | `governance.rs` `CTL_ADD_ELECTOR_BUSINESS_DAYS` | Elector Policy v1.0 §2.2.5, §2.4.2: an elector has up to 5 business days for an addition ballot. An elector replacement adds one. It used the root rule's 3 days until the governance validation |
| `ctl_genesis_sequence` | `auto` | - | CITED | `governance.rs` `genesis_ctl_sequence`, `PRODUCTION_CTL_SEQUENCE_1_FROM_UNIX_S`; `params.rs` `ctl_genesis_sequence` | The genesis CTL's sequence: 1 when `t0` is on or after 2025-07-16 00:00 UTC, 0 before, as the decoded production series (sequence 0 of 2019-12-12; sequence 1 effective 2025-07-16 08:00 UTC, published 2025-08-13). `Some(n)` in `ScmsParams` states it; the scenario key takes `auto`, `0` or `1` |
| `la_ids` | `4,5` (ISS); `1,2` (others) | - | CITED | `governance.rs` `la_ids`; `run/mod.rs` `LaState`; `pdu.rs` LA certificates | The `laId` of a provider's LA1 and LA2. ISS's pre-production LA certificates carry `LaSsp` version 1 with 4 and 5 (decoded, SCMS Manager 2025-07-17 test chain file). No other US provider has published an LA certificate; `laId` is provider-local, so they keep 1 and 2 |
| `ctl_removal_carries_root` | `once` | - | CITED | `governance.rs` `build_ctl` (`removed_root`), `remove_root` | The CTL that removes a root carries the dropped root's certificate in `unsigned` once, beside the approved ones, with `rootCaRemove` empty; later CTLs carry the approved roots only. Seen in the test CTL only (2025-07-17, sequence 3, the Eviden root); production practice unknown (no production CTL has removed a root) |
| `ctl_removal_hours` | 48 | h | CITED | `governance.rs` `CTL_REMOVAL_HOURS` | Elector Policy v1.0 §2.2.4 ("executed within as little as 48 hours of receipt of the task order"; §2.4.4 for a root) |
| `review_add_business_days` | 10 | business day | CITED | `governance.rs` `REVIEW_ADD_BUSINESS_DAYS`, `review_vote_at` | Elector Policy v1.0 §2.4.2: the BOD reviews an addition within 10 business days before the EAC votes. Counted as the publication deadlines are. The vote is placed at the end of the window (when within it the Board finishes is unpublished). Used only under `eac_review: elector-policy` |
| `review_removal_hours` | 24 | h | CITED | `governance.rs` `REVIEW_REMOVAL_HOURS` | Elector Policy v1.0 §2.4.4–2.4.5: a removal is investigated within 24 h before the vote |
| `root_termination_notice_days` | 365 | d | CITED | `governance.rs` `ROOT_TERMINATION_NOTICE_DAYS`, `termination_notice`, `remove_root` | [SM-PROV] §3.5.6: a Root CA's termination needs 365 days' notice. A noticed root cannot be removed earlier, emergency or not (the override is unpublished, an ASSUMPTION); the subscriber-majority transfer vote is not modelled |
| `test_ballot_max_idle_days` | 365 | d | CITED | `governance.rs` `TEST_BALLOT_MAX_IDLE_DAYS`, `test_ballot_due` | Elector Policy v1.0 §1.1: a Test Ballot at least yearly, so no elector is idle for more than a year. The engine's interval key (off by default) is D1's |
| `business_day_holidays` | `5 USC 6103` | - | ASSUMPTION | `calendar.rs` `federal_holidays` | SCMS Manager LLC and the providers do not publish their business-day calendar. The engine uses the US federal holidays with the E.O. 11582 observed-date rule, on the Eastern civil date |
| `elector_start_time32` | 503136005 | s | CITED | `governance.rs` | Published elector certificates (2019-12-11) |
| `component_lifetimes` | `ROLE_SPECS` | y | CITED | `governance.rs` `ROLE_SPECS` | Each comes from a published production or test certificate named in its `source`. DCM, LOP and DC take the RA's 6 y as an ASSUMPTION, because none of them has a published certificate. The root's 30 y is the SAESOL production root's; Provider Req. v1.1 §3.6.4 requires 70 y (20 y key), which the other production root (`v2xrootca.ghsiss.com`, 2016-03-22, 70 y) has. One profile serves every provider |
| `genesis_backdate_s` | 31556952 | s | ASSUMPTION | `governance.rs` | One 1609.2 year. A deployed hierarchy predates any run, so the current week's pseudonyms must fall inside their issuers' validity |
| `default_t0_unix_s` | 1767690000 | s | DERIVED | `governance.rs` `DEFAULT_T0_UNIX_S` | The start of US i-period 574, used only when a scenario gives no `time.t0` |
| `provider_regions` | `REGIONS` | - | ASSUMPTION | `deployment.rs` | No provider publishes where it runs. Each provider is placed in a representative public-cloud metro |
| `fibre_fraction` | 0.6667 | c | ASSUMPTION | `deployment.rs` `LatencyModel::DEFAULT_REGION` | 2/3 c, the usual figure for silica fibre (n ≈ 1.47). The radio crate's 4.9 µs/km corresponds to 0.68 c |
| `per_link_overhead_ms` | 1 | ms | ASSUMPTION | `deployment.rs` | Routing, switching and TLS record processing. A placeholder |
| `fleet_share` | preset | - | ASSUMPTION | `deployment.rs` | No market share is published |
| `appcert_backhaul_latency_ms` | 10 | ms | ASSUMPTION | `run/appcert.rs` `RSU_BACKHAUL_LATENCY` | The library default when a driver gives no backhaul (`ScmsRun::add_rsu`); equal to the engine's `BACKHAUL_LATENCY_MS` (checked by `scms_constants`). In the engine the RSU's own backhaul line carries every message of its certificate cycle (`ScmsRun::add_rsu_with` and the carrier), so the medium model (`backhaul/fibre`, `dsl`, `microwave`, `cellular`) decides |
| `appcert_backhaul_capacity_mbps` | 1000 | Mbit/s | ASSUMPTION | `run/appcert.rs` `RSU_BACKHAUL_BPS` | As `backhaul_capacity_mbps`: a gigabit Ethernet port, [RSU41] §2.1 gives the interface and no rate |
| `appcert_id_period_s` | 2592000 | s | ASSUMPTION | `run/appcert.rs` | CAMP says "1 month", taken here as 30 days |
| `appcert_max_gen_ahead_s` | 604800 | s | CITED | `run/appcert.rs` | ra-maxGenDelay 1 week, [SMP] Table 3 |
| `appcert_retry_wait_s` | 3600 | s | CITED | `run/appcert.rs` | ra-minWait 1 h, [SMP] Table 3 |
| `threshold_round_timeout_ms` | 500 | ms | ASSUMPTION | `params.rs` `ThresholdDeployment` | Research option (FROST). No deployment publishes a value |
| `threshold_member_link_latency_ms` | 20 | ms | ASSUMPTION | `params.rs` `ThresholdDeployment` | WAN latency between member organisations. Not published |
| `ma_share_envelope` | `1609.2 encryption to the peer MA` | - | ASSUMPTION | `mbr.rs` `reencrypt_for_ma` | No published specification defines the MA-to-MA sharing format. The forwarding MA re-encrypts the reporter-signed inner SPDU it opened, byte for byte, to the receiving provider's MA certificate as a device encrypts its report ([SM-MBR] §3, without the signing step and with no `Download-Req` header), so the receiving MA verifies the reporter's signature and the evidence itself, and the link carries the real ciphertext length. Draws come from the run's own `ma-share` stream |

---

## 4. Known limitations

1. **Real artefacts would replace these assumptions.** A provider's TLS chain, an RA acknowledgement and a batch `.zip` would turn `repo_url_bytes`, `batch_container_bytes` and the TLS chain into measurements. The SCMS Manager publications carry no such samples.
2. **MA review time and capacity.** No public figure exists. They stay ASSUMPTIONs that can be swept.
3. **TLS 1.3 client Finished.** These 80 B ride with the first request and are not counted in the handshake bytes. The undercount is small and known.
4. **Report sizes for other evidence.** `report_bytes` assumes two BSMs. An Implausible report with one BSM is 918 B as measured (603 + 315), and a real report is always charged its own length.
