# Verify: Direction 1 (`pqc`) competitors and engine feasibility

Written 2026-10-18. This checks `refine-pqc.md` §2, §5 and `research-plan.md` §1. Sources are arXiv API, IACR ePrint search, Semantic Scholar, TRB RiP and RIT pages, local PDFs read with pypdf, and engine grep/sed. Nothing was built and no code was changed.

## 0. Verdict in one paragraph

The direction still has an open cell, but **the "migration order" framing is partly taken**. fikry2026post (IEEE ASSIC, Aug 2026) already proposes migrating enrolment and CA certificates to ML-DSA and session keys to Kyber first, while keeping ECDSA on pseudonyms and CAMs. It justifies this explicitly by harvest-now-decrypt-later. `refine-pqc.md` never mentions this paper. Its abstract says: "applies CRYSTALS-Dilithium (FIPS 204) selectively—to longterm enrollment certificates, CA certificates … while retaining ECDSA for short-lived pseudonymous certificates" (Semantic Scholar abstract, DOI 10.1109/assic70037.2026.11717405). chen2026pseudonym (PQ infrastructure plus hybrid pseudonym certificates) covers the same ground. So "infrastructure first, BSM last" can no longer be the contribution. What remains empty is narrower:

- a **quantitative, retroactive** (harvest-now-*link*-later) linkability analysis of the SCMS provisioning artefacts;
- run per provisioning family and per migration year;
- on a mixed fleet and the real calendar;
- at city scale on a J3161-compliant channel.

No paper, thesis or funded project was found that does this (§2).

## 1. Competitors, newest first

| Work | What it covers | What it announces next | Source |
|---|---|---|---|
| Yoshizawa, Abidin, Pena-Gonzalez, arXiv 2610.11490 (8 Oct 2026) | ZK-signature PQ message authentication for cooperative driving. Message layer only | n/a (new) | arXiv API |
| abrar2026deployment, arXiv 2608.05087 (5 Aug 2026; Alabama/Clemson TraCR) | Falcon-512 vs ECDSA SPDUs with certificate every 5th packet; LTE PC5 Mode 4 under J3161/1 CR limits; one intersection; LOS/NLOS. Mentions HNDL for privacy (p. 3) but does not analyse it. Says SCMS was "designed around the compact ECDSA P-256 credentials, leaving open the question of how SCMS scales" (p. 4) | Falcon on NR-V2X; congestion-adaptive hybrid switching; corridors; hardware profiling; J3161 §7.3.3 SPS+one-shot and the SPS_004 rule | `summaries/abrar2026deployment.md`; PDF pp. 3–4 |
| fikry2026post, IEEE ASSIC (pub. 1 Aug 2026; Egypt) | **Selective migration**: Dilithium on enrolment, CA and DENMs; ECDSA on pseudonyms and CAMs; Kyber for session keys. Containerised CA hierarchy. DSRC capacity *analytical* at 50 vehicles (4.3% vs 161%). Its HNDL motive is attached to 15-year *enrolment certificates*, a signature/forgery asset, not to the confidentiality of the provisioning channel | Not stated (abstract only; paywalled) | Semantic Scholar abstract; OpenAlex date |
| chen2026pseudonym, arXiv 2606.14008 → IEEE CJECE 2026 (Chunghwa/NTUT) | Pure-PQ RCA/ICA/ECA/PCA/RA certificates; hybrid pseudonym certificates; ML-KEM-512 encrypted SPDU; a generalized pseudonym scheme whose stated aim is that a PC public key cannot be linked to the EC public key. It notes BKE's unlinkability rests on ECDLP and "may become vulnerable in the presence of quantum computing" (p. 6). Device benchmarks only | Series continues (below) | PDF pp. 1–2, 6, 17–18 |
| Chen series: 2601.07841 (NTRU key expansion, Jan 2026), 2607.13554 (multivariate anonymous cert.), 2501.07028 (New Taipei field test, ICCPCT 2024), 2401.13691, 2410.03678 | PQ butterfly-like key expansion (NTRU, not FIPS 203/204/206); field OBU test. Single-device scale | More PQ anonymous-certificate variants (pattern) | arXiv API |
| Sengupta et al., arXiv 2602.01342 (Feb 2026) | Adaptive quantum-safe crypto for 6G vehicular networks. Its threat model allows HNDL (p. 3). No SCMS | n/a | PDF p. 3 |
| Mamun, Abrar, Rahman, arXiv 2601.01068 (review, Jan 2026) | ITS PQC review. Names "harvest-now-decrypt-later" and "forge-later" against archived vehicular data (p. 12); PKI/SCMS as a gap domain | Feeds TraCR projects | PDF pp. 4, 8, 12 |
| TraCR RiP 2531081 (Alabama/Clemson/FIU, $498,924, 1 Jan 2025 – **31 Dec 2026**) | (1) hard-problem algorithms; (2) **certificate segmentation for 1609.2** on C-V2X; (3) PQC with FL apps. No SCMS, provisioning or privacy | Segmentation paper due around project end | https://rip.trb.org/View/2531081 |
| TraCR RiP 2696971 (same team, $304,508, 1 Apr 2026 – 31 Mar 2027) | **New, not in refine-pqc.md.** AI-driven PQC "dynamic switching" for C-V2X; PQ homomorphic encryption for RSU–cloud privacy; **field-tested** prototypes. No SCMS or certificates | Field results around Q1–Q2 2027 | https://rip.trb.org/View/2696971 |
| Twardokus PhD thesis (RIT, Aug 2026) | Hybrid authentication for 5G C-V2X; a 6G VPKI; DT-CoVeSS twin. The abstract does **not** mention SCMS, pseudonym provisioning, privacy/linkability, HNDL or mixed fleets | Thesis chapters as papers (inferred) | https://repository.rit.edu/theses/12750 (WebFetch) |
| Rahbari NSF CAREER (2023, ~$600k, 5 yr) | "Roadmap with interim security standards" across 5G and 6G; backward-compatible mechanisms; defence against forged safety messages (authentication, not privacy) | The roadmap (no output found yet) | https://sos-vo.org/node/93643; RIT news |
| Twardokus, Joslin, Rahbari, arXiv 2411.15936 | PQ IKEv2 over constrained networks. Not V2X provisioning, but the group's closest work to PQ *key exchange* | n/a | arXiv API |
| twardokus2024cryptography (NDSS 2024) | Hybrid/PQ V2V; PHA certificate cycle; 93% redundancy | Fully PQ V2V after the transition | `summaries/twardokus2024cryptography.md` |
| samara2025post (ACIT 2025) | Replaces the pseudonym PKI with PQ anonymous tokens (VOPRF). An alternative architecture, not SCMS migration | n/a | `summaries/samara2025post.md` |
| wang2026reproducibility (Zenodo 2026) | Viability-theory dataset on PQ signature cost vs control spacing. Analytical; the companion paper is unread | n/a | `summaries/wang2026reproducibility.md` |
| ePrint 2026/2149 | PQ/hybrid TLS benchmarks on a Raspberry Pi 4B for automotive (ML-KEM, FrodoKEM, BIKE; ML-DSA, Falcon, MAYO…) | n/a. **Useful cost source** for PQ TLS on the provisioning path | eprint.iacr.org/2026/2149 |
| ePrint 2024/946, 2018/1247 (qSCMS) | PQ butterfly constructions (modified Kyber/Dilithium; R-LWE) | n/a | ePrint search |
| NIST / NCCoE | **No V2X PQC project found** | — | WebSearch, no hit |

## 2. Coverage matrix against this direction's contributions

Key: ● covered, ◐ partial or qualitative, ○ absent.

| Work | V2V sig. size on channel | Certs on air / PHA | J3161 C-V2X (+NR) | SCMS backend / provisioning cost | PQ butterfly family | Infra-first / selective order | Privacy under quantum (PC↔EC) | **Retroactive HNDL linkage of archived provisioning, by year** | City scale | Mixed fleet × calendar timeline |
|---|---|---|---|---|---|---|---|---|---|---|
| twardokus2024 / thesis | ● | ● | ◐ (5G hybrid) | ○ | ○ | ◐ (PHA cycle) | ○ | ○ | ◐ (6G VPKI density) | ◐ (generations) |
| abrar2026 | ● | ● | ◐ (CR limits; no one-shot; LTE only) | ○ (flagged open) | ○ | ○ | ○ (HNDL named) | ○ | ○ (1 intersection) | ○ |
| fikry2026post | ◐ (analytical) | ◐ | ○ (DSRC) | ◐ (prototype CA tree) | ○ | **●** | ○ | ○ (HNDL on EC signatures) | ○ (50 veh.) | ○ |
| chen2026pseudonym + series | ○ | ◐ (sizes) | ○ | ◐ (device timings) | ● (NTRU, generalized) | **●** | ● (design property) | ○ | ○ | ○ |
| TraCR 2531081 / 2696971 | ● | ● (segmentation) | ◐ | ○ | ○ | ◐ (dynamic switching) | ◐ (RSU–cloud HE) | ○ | ○ | ○ |
| Rahbari CAREER | ◐ | ◐ | ◐ | ○ | ○ | ◐ (roadmap) | ○ | ○ | ○ | ◐ (announced) |
| Sengupta 2602.01342; Mamun review | ◐ | ○ | ○ | ○ | ○ | ○ | ○ | ○ (threat named) | ○ | ○ |
| qSCMS, 2024/946 | ○ | ○ | ○ | ◐ | ● | ○ | ● (proofs) | ○ | ○ | ○ |

**The empty cell.** The column "retroactive HNDL linkage of archived provisioning, by year" is empty in every row, and so is its combination with "mixed fleet × calendar". Second-emptiest: **SCMS provisioning cost at fleet scale across P0–P3**, where everyone is ○ or ◐.

**The mechanism is real and checkable from the SCMS facts.** An outside recorder faces only ECC on both layers:
- the EE request is ECIES-encrypted to the RA *inside* TLS ECDHE ([CAMP122] SCMS-341; `03-protocols-and-operations.md` l.36, l.57);
- the ACA response is encrypted to the butterfly cocoon key (l.36);
- download requests are signed with the enrolment key inside TLS (l.59);
- `encPlv1/2` travel RA→ACA per certificate (l.325).

A CRQC therefore recovers enrolment ID → caterpillar/expansion keys → cocoon keys → decrypted pseudonym certificates, which links archived BSM traces to an enrolment.

Two refinements the paper must state:
- **Migrating TLS alone to hybrid ML-KEM does not close it.** The inner 1609.2.1 ECIES layer must also move.
- **The RA's own archive** (it sees requests in clear and stores j, l.119) is an insider harvest point that no channel migration closes.

The second point strengthens the case for PQ butterfly families (P2) over P3.

## 3. What to change in the direction (recommendations)

1. **Retire "order matters more than algorithm" as the headline.** fikry2026post and chen2026pseudonym already argue for infrastructure-first. Cite both as arms. The headline becomes: *"harvest-now-link-later: how many years of archived US V2X traces become linkable under each migration order, and what closing it costs at city scale."*
2. **Rebut fikry's HNDL rationale explicitly.** HNDL threatens confidentiality. Signatures on 15-year enrolment certificates are a *forge-later* (trust-anchor) issue, not harvest-now. Making this distinction precise is itself a framing contribution.
3. **H4 novelty search done (this note):** no hit on arXiv (queries `harvest`+`quantum`+`privacy`+`linkab`; `quantum`+`pseudonym`; `quantum`+`SCMS`), on ePrint, or in two extended web searches. Keep "first" claims hedged until someone also checks the paywalled fikry full text and ETSI TR 103 941-style EU PQC documents.
4. **Competitor timing.** TraCR 2531081 ends 31 Dec 2026 (segmentation). 2696971 runs to Mar 2027 with field PQ switching. The RIT roadmap is still pending. Post the E3 + HNDL note before Jan 2027, as planned.

## 4. Engine feasibility: what exists

| Need | Exists? | Evidence |
|---|---|---|
| PQ/hybrid signature octets on SPDUs | **Yes**, octet overlay on a real P-256 SPDU; ML-DSA-44/65/87, Falcon-512/1024, SLH-DSA, pure and hybrid | `crates/v2xw-engine/src/signature.rs:1-35`; `crates/v2xw-sec/src/scheme/{classical,hybrid,hybrid_mldsa65,post_quantum}.rs` |
| KEM choice for PCA/device encryption | **Yes**: `security.kem` (`ecies-p256`, `ml-kem-512/768/1024`, hybrid ECIES+ML-KEM-768); wired into the SCMS params | `crates/v2xw-engine/src/scenario/schema.rs:2413-2416`; `crates/v2xw-sec/src/scheme/kem.rs`; `crates/v2xw-engine/src/phase2.rs:2022-2026`; `crates/v2xw-proto/src/hybrid.rs:202-230` |
| Provisioning credential cost (P1 only) | **Yes**: per-certificate PQ key upload; certificate, batch, CRL growth; backend on RPi5 liboqs, device on MK6 | `crates/v2xw-proto/src/hybrid.rs:1-55` (says "There is no post-quantum butterfly", l.24 — wrong, see 2024/946, 2601.07841) |
| Provisioning over cellular TCP/TLS | **Yes**: per-vehicle Uu, TCP CUBIC, TLS 1.2/1.3, 5-min sessions, measured classical flights (256/1,066/80 B) | `crates/v2xw-engine/src/backend.rs:14, 260-290`; `crates/v2xw-radio/src/ip_transport.rs:396-430` |
| **PQ/hybrid TLS** (X25519MLKEM768, ML-DSA server chain) | **No**: `TlsVersion` is {None, Tls12, Tls13} with classical flights only | `ip_transport.rs:396-403`; `backend.rs:277-279` |
| Fragmentation / PHA cycle | **Yes** | `crates/v2xw-net/src/frag/{cert_cycle,generic,facilities}.rs` |
| Sidelink sizing (subchannels per TB) | **Yes**, by MCS payload bits | `crates/v2xw-radio/src/sidelink.rs:900-904`; used at `cv2x.rs:644` |
| J3161/1 SPS (keep 0.8, CR limits, blind HARQ) | **Yes** | `crates/v2xw-radio/src/sps.rs:160-180`; `sidelink.rs:1350` |
| **J3161 §7.3.3 SPS+one-shot, SPS_004 rounding** | **No**: "is not modelled" | `sps.rs:172` |
| Real calendar (2015 epoch, weekly i) + fast-forward through backend events | **Yes** | `schema.rs:656-676` |
| **Per-vehicle crypto capability / mixed old-new fleet** | **No**: one scenario-wide `security.signature`/`kem`. The only `fleet_share` is a provider's RSU share | `schema.rs:2409-2416, 2530-2533` |
| **HNDL / retroactive-linkability metric** | **No**: linkage appears only in CRL/health metrics | `crates/v2xw-metrics/src/{security,health}.rs` (grep) |
| PQ butterfly (P2), ECC-butterfly + PQ-CA (P0), P0+KEM (P3) | **No** (P1 only) | `hybrid.rs:24-35` |

## 5. Build list (rough size; order = dependency)

1. **Per-vehicle crypto capability + model-year fleet mix** (`security.fleet[] {share, signature, kem, ota_upgradable, model_year}`), receiver acceptance rules, metrics split by class. ~800–1,200 LOC across the schema, node security stack and metrics. Blocks E2/E4.
2. **Provisioning families P0, P2, P3** beside P1 in `v2xw-proto/src/hybrid.rs`. P2 from the published sizes and timings of 2024/946 and qSCMS; fix the l.24 comment. ~500–700 LOC plus tests.
3. **PQ/hybrid TLS variant** (`TlsVersion::Tls13HybridMlkem`, an ML-DSA server-chain flight size from measurement, as for the existing `tools/scms_constants/tls_handshake_sizes.py`; timings from ePrint 2026/2149). ~150–250 LOC plus one measurement script.
4. **HNDL linkability metric**: a per-artefact archive ledger, i.e. for each recorded flight, which key protects it (TLS-KEM, RA-ECIES, cocoon-ECIES, encPlv) and its year. The exposure function maps CRQC year × migration year → the share of pseudonyms linkable to an enrolment, including the RA-insider variant. ~400–600 LOC, mostly post-processing over existing backend events; no radio dependence.
5. **J3161 §7.3.3 SPS+one-shot and SPS_004 subchannel rounding** in `sps.rs`/`sidelink.rs`. ~300–500 LOC; **needs the J3161/1 text** (currently second-hand via abrar).
6. **Symmetric + periodic-PQ-signature arm** (PQC-TR §5.2.4). ~300 LOC.
7. **Hardware timing import** (E5) plus a cloud runner (shared with Directions 2 and 3).

Items 1–4 alone support the fallback paper (provisioning cost + harvest-now-link-later), which is also where the empty cell sits. Items 5–6 are only needed for the channel break-point claims.
