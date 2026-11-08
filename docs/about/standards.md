# Standards and references

The standards, specifications and papers the engine's code cites, with the sections it
cites most and the modules that implement or calibrate against them. The list is built
from the citations in `crates/` (doc comments and model cards); a document the code does
not cite is not listed. The per-parameter source of every model default is on its card,
published on the [Model parameters](../configuration/reference/models.md) page.

!!! note "Licensed text"
    Several of these documents (SAE, IEEE, ETSI before publication, 3GPP drafts) are
    licensed. The engine reproduces field names, identifiers, constants and table values
    needed for interoperability, never the text. Test vectors transcribed from licensed
    documents carry their citation and the source file's hash (`tools/oracle/transcribe`).

## Security and credential management

| Document | Sections the code cites | Implemented in |
|---|---|---|
| IEEE 1609.2-2022 (and -2016, -2025 editions for comparison), *Security Services for Applications and Management Messages* | §5.2 (receiver processing), §5.3.1–§5.3.5, §6.3 (data structures: §6.3.4, §6.3.9, §6.3.26, §6.3.29), §6.4.3, §8 (peer-to-peer certificate distribution) | `v2xw-sec` (`envelope`, `cert`, `chain`, `crl`, `ctl`, `primitive`, `sig1609`, `p2pcd`), `v2xw-core::time` (Time64 and generation time), `v2xw-node` (`runtime`, `secure`, `rxval`, `regions`, `p2pcd`), the ASN.1 bindings in `v2xw-msg` |
| IEEE 1609.2.1-2022, *Certificate Management Interfaces for End Entities* | §6.3.5 (SPDUs, incl. §6.3.5.4, .7, .8, .14), §9.3 (download and provisioning, §9.3.4) | `v2xw-proto::scms`, `v2xw-msg` (generated 1609.2.1 bindings), `v2xw-engine::phase2` |
| IEEE 1609.2a-2017 (amendment) | §6.3.25, §8.2.x (peer-to-peer certificate distribution, §8.2.4), Annex D.6 vectors | `v2xw-sec::p2pcd`, `v2xw-net::frag::cert_cycle`, `v2xw-sec/tests/ieee1609_2a_d6.rs` |
| CAMP VSC5, SCMS Proof-of-Concept Implementation, Release 1.2.2 (2016), and its EE Requirements (2016) | Table 3 (example sizes), Table 67 (CRL handling), §2.1.5.3.2, §5.2.8.3 | `v2xw-proto::scms` (`CampScms`, `params`), `v2xw-sec` (`butterfly`, `linkage`), `v2xw-sec/tests/official_vectors.rs` |
| SCMS Manager, *Misbehavior Report and Application Specification* v1.0 (SM-MBR), with *ASN.1 for Misbehavior Reporting* v1.1 | §2.2, §2.3 (PSID 0x20 report subtypes and thresholds, §2.3.1.x), §3, §4 | `v2xw-threat::us_mbr`, `v2xw-proto::scms::mbr`, `v2xw-engine::phase2` |
| SCMS Manager published CTL, certificate-chain and elector files | | `v2xw-proto::scms::governance`, `v2xw-sec/tests/real_artefacts.rs`, `tools/scms-artefacts` |
| ETSI TS 103 097 (V1.3.1, V2.1.1), *ITS security; header and certificate formats* | §4.1, §5.1–§5.2, §7.1.1–§7.1.2, §7.2.1 | `v2xw-sec::envelope`, `v2xw-msg::sec_types` |
| ETSI TS 102 941 (V2.2.1), *ITS security; trust and privacy management* | §6.1.4, §6.1.6, §6.2.2, §6.2.3.x (enrolment and authorization), §6.3.x (CRL/CTL distribution) | `v2xw-proto::etsi::ts102941` |
| NIST FIPS 186 (ECDSA), FIPS 203 (ML-KEM), FIPS 204 (ML-DSA), FIPS 205 (SLH-DSA) | parameter sets and sizes | `v2xw-sec` (`ec`, `p384`, `primitive`, `scheme::{post_quantum, kem, hybrid}`) |
| NIST CAVP sample vectors (ECDSA P-384, SHA-384, AES-128-CCM, SP 800-135 X9.63 KDF) | | `v2xw-sec/tests/cavp.rs`, `v2xw-sec::ccm` |
| SECG SEC 1 (elliptic curve cryptography) and SEC 4 (ECQV implicit certificates) | | `v2xw-sec` (`ec`, `cert`, `crypto`, `butterfly`) |
| ITU-T X.696 (OER) and X.697 (JER) | X.696 §30 (open types) | the generated bindings in `v2xw-msg`; `v2xw-proto::scms::pki_view` (JER) |
| RFC 8446 (TLS 1.3), RFC 5246 (TLS 1.2) | | `v2xw-radio::ip_transport`, `v2xw-engine::backend` (EE to SCMS sessions) |
| Brecht et al., "A Security Credential Management System for V2X Communications", IEEE T-ITS 2018 | §V, §VI-F (CRL sizes) | `v2xw-proto::scms`, `v2xw-threat::ma_corroborate`, `v2xw-sec/tests/crl_size_brecht.rs` |
| Whyte et al., IEEE VNC 2013 (pseudonym certificate provisioning) | | `v2xw-proto::scms::run::revoke` |

## Radio and access

| Document | Sections the code cites | Implemented in |
|---|---|---|
| IEEE 802.11-2020 (and -2016), OCB operation (802.11p) | §17 (OFDM PHY: §17.3.10.6 receiver sensitivity, §17.4.3 TXTIME), §10.22.2 and §10.23.2 (EDCA), §9 (frame formats) | `v2xw-radio` (`phy`, `mac`, `types`) |
| ETSI EN 302 663 (V1.3.1), *ITS-G5 access layer* | Table 1 (sensitivities), §4.2, §4.3.1 | `v2xw-radio` (`mac`, `types`, `phy`) |
| ETSI TS 102 687 (V1.2.1), *Decentralized Congestion Control* | §5.2–§5.4 (reactive and adaptive DCC, gatekeeper) | `v2xw-radio::dcc` |
| IEEE 1609.4-2016, *Multi-Channel Operation* | §6.2.5 (channel intervals) | `v2xw-radio` (`mac`, `wave_schedule`), `v2xw-engine::run` |
| SAE J2945/1 (2016, 2020), *On-Board System Requirements for V2V Safety Communications* | Table 21 (accuracy limits), §6.3.x (BSM transmission and congestion control), §6.5.2 | `v2xw-radio::dcc` (`SaeJ2945Dcc`), `v2xw-msg::generator` (BSM timing), `v2xw-node::bsm_timing`, scenario presets |
| SAE J3161/1, *LTE-V2X deployment profiles* | §7.3.3 | `v2xw-radio::sidelink`, `v2xw-radio::dcc`, `v2xw-engine::run::sidelink` |
| 3GPP TS 36.213 | §14.1.1.6, §14.1.1.4C (sidelink mode 4 resource selection), §7.1.7.2.1 | `v2xw-radio::{sps, sidelink}` |
| 3GPP TS 36.321 | §5.14.1.1 (SL-SCH, reselection counter) | `v2xw-radio::sps` |
| 3GPP TS 38.214 | §8.1.4 (NR sidelink mode 2 resource selection), §5.1.3.1 (MCS tables) | `v2xw-radio::{sidelink, sps}` |
| 3GPP TS 38.321 | §5.22.1 (SL-SCH) | `v2xw-radio::sps` |
| 3GPP TR 37.885 | §6.1.x (scenarios), §6.2.1 (pathloss, LOS/NLOS/NLOSv) | `v2xw-radio` (`prop::Tr37885`, `obstacle`), `v2xw-mobility::demand::tr36885` |
| 3GPP TR 36.885 | §A.1.4 (shadowing correlation) | `v2xw-radio::winner`, `v2xw-mobility::demand::tr36885`, `v2xw-metrics::comms` |
| 3GPP TS 22.185 | §5.1 (100 ms latency requirement) | `v2xw-metrics::health`, `v2xw-copilot::analyst` |
| FCC 24-123 (C-V2X in 5.895–5.925 GHz) | | `v2xw-radio::regulation` |
| ITU-R P.838-3 (rain), P.833-10 (vegetation), P.2040-3 (building materials) | | `v2xw-radio` (`prop`, `foliage`, `obstacle::reflection`) |
| WINNER+ B1 | | `v2xw-radio::winner` (`propagation/winner-plus-b1`) |
| Abbas et al. 2015 (V2V log-distance with dual slope) | Eq. 4 | `v2xw-radio::prop` (`LogDistanceShadowing`) |
| Sommer et al. 2011 (building obstacle model); Sommer et al. 2012 (two-ray interference) | | `v2xw-radio::obstacle`, `v2xw-radio::prop` |
| Mangel, Klemp & Hartenstein 2011 (urban corner model) | | `v2xw-radio::prop` (`GeometricUrbanV2v`) |
| Karedal et al. 2011; Boban et al. 2014 (GEMV²) | | `v2xw-radio` (`prop`, `obstacle::reflection`, `foliage`) |
| Nakagami 1960; Cheng et al. 2007 | | `v2xw-radio::fading` (`NakagamiFading`) |
| Pei & Henderson; L. E. Miller, NIST 2003 (OFDM BER) | Tables 3.1.1–3.1.3 | `v2xw-radio::per` (`phy/80211p/nist-per`) |
| Lusvarghi et al. 2024; WiLabV2Xsim; Bazzi 2018; Molina-Masegosa & Gozalvez 2017 | | `v2xw-radio::{bler, sidelink}` |
| Torrent-Moreno et al., IEEE TVT 2009 | | `v2xw-metrics::load` |
| Friis; Rappaport, *Wireless Communications* 2nd ed. 2002 | | `v2xw-radio::prop` |

## Network and messages

| Document | Sections the code cites | Implemented in |
|---|---|---|
| IEEE 1609.3-2016 / -2020, *Networking Services* (WSMP, WSA) | §8.2 (WSMP and WSA formats, §8.2.3) | `v2xw-net::{wsmp, netlayer}`, `v2xw-msg::wsa_codec`, `v2xw-node::rsu` |
| IEEE 1609.12 (identifier allocations: PSIDs) | | `v2xw-msg::{wsa, registry}` |
| ETSI EN 302 636-4-1 (GeoNetworking), EN 302 636-5-1 (BTP) | §7.2, §8.1, §9.x (forwarding), §10 | `v2xw-net::{gn, netlayer, stack}` |
| ETSI TS 103 301 (facilities layer protocols) | §5.4.2, §6.4.1 | `v2xw-net::netlayer` |
| SAE J2735 (2016-03, 2024-09), *V2X Communications Message Set Dictionary* | data frames and elements (§6.90, §6.91, §6.93, §7.160, §7.203, §7.213, §7.234) | `v2xw-msg::j2735` (BSM, MAP, SPaT, PSM, SRM, SSM codecs), `v2xw-msg::size_model` |
| SAE J2945/9 (VRU safety messages) | §6 | `v2xw-node::vru` |
| ETSI EN 302 637-2 (CAM) V1.4.1 | §6.1.3 (generation rules) | `v2xw-msg::generator` (`CamGenerator`), `v2xw-node::generate` |
| ETSI TS 103 900 (CAM, Release 2) and TS 103 831 (DENM, Release 2) ASN.1 | the message definitions compiled into the bindings | `v2xw-msg` (`build.rs`, `cam`, `denm`); `third_party/asn1/etsi` |
| ETSI EN 302 637-3 (DENM) V1.3.1 | §6, §8.2.1.5 | `v2xw-msg::denm`, `v2xw-node::events`, `v2xw-net::stack::den` |
| ETSI TS 103 324 (CPM) V2.1.1 | §5.3.3, §6.1.2.x, §7.1.x (generation and segmentation) | `v2xw-msg::cpm`, `v2xw-node::cpm`, `v2xw-net::frag::facilities` |
| ETSI TS 103 300-3 (VAM) | §6.2 | `v2xw-node::vru`, `v2xw-msg::vam` |
| ETSI TS 102 894-2 (common data dictionary) | | `v2xw-msg` (`cam`, `denm`) |
| ETSI TR 103 562 (collective perception analysis) | §3, §5.3.2, §5.5 | `v2xw-msg::etsi_size` |
| C2C-CC triggering conditions and pseudonym profiles | | `v2xw-node::events`, `v2xw-msg::etsi_size`, pseudonym strategies |

## Misbehaviour detection

| Document | Sections the code cites | Implemented in |
|---|---|---|
| ETSI TS 103 759 (V2.1.1, V2.2.1), *Misbehaviour reporting service* | §4 (detection types and classes), §7 | `v2xw-threat::ts103759` (`detect/ts103759`) |
| VeReMi: van der Heijden et al., SecureComm 2018 | Table 3 | `v2xw-record::dataset::veremi`, `v2xw-threat::attack_legacy` |
| VeReMi Extension: Kamel et al., IEEE ICC 2020 | | `v2xw-record::dataset::veremi` (the `veremi` exporter), `tools/realworld/veremi_baseline.py` |
| F2MD: Kamel et al., IEEE WCNC 2019 and IEEE TVT 2020 | | `v2xw-threat::ts103759` (F2MD-style checks), `v2xw-threat::attack_ext`, `v2xw-metrics::detection` |
| Kamel et al. 2019 (CaTch confidence-aware checks) | | `v2xw-threat::{detect, ts103759}` |

## Traffic and mobility

| Document | Cited for | Implemented in |
|---|---|---|
| Treiber, Hennecke & Helbing 2000, *Phys. Rev. E* 62, 1805 (IDM) | the IDM and its freeway calibration (Table 1) | `v2xw-mobility::carfollowing::idm` (`mobility/car-following/idm`), `v2xw-trafficval::model` |
| Kesting, Treiber & Helbing 2010 | the IDM car and truck parameter sets (the medium-tier default drivers) | `v2xw-mobility::carfollowing::idm` (`IdmPreset::Kesting2010`) |
| Kesting, Treiber & Helbing 2007, *TRR* 1999 (MOBIL) | the lane-change criteria and politeness | `v2xw-mobility::lanechange::mobil` |
| Treiber & Kesting 2013, *Traffic Flow Dynamics* | IDM variants, string stability | `v2xw-mobility::carfollowing`, `v2xw-trafficval` |
| Krauß 1998, PhD thesis, Universität zu Köln | the collision-free stochastic model (SUMO's default) | `v2xw-mobility::carfollowing::krauss` |
| Gipps 1981, *Transportation Research Part B* 15(2), eqs. (1)–(5) | the behavioural car-following model | `v2xw-mobility::carfollowing::gipps` |
| Wiedemann 1974; the W99 formulation (PTV Vissim) | the psycho-physical model | `v2xw-mobility::carfollowing::w99` |
| Milanés & Shladover 2014; Ploeg et al. 2011 | ACC and CACC control laws | `v2xw-mobility::carfollowing::{acc, cacc}` |
| Helbing & Molnár 1995 | the social-force pedestrian model | `v2xw-mobility::vru::social_force` |
| Erdmann 2015 (SUMO SL2015 sublane model) | lateral dynamics | `v2xw-mobility::lateral::sl2015` |
| Highway Capacity Manual, 6th ed. (2016), and 7th ed. | saturation flow, lost time, capacity (e.g. Exhibit 12-6), signal timing | `v2xw-mobility::{calibration, intersection::signal_fixed_time, engine}`, `v2xw-trafficval` |
| FHWA *Signal Timing Manual* 2nd ed.; FHWA Road Weather Management | actuated and NEMA dual-ring control; weather speed and headway factors | `v2xw-engine::nema`, `v2xw-mobility::weather` |
| NEMA TS 2-2016 | actuated and dual-ring control, the conflict monitor | `v2xw-engine::{nema, actuated}`, `v2xw-trafficval::nema_safety` |
| Gettman et al. 2008 (SSAM) | surrogate safety thresholds (TTC, PET) | `v2xw-metrics::{ssm, safety}`, `v2xw-node::safety`, `v2xw-trafficval::safety` |
| Edie 1963 | generalised flow, density and speed | `v2xw-metrics::safety`, `v2xw-mobility::fd` |
| Greenshields 1935; Sugiyama et al. 2008 | the fundamental diagram; the ring-road jam experiment | `v2xw-trafficval` (`fdlab`, `model`, `traffic_calibration`) |
| 3GPP TR 36.885 / TR 37.885 traffic models | highway and urban drop models | `v2xw-mobility::demand::tr36885` |
| GTFS | transit schedules | `v2xw-demand::transit` |
| NYC DOT Automated Traffic Volume Counts | Manhattan's hourly profile | `v2xw-mobility::demand::daily` |

## Energy and emissions

| Document | Implemented in |
|---|---|
| HBEFA 3.1 emission factors, polynomial fit after Krajzewicz et al. 2015 | `v2xw-energy::hbefa3` |
| PHEMlight (Hausberger et al. 2009) | `v2xw-energy::phemlight` |
| US EPA MOVES operating modes; MOVESTAR (Wang, Wu & Scora 2020) | `v2xw-energy::moves` |

## World and terrain

| Source | Implemented in |
|---|---|
| OpenStreetMap | `v2xw-world::osm` |
| SUMO `net.xml`, ASAM OpenDRIVE | `v2xw-world::sumo` |
| SRTM 1 arc-second, Copernicus GLO-30 DEMs | `v2xw-world::dem` |
| Natural Earth admin-0, US Census cartographic boundary states | `crates/v2xw-node/data/regions.bin` (`tools/regions/build.py`) |

## Simulators compared against

| Simulator | Use |
|---|---|
| SUMO | co-simulation tier and cross-validation (`v2xw-interop`, `cross_validate`) |
| Veins, OpenCV2X | radio cross-validation figures (`v2xw-interop::radio`) |
