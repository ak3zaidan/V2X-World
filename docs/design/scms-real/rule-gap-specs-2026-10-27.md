# Rule-gap specs: the 51 SCMS rules not yet at "matches" (2026-10-27)

**Summary**
1. Scope: the 51 of 326 rules that are not "matches" or "fixed" in `06-validation-summary.md` §2.1: 22 documented deviations (§5) and 29 unverifiable rows (§7), at base `4aa798bb`.
2. Classes: **10 implementable** from public sources, **8 partly** implementable (a public clause, with one unpublished choice labelled ASSUMPTION), **10 decisions or re-labels** that need no code, and **23 not observable or unpublished**.
3. Highest impact: the WSA encoder plus report uploads on the advertised service channel (veh U5 + M11). WP-13's channel map makes M11 reachable today for two-radio units. Single-radio OBUs need WP-16.
4. Next: the 1609.2.1 `infoStatus` SPDU (proto A16), and HTTP/TLS framing on every EE–RA exchange (proto B9, from CAMP's Table 79 paths).
5. Next: the CTL artefact bytes, which can be checked against the published production series: `effectiveDate` = ballot date (gov C-9), the genesis sequence (C-10), and the dropped root carried (C-11). Then the ISS `LaSsp` (L-11).
6. Then the vehicle-side rules: the 2 km measured as straight-line distance (veh C6) and country-region checks (veh R10).
7. Long-run governance: review delays (G-2), the yearly Test Ballot (G-10) and 365-day root termination (L-8). Small and independent of each other.
8. Seven no-code closures can be made on evidence already in the repo: #35/E7 and #32 become matches, #38/R11 and #30 already behave as deployed, and C13 follows the production standard. Two need the owner: CF-IPERIOD (the evidence favours the current code, §3) and M13.
9. Total estimated size of the 18 specs: 11 S, 2 S–M and 5 M. Single-radio M11 adds one L, which is WP-16. No spec needs a source that is not already cited in this repo or linked here.
10. None of this was built or run (doc-only track, no cargo). Every test below is named with the fault that must turn it red, per the "checks that cannot fail" rule.

Sources, keyed as in `03-protocols-and-operations.md` §0 and `validation/governance-deployment.md` "How it was checked": [SMP] SCMS Manager 1609.2.1 Profile v1.0.3; [SM-MBR] SCMS Manager MBR spec v1.0; [SM-PROV] Provider Requirements v1.1; Elector Policy v1.0 (2019); Elector Technical Specification v1.1 ("Tech Spec"); EAC Requirements v1.0; [CAMP122] CAMP SCMS PoC EE Req. 1.2.2; [ASN-2.1] the 1609.2.1-2022 ASN.1 modules (cached at `tools/oracle/asn1/ieee1609.2.1-2022/`); [PRIMER] FHWA-JPO-19-775; [CVP-P2] USDOT CV Pilot lessons learned (2018); IEEE 1609.3 ASN.1 via the TCI_ASN1 mirror (`docs/design/research/R4-messages-envelopes.md` l.146, l.394).

## 1. Classification of all 51

I = implementable, P = partly, D = decision / re-label only, N = not observable or unpublished.

| Row | Was | Class | Spec |
|---|---|---|---|
| veh U5 | dev | I | §2.1 |
| veh M11 | dev | I (S with 2 radios; L with 1) | §2.2 |
| proto A16 | dev | P (format public; whether devices poll it is not) | §2.3 |
| proto B9 | unv | P (CAMP paths public; 1609.2.1 paths UNCONFIRMED, 03 §4.1) | §2.4 |
| gov C-9 | dev | I | §2.5 |
| gov C-10 | dev | I | §2.6 |
| gov C-11 | dev | P (seen in the test CTL only) | §2.7 |
| gov L-11 | dev | I | §2.8 |
| veh C6 | dev | I | §2.9 |
| gov G-2 | dev | I | §2.10 |
| gov L-8 | dev | I | §2.11 |
| gov G-10 | dev | I | §2.12 |
| veh R10 | dev | I | §2.13 |
| proto H10 | dev | P (the Uint8 is public; the overflow behaviour is not) | §2.14 |
| proto D11 | unv | P (the values are public in [SMP] Table 3; their semantics are in paywalled tables) | §2.15 |
| proto B5 | unv | P (the bound is public; no deployed figure exists) | §2.16 |
| gov G-11 | dev | P (dual signatures are public; no PQ method is chosen) | §2.17 |
| proto F9 | unv | P (the US uses SM-MBR, which the engine already does; TS 103 759 is EU only) | §2.18 |
| crypto #10, gov K-5 | dev | D | §3 |
| crypto #35, proto E7 | dev | D: re-label as a match | §3 |
| crypto #38, veh R11 | dev | D: re-label as a match | §3 |
| crypto #30 | dev | D: re-label as a match | §3 |
| crypto #32 | unv | D: re-label as a match | §3 |
| proto C13 | unv | D: re-label as a match | §3 |
| veh M13 | unv | D (owner) | §3 |
| gov C-14 | dev | N (off-network) | §4 |
| crypto #26, #59, #71, #73 | unv | N | §4 |
| proto D17, H8 | unv | N | §4 |
| gov G-1, C-4, D-9, D-10, D-11, E-2 | unv | N | §4 |
| veh S1, S6, P7, P8, C10, C11, M9, M10, V3, U3 | unv | N | §4 |

## 2. Implementation specs, ordered by realism impact

Each spec gives the source, the observable behaviour, where it goes, the evidence, the test with its fault, and a size. "New" marks a name that does not exist yet.

### 2.1 veh U5: a WSA encoder (I, M)
- **Source:** IEEE 1609.3 ASN.1 module `IEEE-1609-3-WSA`. Its structure: `SrvAdvMsg {version, body}`, `ServiceInfo {serviceID, channelIndex, chOptions}`, `ChannelInfo {operatingClass, channelNumber, powerLevel, dataRate, extensions}` and `RoutingAdvertisement {lifetime, ipPrefix, ipPrefixLength, defaultGateway, primaryDns}`. The engine's `research/R4` l.146 verified this against the TCI_ASN1 mirror. The PSID is 135 (`v2xw-msg/src/registry.rs:92`).
- **Behaviour:** an RSU with role `WsaBroadcast` sends a real encoded WSA. It advertises one ServiceInfo for SCMS connectivity, a ChannelInfo for the WP-13 service channel, and a RoutingAdvertisement (an IPv6 prefix and its lifetime). The frame stops being counted as unsized.
- **Where:** a new `crates/v2xw-msg/src/wsa.rs`, with its codec arm next to `MsgType::Wsa` in `codec.rs`. `v2xw-node/src/rsu.rs` `payload_length` drops the `wsa_payload_bytes` special case (l.1784), and the engine hands the bytes in through `RsuRuntime::set_payload`. Copy `wsa.asn`/`wee.asn` into `tools/oracle/asn1/ieee1609.3/` and record their sha256 in `vectors-provenance.md`.
- **ASSUMPTIONs to label:** the service PSID advertised for "SCMS connectivity" (no source names one; 35, the 1609.2.1 certificate-management PSID of 03 §3, is the candidate). The repeat rate is "1 per 5 s", which 04-models l.1001 already marks UNVERIFIED. The encoding rule must be read from the module header before coding.
- **Evidence:** WSA bytes in the air records; `RsuRuntime::modelled_broadcasts` no longer counts WSA. The oracle decodes the recorded WSA with asn1tools against the cached module.
- **Test:** `rsu_runtime.rs`: replace `spat_and_map_are_sized_by_the_size_model_and_a_wsa_is_not` with `a_wsa_is_encoded_and_decodes_per_the_1609_3_module`. **Fault:** a wrong `channelIndex`, or a dropped RoutingAdvertisement. The oracle decode or the field check must go red.
- **Sources resolved (2026-10-27, sources track; full record in `third_party/asn1/ieee1609.3/PROVENANCE.md`):**
  - **Modules cached.** `third_party/asn1/ieee1609.3/` holds `wsa.asn`, `wee.asn`, `wsm.asn` and the ISO 17419 `CITSapplMgmtIDs` module. WSA imports `VarLengthNumber` from the ISO module, so WSA does not compile without it. They come from TCI_ASN1 commit `63d82d87976b` (2017-05-21), byte-identical, with sha256 recorded. **No licence is stated** upstream; they are treated like the IEEE 1609.2/1609.2.1 copies. The oracle copy under `tools/oracle/asn1/ieee1609.3/` and the `vectors-provenance.md` row are still owed by the build piece. They are the same four files.
  - **Encoding rule: UPER.** All three IEEE modules declare `AUTOMATIC TAGS`. Their header comment says the "mapping of UPER on the intended binary presentation … was validated with the OSS ASN.1 Studio tool". `wsm.asn` adds that `VarLengthNumber` "is the p-encoded PSID". Cross-checked with asn1tools 0.169 (UPER): a sample WSA round-trips, and its octets match the USDOT RSU Spec v4.1 Appendix D example (pp. 118–120): `0x3F`/`0x11` header, `0x09`/`0x11` channel-index octets, PSID `ef ff ff fe`, data rate `0x0C`. The one exception is App. D's power octet `0x14`: UPER's +128 offset gives `0x94`, and the ASN.1 governs. asn1tools' OER codec cannot compile `wee.asn`, which does not matter since OER is not the rule. Edition: WSA protocol version 3 = IEEE 1609.3-2016, the edition RSU Spec v4.1 Req_586 cites. Whether 2020 changed it is UNVERIFIED (paywalled).
  - **PSID to advertise.** Pick **0x23 (35)**. The IEEE RA PSID registry lists 0x23 as "WAVE security management", IEEE Std 1609.2. The 1609.2.1 module `Ieee1609Dot2Dot1Protocol.asn` l.143–146 defines `SecurityMgmtPsid ::= Psid (35)`, which "identifies security management activities". No public deployment document names the PSID a US RSU advertises for SCMS access. FHWA-JPO-20-804 p.4 says only that NYC's SCMS access "is advertised through the RSU using … WSAs", and p.15 says WSA practice varied by vendor. So 0x23 stays labelled ASSUMPTION (registry-based). **Add a second ServiceInfo: IPv6 Routing, PSID 0x1020407E** (p-encoded `0xEFFFFFFE`). It is IEEE-registered to 1609.3, and RSU Spec v4.1 App. D advertises it, on its own service channel, next to the WRA. A WSA carrying a RoutingAdvertisement without it would not match the reference example. The WSA's own PSID 0x87 (135, `registry.rs:92`) agrees with the registry ("WAVE Service Advertisement", 0p80-07).
  - **Repeat rate.** RSU Spec v4.1 App. D gives RepeatRate "32 … the WSA is repeated 50 times per 5 seconds" (0x32 = 50). That confirms the unit, **WSAs per 5 s**, from a public USDOT source. Its range test transmits WSAs at 10 Hz. The engine's "1 per 5 s" (04-models l.1001) is therefore a configured value, not the reference one. The default should become 50 per 5 s (10 Hz), with the current value kept as an option. That changes CCH load, so it moves goldens and lands as its own listed change.
  - **Placement.** Req_587 puts WSAs on the CCH and Req_588 in time slot 0. Req_586 requires the 1609.3-2016 Annex H.1 security profile, so the WSA is a signed `Ieee1609Dot2Data`.

### 2.2 veh M11: report uploads on the advertised service channel (I; S with two radios, L with one)
- **Source:** [SM-MBR] §4: a device uploads "whenever in range of an RSU offering SCMS connectivity". IEEE 1609.4-2016 §6.2.5 allows services on a service channel (cited in `run/channels.rs`).
- **Behaviour:** a vehicle relays a report only after it has heard a WSA (§2.1) that advertises SCMS connectivity and whose lifetime is still open. The upload and its acknowledgement go out on the advertised service channel, so report airtime moves off channel 172. On the 2026-10 baseline that was 6.89 s of 23.52 s of airtime, 0.21 % at full fleet.
- **Where:**
  - `v2xw-engine/src/run.rs` `relay_report` (l.9329): gate on a new `heard_scms_wsa(node, now)` instead of `phase2.relay_in_range(pos, "report-forward")` alone.
  - The channel comes from `run/channels.rs` `ChannelPlan::channel_for(MsgType::Mbr)`: frames already take their channel from `channel_for` at run.rs l.5490/5846. Add `mbr` to the US SCMS profile's default `radio.channels` entry once U5 exists.
  - With `radios: 1` (the OBU default), the upload needs IEEE 1609.4 alternating access: WP-16, size L.
- **Evidence:** `channel_busy_ns` per channel (ch 172 report share → 0), `reports_uploaded_relay` unchanged on the same seed, and a new `relay_without_wsa` = 0.
- **Test:** `tests/multichannel.rs` new `report_uploads_move_to_the_wsa_service_channel`. Same seed, with and without the map: the CBR172 drop equals the relay airtime share within 5 %. **Fault:** remove the WSA gate, then upload through an RSU that does not advertise. `relay_without_wsa` > 0 must turn the test red.

### 2.3 proto A16: the `infoStatus` SPDU (P, M)
- **Source:** [ASN-2.1] `Ieee1609Dot2Dot1CertManagement.asn` l.341 defines `CertificateManagementInfoStatus {crl SEQ OF {cracaId, series, issueDate}, ctl SEQ OF {ctlSeriesId, sequenceNumber, lastUpdate}, caCcf Time32, ma SEQ OF {psids, updated}, ra Time32 OPT}`. 03 §3 l.111 records it as signed, PSID 35, by the RA or DC certificate. [SMP] §4.4 profiles 6.3.5.14 with If-Modified-Since (03 §4.1 table).
- **Behaviour (option `security.scms.status_poll: composite | info-status`, new):**
  - Under `info-status`, the hourly poll (`CRL_FETCH_INTERVAL_S`) asks for infoStatus with If-Modified-Since. It is answered 412 when the copy is current, or with the signed status otherwise.
  - The device then fetches only what changed: the composite CRL (6.3.5.8) when a `(cracaId, series)` `issueDate` is newer; the chain file (6.3.5.7) when `ctl.sequenceNumber` or `caCcf` moved; the RA or MA certificate when `ra` or `ma.updated` moved.
  - Default: `composite`, today's behaviour. No source says deployed devices poll infoStatus first.
- **Where:**
  - `v2xw-engine/src/phase2.rs` `crl_polls_due` (l.7313), `phase_crl_poll` (l.7404) and `crl_poll_failed` (l.7500).
  - The encoder sits beside the composite CRL builder in `v2xw-proto/src/scms/pdu.rs`. It is signed with the RA key the composite answer already uses.
- **Evidence:** a `pki.flow` kind `info-status` with real bytes, new `info_status_polls`/`info_status_changed`, the existing `crl_polls_not_modified`, and an oracle decode in `check_run.py` against the cached module.
- **Test:** `tests/report_signing.rs` new `an_info_status_poll_fetches_the_composite_only_after_a_change`: one revocation gives exactly 2 exchanges, a quiet hour gives 1 (a 412). **Fault:** the device ignores `issueDate` and always fetches the composite. The quiet-hour count becomes 2 and the test goes red.

### 2.4 proto B9: HTTP request framing on every EE–RA exchange (P, S–M)
- **Source:** [CAMP122] §6.2–6.3, Table 79 (03 §4.2): methods, paths and ports, for example `POST /provision-pseudonym-certificate-batch`, `GET /download/batch` with a Base64 `Download-Req` header, `GET /download/info`, and `https://<MA>:8894/download-crl/`. TLS record framing: RFC 8446 §5.2, a 5-byte record header plus the AEAD expansion.
- **Behaviour:** each request is charged its request line, a minimal header set and the Base64 expansion (4/3) of any `Download-Req`. Each answer is charged its status line and headers, like `CRL_NOT_MODIFIED_BYTES` (phase2.rs l.420) already is. TLS records are charged once, by whichever layer owns TLS.
- **Where:** a new `v2xw-proto/src/scms/http.rs` `fn request_bytes(method, path, headers, body_len)`, used by the flow builders in `scms/run/provision.rs`, `scms/run/enrol.rs` and `phase2.rs` (polls). The upload already counts its 315 B `Download-Req` (`scms/mbr.rs` l.94), so reuse that and do not count it twice. Check the `net.backhaul` TLS model before adding record overhead.
- **ASSUMPTION:** the header set (Host, Content-Type, Content-Length) and the path strings for the 1609.2.1 flows, which are UNCONFIRMED (03 §4.1). Label both in `constants.md`.
- **Evidence:** `net.bytes` per flow kind, and the request-bytes column of the conformance run.
- **Test:** `v2xw-proto/tests/` new `http_framing.rs`: a download request is charged its Table 79 path length + header + ⌈4/3·n⌉. **Fault:** drop the Base64 expansion. The byte assert goes red.

### 2.5 gov C-9: CTL `effectiveDate` is the ballot date (I, S)
- **Source:** the decoded production CTL. Sequence 1 is effective 2025-07-16 08:00 UTC and was published 2025-08-13. The test sequences were published 0–4 days after their effective dates (`validation/governance-deployment.md` C-9).
- **Behaviour:** `effectiveDate` = Time32 of the vote (`decided_at`). Publication stays at the deadline.
- **Where:** `v2xw-proto/src/scms/governance.rs` `decide` (l.2256). Change `effective` from `time32(at)` to `time32(decided_at)`. This moves every SCMS digest, so re-bless them.
- **Evidence:** the CTL records' `effectiveDate` ≤ `published_at` in `Governance::log`.
- **Test:** `governance/tests.rs` new `a_ctl_is_effective_at_its_vote_and_published_by_the_deadline`. **Fault:** restore `time32(at)`. `effectiveDate` > vote turns it red.

### 2.6 gov C-10: the genesis CTL sequence (I, S)
- **Source:** the live production series: sequence 0 on 2019-12-12 with one root, then sequence 1 effective 2025-07-16 with two roots (C-10).
- **Behaviour:** a run whose `t0` is on or after 2025-07-16 starts at sequence 1. An earlier `t0` starts at 0.
- **Where:** the genesis path of `governance.rs` (`CtlChangeKind::Genesis`, `next_sequence` l.2247). Take the date rule from the real calendar (`scms/calendar.rs`), and add a new key `governance.ctl_genesis_sequence` to override it.
- **Test:** a 2026 run's first CTL is sequence 1, and a 2024 run's is 0. **Fault:** hard-code 0. The 2026 assert goes red. Re-bless the digests.

### 2.7 gov C-11: a CTL that drops a root still carries it (P, S)
- **Source:** the decoded test CTL package, sequence 3, carries the Eviden root that it no longer approves (C-11). The production practice is unknown, because no production CTL has dropped a root yet.
- **Behaviour:** in the CTL that removes a root, `root_certs` = the approved roots plus that removed root. The certificate is not repeated in later sequences.
- **Where:** `governance.rs` `build_ctl` (l.1985, the `root_certs` list), given the removed root by `remove_root` (l.2320).
- **Test:** decode the removal CTL: certificates = approve entries + 1; the next CTL: certificates = approve entries. **Fault:** no carry. The +1 assert goes red.

### 2.8 gov L-11: the ISS `LaSsp` (I, S)
- **Source:** the decoded ISS LA certificates carry `LaSsp` version 1 with `laId` 4 and 5 (L-11).
- **Behaviour:** the ISS-profile provider's LA1 and LA2 carry version 1 with ids 4 and 5. Other providers keep their own values.
- **Where:** `v2xw-proto/src/scms/pdu.rs` l.816/826 (`LaSsp::new(Uint8(2), Uint16(1|2))`), made per-provider. The `la_id` in `scms/msg.rs` l.298 and the CRL `LAGroup` ids must follow.
- **Risk:** the 2022 module may constrain the version to 2. If `check_*` refuses version 1, encode through the 2020 edition's type. Do not drop the constraint check.
- **Test:** the oracle compares the LA certificate's SSP bytes with the decoded ISS ones, and a linkage CRL entry carries `la1Id` 4. **Fault:** version 2. The byte compare goes red.

### 2.9 veh C6: 2 km as straight-line distance from the last change point (I, S)
- **Source:** [CVP-P2] item 25 and [PRIMER] pp.8–9: change after 5 min **and** ≥ 2 km "from the last change point" (veh C4, C6).
- **Behaviour:** `RotationPolicy::is_due` gets the displacement from the position at the last change, not the distance travelled. Travelled distance stays as a reported figure.
- **Where:** `v2xw-node/src/stores.rs`. Add a change point next to `distance_since_change_m` (l.168), reset where l.296/348 reset it, and a new `CertStore::position(x, y)` called from `runtime.rs` l.1168 and `vru.rs` l.1501. `is_due` (l.143) then takes the displacement.
- **Evidence:** pseudonym change counts and the PT30 change rate. Same seed: changes are equal or later on winding routes.
- **Test:** `stores.rs` new `a_loop_back_to_the_change_point_is_not_far_enough`: 2.5 km travelled with 0.3 km displacement gives no change under `J2945_1`. **Fault:** pass the travelled distance. A change happens and the test goes red.

### 2.10 gov G-2: board review and investigation delays (I, S)
- **Source:** Elector Policy §2.4.2: the BOD reviews an addition within 10 business days. §2.4.4–2.4.5: it investigates a removal within 24 h before voting.
- **Behaviour:** a scenario may give a request instant (new `governance.*.requested_at`). The vote then falls at the request plus the review: 10 business days on the US federal calendar for an addition, 24 h for a removal. Without a request instant, today's "decision instant" input stays.
- **Where:** `v2xw-engine/src/phase2.rs` `governance_due` (l.4935), and `scms/calendar.rs` `is_business_day` (l.185).
- **Evidence:** `Governance::log`: a new `eac-request` event, then `eac-add-root` 10 business days later.
- **Test:** a request on the Friday before a federal holiday votes on the correct business day. **Fault:** a delay of 0. The date assert goes red.

### 2.11 gov L-8: root termination with 365 days' notice (I, S–M)
- **Source:** [SM-PROV] §3.5.6: a root's termination needs 365 days' notice, and a transfer needs a subscriber majority.
- **Behaviour:**
  - A new `governance.root_termination_notice_at` logs `provider-termination-notice`. `remove_root` for that root is refused before notice + 365 d, and counted in a new `early_root_removal_refused`.
  - The transfer vote is not modelled. Subscriber votes are unpublished, the same as G-1.
- **Where:** `governance.rs` `remove_root` (l.2320) and `phase2.rs` `governance_due`. Runs past a year use fast-forward.
- **Test:** a removal at notice + 200 d is refused, and one at notice + 366 d publishes. **Fault:** skip the check. The refusal assert goes red.

### 2.12 gov G-10: the yearly Test Ballot (I, S)
- **Source:** Elector Policy §1.1: a Test Ballot at least yearly, so no elector is idle for more than a year.
- **Behaviour:** the engine calls `Governance::test_ballot` (l.2466) once no elector has signed for a new `governance.test_ballot_interval_days`, default 365.
- **Where:** `phase2.rs` `governance_due`, keyed on the last signature time in `Governance::log`.
- **Evidence:** `eac-test-ballot` entries in `Governance::log`, and `gov_decisions`.
- **Test:** a 400-day fast-forward run has at least one ballot, and every elector has signed within 365 d. **Fault:** the scheduler is off. Zero ballots turns it red.

### 2.13 veh R10: country regions checked against the location (I, M)
- **Source:** the receiver region-consistency check of 1609.2 §5.1.2/§5.2, as the checklist cites them (`validation/vehicle-and-roadside.md` §1.1). The country polygon comes from the US Census Bureau cartographic boundary files, which are public domain.
- **Behaviour:** `identifiedRegion countryOnly(840)` is checked against the generation location: outside it, the message is refused with `OutsideRegion`. `countryAndRegions` uses state polygons. Anything else stays `region_unchecked`.
- **Where:** `v2xw-node/src/rxval.rs` `location_in` (l.625), the `_ => None` arm. The polygon is compiled in (`include_bytes!`), because `v2xw-node` cannot read the world (`crate::firewall`). Simplify it to about 2k vertices.
- **Impact:** low. US pseudonyms carry no region, so `region_unchecked` is already 0 on the benchmark. It matters for component or app certificates with regions, and for cross-border attack scenarios.
- **Test:** `tests/rx_validation.rs` new `a_us_country_certificate_is_refused_in_toronto`. **Fault:** the arm returns `None`. `region_unchecked` rises instead and the test goes red.

### 2.14 proto H10: `indexWithinI` exhaustion (P, S)
- **Source:** `Ieee1609Dot2CrlBaseTypes` (cached, `tools/oracle/asn1/ieee1609.2-2022/`): `indexWithinI` is a Uint8. What a CRL Generator does at 256 is unpublished.
- **Behaviour (ASSUMPTION, labelled):** once index 255 is used at an `iRev`, further publications in that i-period are coalesced into one pending CRL. It is issued at the next i-period with index 0 and counted in a new `crl_index_exhausted`, and the SLA check of `run/crl.rs` `business_day_sla` records any breach.
- **Where:** `v2xw-proto/src/scms/run/crl.rs` `issue_secured_crls` (l.309; `next_index` l.330 saturates today).
- **Test:** `tests/crl_issuance.rs`: 300 publish-on-decision revocations in one week give no two CRLs with equal `(iRev, indexWithinI)`. **Fault:** today's saturation. Duplicate 255s turn it red.

### 2.15 proto D11: the `*-maxAge` values, under the request-age reading (P, S)
- **Source:** [SMP] Table 3: `download-maxAge` 1 h, `ra-maxAge` 1 week. Their semantics are in the paywalled 1609.2.1 Tables 5–7 (03 §6.2).
- **Behaviour (reading labelled ASSUMPTION):** the RA refuses a `Download-Req` whose `AuthenticatedDownloadRequest.time` ([CAMP122] Table 79, 03 §4.2) is more than 1 h old, and counts it in a new `download_req_too_old`. Honest devices never trip it. A replayed header does.
- **Where:** the RA download handler in `v2xw-proto/src/scms/run/provision.rs`.
- **Test:** a header replayed 2 h later is refused. **Fault:** check disabled. The replay is served and the test goes red.

### 2.16 proto B5: the RA's 1 s acknowledgement bound as a measured SLA (P, S)
- **Source:** [CAMP122] §5.2.8.3 SCMS-521 (ACK within 1 s). SCMS-522/523 (the 10 s retry and the cap) are already built (`v2xw-proto/tests/ee_retry.rs`).
- **Behaviour:** each RA request records its ACK delay, from the `ServiceQueue::admit` result (`v2xw-proto/src/service.rs` l.130) minus the arrival. Report a new `ra_ack_over_1s` count and the p99. Nothing is refused, because no deployed RA figure exists.
- **Test:** an overloaded RA (c = 1, a fleet start) reports `ra_ack_over_1s` > 0, and an idle one reports 0. **Fault:** record arrival as admission. Always 0, so the overload assert goes red.

### 2.17 gov G-11: post-quantum elector replacement by dual signatures (P, M)
- **Source:** Elector Technical Specification v1.1 §2.5: dual signatures during a transition. No quantum-safe method has been chosen.
- **Behaviour:** an option `replace_elector` with `pq: true` signs the transition CTLs with both the old P-256 key and the new PQ key. It uses the engine's hybrid-PQ algorithm, labelled ASSUMPTION.
- **Where:** `governance.rs` `replace_elector` (l.2414) and `build_ctl`.
- **Test:** a device that knows only the old key accepts the transition CTL, and so does a device that knows only the new key. **Fault:** a single signature. The old-key device refuses and the test goes red.

### 2.18 proto F9: TS 103 759 report content for the EU profile (P, M; no US effect)
- **Source:** ETSI TS 103 759 §4–7 (the `EtsiTs103759Data` structure, as cited in `docs/design/05-protocols.md` l.105). ETSI publishes it free. The US deployment uses [SM-MBR], which the engine already does, so the US rows are unchanged.
- **Where:** the `v2xw-threat` report builder, behind a profile switch. This is also the summary's §9 item 10 "TS 103 759 selectable in `detection.local`".
- **Test:** an EU report decodes against the ETSI module. **Fault:** the SM-MBR body under the EU profile. The decode goes red.

## 3. Decisions and re-labels (no code)

- **crypto #10 / gov K-5 (CF-IPERIOD):** CAMP's CV-pilot production table ([CAMP122] Table 5, for example 428 662 805 = 2017-08-01 09:00:00 UTC with the leap seconds counted) is a real deployed artefact, and the engine reproduces it. The 2 s rule (718 966 803) comes from the POC text, §5.1.5.4.2, which leaves leap seconds "out of scope for POC". Recommendation: keep the code, and re-label both rows as "matches the deployed table; deviates from the POC rule". Owner sign-off is needed, because the summary lists it as an owner decision.
- **crypto #35 / proto E7:** re-label as a match. CAMP Table 1 prints no SPaT, MAP or TIM SSP (gov E-8, F-3), and the one printed SSP is pinned byte-exact. The deviations would then be 20 (summary §8 item 1).
- **crypto #32:** re-label as a match. Pseudonyms carry no region under [SMP] §6.3.3 (gov F-2).
- **crypto #38 / veh R11:** keep the behaviour and re-label as a match. It is what deployed devices do, and it is counted.
- **crypto #30:** re-label as a match. It follows [SMP] §6.3.1, the deployed profile, and the bytes are identical.
- **proto C13:** re-label as a match. The engine follows the production online re-enrolment ([SM-REENR]), and the pilots' manual return to a secure environment ([CVP-P2] item 27) is not production practice.
- **veh M13:** owner decision. The default already files no roadside reports (the DETPRIV reading). Accepting that reading makes it a match (summary §8 item 6: 276/326).
- **veh C10:** a text fix only. `vehicle.rs` `set_identity` clears the path history (summary §8 item 4). The rule itself stays unverifiable, because J2945/1 is paywalled.

## 4. Not observable or unpublished (23): why no spec

- **crypto #26:** no DCM certificate is published.
- **crypto #59:** the CCM nonce body text is paywalled. The nonce is random on the air, so there is nothing to compare.
- **crypto #71:** group linkage is off in US practice.
- **crypto #73:** these are NULL interfaces in 1609.2.1, internal to the backend.
- **proto D17:** no first-batch delay is published.
- **proto H8:** the `nextCrl` interval is already a key (`params.crl.next_crl`). It becomes observable only from a decoded real CRL. [CAMP122] notes that the CRL store does not authenticate downloaders, but no production CRL URL appears in the sources read. A fetch-and-decode would settle it.
- **gov G-1:** EAC votes are not published.
- **gov C-4:** no elector removal has been published.
- **gov C-14:** the courier flash drive and the video call happen off the network.
- **gov D-9, D-10, D-11, E-2:** data-centre regions, latency, fleet shares and the 6-year encoding are unpublished. D-10 is physics-based.
- **veh S1, U3:** deployed verification policies are not published.
- **veh S6:** this is a measurement, not a rule. It sits inside the sourced 5–30 % band.
- **veh P7, P8:** P2PCD limits and flavour are not in any US profile.
- **veh C10, C11, V3:** J2945/1 and J2945/9 are paywalled.
- **veh M9, M10:** [SM-MBR] leaves the quota and the rate device-specific.
