# Refinement: Direction 1, "Quantum-safe US V2X" (key `pqc`)

Written 2026-10-18. It refines `big-three.md` §2 and §5. Sources were read from primary text where it could be reached. Text was extracted with pypdf from `research/papers/*.pdf`, from the 5GAA report PDF (https://5gaa.org/sdc_download/9673/?key=ok470ekbj87kz6upytevrxy7rli8oj) and from NIST IR 8547 ipd (https://nvlpubs.nist.gov/nistpubs/ir/2024/NIST.IR.8547.ipd.pdf). Engine facts come from grep and sed only.

## 1. Source check (claims in big-three.md §2 and §5)

| Claim in big-three.md | Verdict | Evidence |
|---|---|---|
| 5GAA/GSMA PQC report published 6 Oct 2026 | **Confirmed** | 5GAA page and PDF (35 pp.) |
| PQC-TR §5.2: larger signatures "reduce spectral efficiency on PC5", cause "more frequent collisions", need "re-optimisation of scheduling"; 10–15-year device lives | **Confirmed**, verbatim | PQC-TR §5.2, §5.2.1 |
| PQC-TR §6: migrate high-priority systems by about 2030, all by about 2035; IEEE 1609 "is starting" PQC extensions; ETSI TVRA cost-benefit; C2C-CC white paper | **Confirmed**, verbatim. §6 adds that the dates are *deployment* dates, so standards must be final "well in advance" | PQC-TR §6 |
| (Missed) PQC-TR §5.2.4 suggests securing frequent messages by "combining symmetric authentication with periodic public-key signatures", with mechanisms chosen by message *impact* | **New ask; add as a profile arm** | PQC-TR §5.2.4 |
| PQC-TR discusses SCMS, butterfly keys or ECQV | **No.** It never mentions them. The "provisioning" angle is ours, not 5GAA's | grep of the PDF: no hits for butterfly, SCMS or implicit |
| "By NIST's dates, ECDSA in V2X must be deprecated around 2030 and disallowed by 2035" | **Correct it.** IR 8547 Table 2 deprecates only *112-bit* ECDSA after 2030. ECDSA ≥128-bit, which includes the P-256 that 1609.2 uses, is only "disallowed after 2035". IR 8547 is still an initial public draft (12 Nov 2024), and no final version was found | IR 8547 ipd, Table 2; https://csrc.nist.gov/pubs/ir/8547/ipd |
| IEEE 1609 PQC work | **Only second-hand.** No public 1609.2 PQC PAR was found; the only PQC PAR found is 802.11bt (approved 10 Sep 2025). RIT's Aug 2026 lecture says P1609.2-2025 and P1609.2.1-2026 are "being finalized", with no PQC mentioned. So the PQC amendment comes after these revisions, and the window is open | https://www.ieee802.org/11/PARs/P802.11bt_PAR.pdf; https://www.rit.edu/wisplab/sites/rit.edu.wisplab/files/2026-08/4.%20V2V%20Security%20%28August%202026%29.pdf |
| abrar2026deployment: "signature sizes only … no certificates" | **Wrong.** It models certificate-bearing SPDUs (a certificate every 5th BSM, per J2945/1; size 105+pk B). Falcon-512 needs 1,735 B, which takes 7 subchannels at MCS 11. It meets 90% PDR only at LOS-A under LOS, and nothing meets it under NLOS (claim confirmed). Its stated limits: one intersection; OpenCV2X lacks J3161 §7.3.3 SPS+one-shot and the SPS_004 subchannel rule | abrar2026deployment pp. 1–3, 8, 18–19 |
| twardokus2024cryptography: certificates "up to 93% redundant" | **Confirmed** | NDSS 2024 abstract |
| chen2026pseudonym: field test with an OBU in New Taipei | **Misattributed.** The Danhai/New Taipei field test with a Clientron OBU is in arXiv 2501.07028 (Chen & Lin, ICCPCT 2024). 2606.14008 does device benchmarks only | arXiv 2501.07028 abstract; chen2026pseudonym |
| The proposed profile is "PQ only in the infrastructure and long-lived certificates" | **Not novel.** chen2026pseudonym already proposes pure-PQ RCA/ICA/ECA/PCA/RA and enrolment certificates, with hybrid short-lived pseudonym certificates (PQ CA signature, ECC end-entity key). That profile is an arm to evaluate, not our contribution | chen2026pseudonym §I contributions |
| "Butterfly expansion and ECQV have no standardised PQ equivalent" | **True only with "standardised".** PQ butterfly constructions exist: qSCMS (R-LWE, qTESLA + LPR, implicit and explicit models; ePrint 2018/1247; thesis oliveira2019qscms), and Eaton–Lamontagne–Matsakis, *Provably Secure BKE from CRYSTALS* (ePrint 2024/946, rev. Jul 2025; modified Kyber/Dilithium with proofs of unforgeability and unlinkability). Neither is FIPS 203/204-compliant: qTESLA was dropped by NIST, and 2024/946 modifies the schemes. Also salman2022implementing. The engine's own comment ("There is no post-quantum butterfly", `crates/v2xw-proto/src/hybrid.rs:25-30`; `crates/v2xw-sec/src/scheme/mod.rs:40-42`) overstates this too | ePrint pages; oliveira2019qscms abstract |
| arXiv 2601.11104 overlap | **None.** D'Onghia, Berbecaru and Lioy, X.509 PQ-PKI strategies including CRL/OCSP; not V2X. Useful for the CRL format only | arXiv 2601.11104 abstract |
| "Authors of arXiv 2509.16489 not verified" (§6) | **Now verified:** Ahmad, Abrar, Dasgupta, Rahman (Alabama/Clemson). It is the co-simulation testbed under abrar2026deployment, which makes it the same group | arXiv API |

## 2. Competitor watch (2025–2026)

- **Clemson/TraCR, Alabama and FIU** (Chowdhury, Rahman, Abrar).
  - Output so far: arXiv 2509.16489 (testbed), 2510.08496 (AI-driven PQC), 2601.01068 (ITS PQC review) and 2608.05087.
  - Funded project TRB RiP 2531081: $498,924, 1 Jan 2025 to **31 Dec 2026**. Its objective 2 is "a certificate segmentation algorithm to integrate PQC into IEEE 1609.2" for C-V2X (https://rip.trb.org/View/2531081).
  - **Likely next (inferred):** a 1609.2 certificate-segmentation paper; multi-intersection or NLOS runs; J3161 SPS+one-shot, which is the future work their paper names. Probable venues are TRB 2027 or 2028, IEEE TITS or arXiv, in early 2027.
- **RIT/GVSU** (Twardokus, Rahbari).
  - Twardokus's PhD thesis (Aug 2026), *Cross-Layer Security and Responsive Evaluation Architectures for Quantum Resistance in Connected Vehicles*, has three parts:
    - 5G C-V2X adaptive modulation plus hybrid message authentication, which cuts safety violations by ">92%";
    - a 6G VPKI architecture for 800 vehicles/km²;
    - the DT-CoVeSS digital twin and the RECAST safety link (https://repository.rit.edu/theses/12750).
  - CHASM (TechRxiv 2025) does PQC certificate fragmentation with adaptive modulation for C-V2X (cited in abrar2026deployment).
  - Rahbari's NSF CAREER award ($600k, 5 years from 2023) promises "a roadmap with interim security standards" for the 5G-to-6G transition with backward compatibility (https://sos-vo.org/node/93643).
  - **Likely next:** the thesis chapters as papers, i.e. the 6G VPKI and the C-V2X hybrid scheme at NDSS, USENIX Security or IEEE TMC, plus a roadmap paper.
- **Chen (Chunghwa Telecom / NTUT).** Six PQ-SCMS preprints from 2024 to 2026: PQCMC implicit certificates (2401.13691), PQCWC, the multivariate anonymous certificate (2607.13554), and 2606.14008, now in an IEEE Canadian journal. These are device-level only.
- **Yoshizawa, Preneel (KU Leuven).** ZK-signature PQ message authentication (2610.11490, 8 Oct 2026). It is a message-layer arm.
- **How to differentiate.** Nobody owns these four things:
  1. the **SCMS backend and provisioning** layer (batches, top-up, CRL/linkage seeds, multi-provider);
  2. a **mixed fleet over calendar years**;
  3. **J3161-compliant** LTE plus NR Mode 2 at city density;
  4. **provisioning privacy under harvest-now-decrypt-later** (§3).

  RIT owns the message layer, the safety link and the VPKI design, so do not compete there; cite it and use it as arms. Post the E2 preprint before TraCR's end date of Dec 2026, if possible.

## 3. Sharpened contribution

**Thesis (one sentence).** For the US SCMS, the migration order matters more than the choice of algorithm. Signatures on pseudonym certificates can stay classical until close to the 2035 cutoff, under a crypto-agile hybrid certificate cycle. The *pseudonym-provisioning channel* is already exposed today: an adversary who records it now can link pseudonyms to enrolments retroactively once a quantum computer exists. So the KEM on that channel, and the infrastructure certificates, must migrate first.

**Headline result wanted.** On a J3161-compliant LTE-V2X and NR Mode 2 channel at USDOT Plan densities:
- every "PQ everywhere" or "hybrid everywhere" profile drops below 90% PDR above a named density or fleet share;
- an *infrastructure-first, provisioning-KEM-first, hybrid-certificate-cycle* order stays within about 5 points of ECDSA awareness through 2035;
- that order also closes retroactive-deanonymisation exposure about N years earlier;
- per-certificate PQ keys, the non-butterfly fallback, multiply the provisioning upload and PCA load by a named factor. That factor is the number 1609 needs to decide whether a PQ butterfly (ePrint 2024/946) is worth standardising.

**Contribution claims (each defensible).**
1. **The first system-scale, standards-compliant evaluation of PQ migration for the US SCMS.**
   - Coverage: certificates, P2PCD, provisioning, CRLs, a mixed fleet by model year, LTE Mode 4 under J3161 (including SPS+one-shot, which OpenCV2X lacks) and NR Mode 2.
   - It reproduces abrar2026deployment and twardokus2024cryptography as validity anchors.
2. **A provisioning cost model and crossover analysis over four families:**
   - (P0) classical ECC butterfly with PQ CA signatures (the chen2026pseudonym hybrid);
   - (P1) per-certificate PQ keys encrypted to the PCA, the engine's current model;
   - (P2) PQ butterfly (qSCMS; ePrint 2024/946), modelled by its published sizes and timings;
   - (P3) P0 plus an ML-KEM or hybrid KEM on the download path.

   The thesis **evaluates** these families and does not invent new PQ cryptography. It needs no new proofs and states the cryptographic assumptions of each family.
3. **A harvest-now-link-later privacy analysis of SCMS provisioning.**
   - It counts which recorded artefacts (RA requests, PCA-encrypted batches, TLS) let a future quantum adversary link archived BSM traces to enrolment identities, under each family and each migration year.
   - *Novelty is unverified. Run a targeted search before claiming it.*
4. **A migration-order profile with break-points, as a 1609 WG contribution input.**
   - For each profile: the (year × density × radio) cell where it breaks.
   - Also the cost of each hybrid verification rule against strip and verification-exhaustion attacks.
   - It answers PQC-TR §6's call for standards "well in advance" and §5.2.4's call to differentiate by message impact.

## 4. Paper plan

**Research questions.**
- RQ1: At what density and fleet PQ share does each certificate profile break J3161/J2945-1 reliability?
- RQ2: What does each provisioning family cost, in uplink and downlink bytes per vehicle per week, PCA/RA CPU and CRL bytes?
- RQ3: What privacy exposure under harvest-now-decrypt-later does each migration order leave, and for how many years?
- RQ4: Which hybrid verification rule resists stripping and exhaustion at an acceptable backlog?

**Artefact.** Five things:
- the migration-profile specification (encodings, P2PCD rules, which keys migrate when);
- a per-vehicle capability model (crypto by model year, plus over-the-air-updatable or not, per PQC-TR §4.2's "legacy dilemma");
- the four provisioning families in the engine;
- an open experiment grid, which doubles as SCMS-Bench track T4;
- measured ARM timings.

**Cost sizing.** The basis is ~2.4 h per 600 s at ~450 vehicles, and ~5 min for small fleets (`critique-methods.md` l.141). Channel metrics are distribution-level, which `critique-methods.md` §1 allows, so 120 s measurement windows after a 30 s warm-up suffice.

| Exp. | Factors × levels | Runs | Cost and where |
|---|---|---|---|
| E1 validity | abrar's six LOS levels × {LOS, NLOS} × {ECDSA, Falcon-512} (24 cells) × 5 seeds; plus a twardokus redundancy check on 3 densities × 3 seeds | 129 | ≈ 20–30 h local (single intersection, ≤ 200 vehicles) |
| E2 migration trajectory | fleet PQ share {10, 30, 60, 100%} × density {3 Plan-anchored levels} × radio {LTE-M4/J3161, NR-M2} × profile {ECDSA, ML-DSA-44 everywhere, hybrid everywhere, PHA cert cycle (twardokus2024cryptography), infrastructure-first hybrid PC (chen2026pseudonym), symmetric + periodic PQ signature (PQC-TR §5.2.4)} = 144 cells × 5 seeds | 720 | **Cloud.** About 300–600 h at 150 s windows; ~30 parallel workers for about 1 day. Sixteen corner cells are rerun at 600 s locally as a check |
| E3 provisioning and CRL | family {P0–P3} × PQ scheme {ML-DSA-44, Falcon-512, ML-DSA-65} × batch {20, 60/week (NYC)} × fleet scale {1k, 10k devices, extrapolated} × 3 seeds | 144 | Small-fleet backend runs at ~5 min each ≈ 12 h local; scaled analytically and checked at 2 sizes |
| E4 hybrid attacks | rule {accept either, require both, PQ-if-capable} × attacker {strip, oversize-flood} × rate {2} × density {2} × 5 seeds | 120 | ≈ 40 h local or cloud |
| E5 hardware | ECDSA, ML-DSA-44/65, Falcon-512 and hybrid sign/verify; ML-KEM encaps/decaps on two OBU-class ARM boards (e.g. Cortex-A53 and A72) with liboqs | n/a | About 1 week; feeds `crates/v2xw-node/src/profiles.rs` |

**Baselines.**
- ECDSA P-256 today, under J2945/1 certificate-every-5th;
- abrar2026deployment's Falcon-512 configuration;
- twardokus2024cryptography PHA;
- chen2026pseudonym hybrid PC;
- yoshizawa2026zero as an optional message arm.

**Validation anchors.**
- abrar's numbers: 1,735 B, 7 subchannels, LOS-A only;
- twardokus 93% redundancy;
- USDOT LTE-V2X field PER (ylmaz2025simulated) and NIST 1609.2 captures (song2024c), as in big-three.md;
- FIPS 204/206 sizes;
- the J3161 SPS rules;
- NYC weekly batch sizes (big-needs).

**Figures and tables.**
- (F1) Validity: engine versus abrar PDR(d) and versus twardokus redundancy.
- (F2) Break-point map: PDR ≥ 90% region per profile in the fleet-share × density plane, LTE versus NR.
- (F3) Migration trajectory 2027–2036: awareness, CBR and verification backlog per profile.
- (T1) Provisioning cost per family: upload/download bytes, PCA CPU, CRL growth.
- (F4) Retroactive-linkability exposure (share of archived pseudonyms linkable) versus year, per migration order.
- (T2) Hybrid verification rules: attack success versus backlog.

**Hypotheses.**
- H1: hybrid-everywhere breaks first on LTE/J3161 above LOS-B. It is milder on NR Mode 2.
- H2: PHA and infrastructure-first PC stay within 5 points of ECDSA up to 60% share.
- H3: P1 costs ≥ 10× the P0 upload, and P2 recovers most of it.
- **H4 (decision-changing):** provisioning-KEM-first removes most retroactive linkability years before any BSM-signature migration, at negligible channel cost. If it holds, 1609.2.1 should migrate first and 1609.2 BSM signatures last, which reverses the message-first emphasis of current work.

## 5. Engine prerequisites mapped to code

**Exists.**
- Schemes:
  - `ecdsa-p256`, `hybrid-mldsa44-ecdsa-p256`, `hybrid-falcon512-ecdsa-p256`, `hybrid-mldsa65-ecdsa-p256` and `ml-dsa-44` (`crates/v2xw-sec/src/scheme/{classical,hybrid,hybrid_mldsa65,post_quantum}.rs`);
  - KEMs `ml-kem-512/768/1024` and the `hybrid-ecies-p256-ml-kem-768` hybrid (`crates/v2xw-sec/src/scheme/kem.rs`).
- Credential-cost model for batches, chains, trust lists and CRLs (`crates/v2xw-proto/src/hybrid.rs`).
- PHA certificate cycle (`crates/v2xw-net/src/frag/cert_cycle.rs`).
- Device timings (`crates/v2xw-node/src/profiles.rs`).
- LTE Mode 4 / NR Mode 2 (`crates/v2xw-radio/src/cv2x.rs`, `sps.rs`) and J3161/1 DCC (`crates/v2xw-radio/src/dcc.rs:1251`).
- Linkage seeds (`crates/v2xw-proto/src/scms/mod.rs:515`).
- P2PCD metrics (`crates/v2xw-metrics/src/security.rs`).

**Must build, in order.**
1. **Per-vehicle crypto capability.** Today there is one scenario-wide `security.signature` (`crates/v2xw-engine/src/scenario/schema.rs:2409-2412`). The build adds a per-vehicle scheme and mixed-fleet receiver rules. This blocks E2 and E4.
2. **J3161 §7.3.3 SPS+one-shot and the SPS_004 subchannel rounding.** The engine says "not modelled" (`crates/v2xw-radio/src/sps.rs:173`). This is the same gap abrar names, so closing it is a differentiator.
3. **Provisioning families P0, P2 and P3** next to the current P1 in `v2xw-proto/src/hybrid.rs`, with the "no PQ butterfly" comments corrected.
4. A **harvest-now-decrypt-later exposure metric** (archived artefacts → linkable pseudonyms).
5. **Symmetric + periodic-signature arm** (PQC-TR §5.2.4).
6. **Imported hardware timing tables.**
7. **Short-window runs and a cloud runner.**

Per-event RNG and warning-driven driving are *not* blocking, because these are distribution-level metrics (`critique-methods.md` l.11, l.61).

## 6. Risks and mitigations; fallback

- **Biggest new risk: RIT has already promised a "roadmap with interim standards"** (NSF CAREER), and the Aug 2026 thesis has the hybrid C-V2X and VPKI pieces. Mitigation: own the SCMS backend, provisioning, harvest-now-decrypt-later and mixed-fleet calendar, which RIT does not touch (its thesis abstract never mentions SCMS). Use their schemes as arms, and preprint early.
- **TraCR certificate segmentation lands around Dec 2026.** Mitigation: include segmentation as an arm. Our J3161 one-shot fix targets their named limitation.
- **The 2030 urgency is weaker than stated**, because P-256 is only disallowed after 2035 and IR 8547 is still a draft. Mitigation: frame the work on PQC-TR §6's lead-time argument and on harvest-now-decrypt-later, which is urgent now.
- **H4 is already known.** Mitigation: run the targeted novelty search first (week 1). If it is known, keep it as quantification.
- **Radio validity.** E1 must match abrar within the seed confidence intervals, or the paper reports bounded deviations.
- **Fallback paper if the channel result is null** (every profile fits): *"The cost of post-quantum pseudonym provisioning and retroactive linkability in the US SCMS"*. It needs only E3+E4+H4, with no radio-validity dependence. Targets: IEEE VNC 2027, PETS or IEEE TVT.

## 7. Six-month plan (2026-10-18 → 2027-04)

| Month | Work |
|---|---|
| Nov 2026 | H4 novelty search; prereqs 1–2; E1 reproduction; order the ARM boards |
| Dec 2026 | Prereqs 3–5; E5 timings; E3 runs. Short preprint of E1+E3 before TraCR's 31 Dec end |
| Jan 2027 | Cloud runner; E2 grid; first F2/F3 |
| Feb 2027 | E4; VehicleSec '27 short paper (deadline **inferred** late Feb 2027; the '26 deadline was 24 Feb 2026, https://www.usenix.org/node/315631) |
| Mar 2027 | Write the full paper; draft the 1609 WG contribution |
| Apr 2027 | Submit |

**Venue.**
- Main: NDSS 2028 summer cycle, deadline **inferred** late Apr 2027. NDSS 2027 dates were not confirmed from the official CFP (https://www.ndss-symposium.org/ndss2027/submissions/).
- Alternative: USENIX Security '28 cycle 1, **inferred** about Aug 2027.
- USENIX Security '27 cycle 2 (submission 26 Jan 2027, **confirmed**, https://www.usenix.org/node/321195) is too early for the full study.
- Journal: IEEE TIFS, rolling.
