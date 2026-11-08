# Glossary

## Messages and protocols

| Term | Meaning |
|---|---|
| BSM | Basic Safety Message, SAE J2735; generated per SAE J2945/1 |
| CAM | Cooperative Awareness Message, ETSI EN 302 637-2 |
| DENM | Decentralized Environmental Notification Message, ETSI EN 302 637-3 |
| CPM | Collective Perception Message, ETSI TS 103 324 |
| VAM | VRU Awareness Message, ETSI |
| PSM | Personal Safety Message (VRU), SAE J2735 |
| SPaT, MAP | Signal Phase and Timing; intersection geometry (SAE J2735) |
| SRM, SSM | Signal Request and Signal Status Messages (signal priority) |
| WSA | WAVE Service Advertisement, IEEE 1609.3 |
| WSMP | WAVE Short Message Protocol, IEEE 1609.3 |
| GN, BTP | GeoNetworking (EN 302 636-4-1) and Basic Transport Protocol (EN 302 636-5-1) |
| SHB, TSB, GBC, GUC | GeoNetworking single-hop broadcast, topologically scoped broadcast, geo-broadcast, geo-unicast |
| CBF | contention-based forwarding (GeoNetworking) |
| SPDU | secured protocol data unit (IEEE 1609.2) |
| COER, UPER | canonical octet / unaligned packed encoding rules of ASN.1 |
| PSID | provider service identifier: what an SPDU is for, checked against certificate permissions |
| VWP | the engine-to-browser wire protocol ([protocol/vwp-v1.md](protocol/vwp-v1.md)) |
| MCAP | the recording container format |

## Radio

| Term | Meaning |
|---|---|
| DSRC, ITS-G5 | IEEE 802.11p-based V2X (US and European names) |
| LTE-V2X, NR-V2X | 3GPP sidelink (PC5) V2X; Mode 4 (LTE) and Mode 2 (NR) are autonomous resource selection |
| Uu | the cellular link between a device and the base station |
| OCB | outside the context of a BSS: 802.11 operation without association |
| EDCA | enhanced distributed channel access, the 802.11 contention MAC with four access categories |
| SPS | semi-persistent scheduling with sensing (sidelink) |
| SCI | sidelink control information |
| BLER, PER | block and packet error rate |
| SINR, RSRP | signal-to-interference-plus-noise ratio; reference signal received power |
| CBR | channel busy ratio |
| DCC | decentralised congestion control (ETSI TS 102 687; SAE J2945/1 for the US) |
| LOS, NLOS, NLOSv, OLOS | line of sight; non-line of sight; NLOS by vehicles; obstructed LOS |
| PDR, PIR, AoI, NAR | packet delivery ratio; packet inter-reception time; age of information; neighbour awareness ratio |

## Credential systems

| Term | Meaning |
|---|---|
| SCMS | Security Credential Management System, the US V2X PKI (CAMP design) |
| CCMS, ITS PKI | the European C-ITS credential management system (ETSI TS 102 941) |
| OBU, RSU, EE | on-board unit, roadside unit, end entity |
| DCM | Device Configuration Manager |
| ECA, EA | Enrolment CA (SCMS); Enrolment Authority (ETSI) |
| RA | Registration Authority: receives provisioning requests, expands and shuffles them |
| PCA, ACA, AA | Pseudonym / Authorization CA (SCMS); Authorization Authority (ETSI) |
| LA1, LA2 | the two Linkage Authorities, each holding half of a device's linkage seeds |
| MA | Misbehaviour Authority: receives reports, investigates, revokes |
| CRL, CRLG | certificate revocation list; CRL Generator |
| CTL, ECTL | certificate trust list (SCMS, signed by the electors); European CTL (signed by the TLM) |
| TLM, CPOC, DC | Trust List Manager; C-ITS Point of Contact; Distribution Centre |
| LOP | Location Obscurer Proxy: hides a device's network address from the RA and MA |
| LPF, GPF | Local and Global Policy Files |
| electors | the governance keys that sign the CTL (any 3 of 5 in the US) |
| i-period | the pseudonym validity period index (one week in the US deployment) |
| butterfly key expansion | the construction letting the RA generate unlinkable per-period public keys from one upload (CAMP SCP1) |
| linkage value, linkage seed, pre-linkage value | the values that let the SCMS revoke a device's future pseudonyms without linking its past ones (CAMP SCP2) |
| ECQV | elliptic-curve Qu-Vanstone implicit certificates (SEC 4) |
| HashedId8 | the 8-octet certificate digest used as signer identifier (IEEE 1609.2) |
| P2PCD | peer-to-peer certificate distribution (IEEE 1609.2 clause 8) |
| MBR | misbehaviour report |
| FROST, CGGMP21 | threshold Schnorr (RFC 9591) and threshold ECDSA protocols |

## Traffic

| Term | Meaning |
|---|---|
| IDM, EIDM | Intelligent Driver Model; Extended IDM |
| MOBIL, LC2013, SL2015 | lane-change model; SUMO's lane-change and sub-lane models |
| ACC, CACC | adaptive and cooperative adaptive cruise control |
| HCM | Highway Capacity Manual |
| NEMA | the dual-ring eight-phase signal controller standard |
| VRU | vulnerable road user: pedestrians, cyclists, motorcyclists |
| OD, DTA | origin-destination; dynamic traffic assignment |
| MFD | macroscopic fundamental diagram |
| TTC, PET, DRAC | time to collision; post-encroachment time; deceleration rate to avoid a crash |
| FCW, EEBL, IMA, LTA, BSW, PCW, RLVW, GLOSA | forward collision warning; emergency electronic brake light; intersection movement assist; left-turn assist; blind-spot and lane-change warning; pedestrian collision warning; red-light violation warning; green-light optimal speed advisory |

## Simulator terms

| Term | Meaning |
|---|---|
| model card | the mandatory description of a model: equations, sourced parameters, assumptions, omissions, validation, determinism |
| tier | `abstract`, `medium` or `high` fidelity of a model family |
| ground truth (GT) | the simulator's own state, which no device could know |
| belief | what a node knows: its GNSS fix, its clock, what it received |
| NODE-only | a recording or dataset profile that refuses every ground-truth record |
| visibility class | the label on each recording channel saying who could know its contents |
| content digest | SHA-256 over a recording's messages; the identity of a run's output |
| scenario hash | SHA-256 over the canonical JSON of the resolved scenario |
| focus region | a microscopic area inside a mesoscopic city |
| fast-forward window | a span in which traffic and radio pause and credential events are stepped through |
| `todo-calibrate` | a parameter default without a published source, carrying its calibration plan |
