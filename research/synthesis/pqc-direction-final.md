# Direction 1, final: harvest now, link later in the US SCMS

Written 2026-10-18. Merges `verify-pqc-relinking.md`, `verify-pqc-order.md` and `verify-pqc-competitors.md` into one decision. It supersedes the thesis, contributions and experiment matrix in `research-plan.md` §1. Those three files hold the citations and page numbers for each claim here.

## 1. Verdict: needs refinement (keep the direction, change the headline)

- **The migration order is already published, so it cannot be the contribution.**
  - Generic guidance already says "key establishment first, signatures later" (NIST IR 8547 *initial public draft*, pp. 14–15 and 24; RFC 9958; BSI).
  - V2X orders exist:
    - twardokus2024cryptography (era plan, ECDSA BSMs under a partially hybrid cycle);
    - **fikry2026post** (IEEE ASSIC, Aug 2026; PQ CA and enrolment certificates plus Kyber, with ECDSA pseudonyms; earlier notes missed it);
    - chen2026pseudonym (PQ infrastructure, hybrid pseudonym certificates, PQ KEM to RA and PCA).
- **The re-linking threat is real, specific and unclaimed.** It is narrower than first written, and it contradicts both standards-side PQC studies:
  - ETSI TR 103 949 Table 3 rates confidentiality protection duration "N/A";
  - 5GAA/GSMA (Oct 2026) p. 10 says harvest-now is "not the main concern for V2X".
  - No source names the mechanism, and "harvest now, link later" has no hits. Residual novelty risk is **low to moderate**: the paywalled fikry full text and the ePrint 2024/946 and 2018/1247 introductions are unread.
- **Three statements in the old thesis were wrong:**
  1. Infrastructure certificates carry **no** harvest-now privacy risk. "Infrastructure first" rests on forgery and lead time, which is a separate and already-published argument.
  2. "The provisioning KEM" is really two components with different costs and reach: TLS (T) and ACA response encryption (R).
  3. Migration stops *new* exposure. It never closes what is already archived, so "closes the exposure N years earlier" is wrong in form.
- **The radio channel does not drive the privacy result.** Provisioning runs over IP. Density and fleet-share break points are a secondary cost result, not the source of the headline.

## 2. Refined direction

**One-sentence thesis.** In the US SCMS, a vehicle's pseudonyms stay unlinkable against a future quantum adversary only as long as the certificate-download path stays confidential, whichever signature algorithm is used. Today that path rests on ECDHE TLS and ECIES to the butterfly cocoon keys. So the ACA response encryption, not the infrastructure certificates or BSM signatures, must migrate first. We quantify the linkable pseudonym-years that each migration order leaves behind, and what closing them costs at city scale.

**Falsifiable form of "order matters more than algorithm".** Across provisioning families P0–P3 and all signature schemes, privacy exposure depends only on the year the response encryption migrates. The signature choice moves only the channel break points.

### 2.1 Threat model

- **Assets.**
  - P–E: linking a pseudonym certificate to its enrolment certificate, and so to a device and, at an access vantage point, to its IP or subscriber.
  - P–P: linking pseudonyms of the same device across i-periods.
- **A_out, the outsider recorder.** Passive. Sits on the access network or backhaul between the vehicle and the LOP/RA, and may also capture BSMs over the air. It records:
  - TLS sessions (I1);
  - `EeRaCertRequest` (I2), the download header (I3), batch zips (I4) and MBR uploads (I5).
- **A_RA, the archive insider.** Holds the RA database (honest-but-curious RA or a breach), with **no ACA collusion**. The RA stores up to 3 years of encrypted batches (I8).
- **A_bc, the broadcast-only baseline.** Holds BSMs (I6) and CRLs (I7). Claim, to be proved by sketch: it gains no linkage. Each certified key is U = B + c·G with uniform c, linkage values come from two 128-bit AES seed chains, and PLVs are AES-CCM.
- **Quantum capability.** A CRQC exists from year Y_q (swept over 2035, 2040 and 2045). It solves P-256 ECDLP with a run budget K per target. Grover on AES-128 is out of scope.
  - Cost per device: one run per recorded TLS session, plus one run on the caterpillar key H. That H unlocks every later batch, because the US profile is "request once, top up forever".
- **Out of scope:** forgery (forward-only; Twardokus's era argument), active attacks, and kinematic tracking (used only as the baseline it adds to).
- **Stated assumptions, each a scenario parameter:** TLS terminates at the RA, not the LOP (CAMP122 §5.1.8); the unified/CUBK response key formula; US pseudonym certificates are implicit (ECQV); how the LA–ACA PLV key is agreed.

### 2.2 Contribution claims

**Defensible (hedge with "to our knowledge"):**
- **C1 Mechanism.** An item-by-item trace (I1–I8) shows that pseudonym unlinkability is a harvest-now asset in the US SCMS.
  - It is concentrated in butterfly provisioning, and broadcast traffic alone is not exposed.
  - The formal hook is Boldyreva et al. (PoPETs 2024): unlinkability holds under ODH plus IND-CPA, which a quantum adversary breaks.
  - This contradicts ETSI TR 103 949 and 5GAA/GSMA.
  - Claim to be the "first to identify the mechanism and quantify it", not the first to link harvest-now to V2X privacy (abrar2026 p. 3 gestures at the link).
- **C2 Outsider/insider split and the standards ask.**
  - Hybrid ML-KEM TLS (T) closes A_out only if TLS is end-to-end to the RA. It never closes A_RA. It is cheap and generic, so say so.
  - Only PQ response encryption with unlinkable per-certificate keys (R) closes both. R means a PQ butterfly or chen's KEM-to-PCA design, and needs a PQ recipient type for `AcaEeCertResponsePrivateSpdu` in 1609.2.1.
- **C3 Exposure by migration order.** Linkable pseudonym-years accrued per migration year, run on the real calendar with a model-year fleet mix. Reported for each order:
  - the year accumulation stops;
  - exposure avoided against signature-first orders;
  - the residual tail from the preload horizon and legacy OBUs.
- **C4 Cost of closing it.**
  - Provisioning cost across P0–P3 and T at fleet scale (upload bytes, RA/ACA/PCA load).
  - The channel cost of each signature arm.
  - The result to show: provisioning-first adds zero over-the-air bytes, while PQ issuer signatures on pseudonym certificates do add bytes.

**Claims to avoid:**
- "First migration order for V2X"; "first to connect harvest-now to V2X privacy"; "infrastructure certificates have harvest-now risk".
- "Hybrid TLS fixes it", and equally "TLS migration is useless"; "closes the exposure N years earlier"; C3 presented as a radio co-simulation output.
- "NIST requires P-256 gone by 2030" (the IR is a draft; P-256 is disallowed only after 2035). "No PQ butterfly exists" (ePrint 2024/946, qSCMS and Chen's NTRU work exist, though none uses FIPS 203/204/206).
- Accepting fikry's harvest-now rationale. A 15-year enrolment signature is forge-later; rebut it explicitly.

### 2.3 Headline results wanted

- **R1 (main figure).** Cumulative linkable pseudonym-years against calendar year for five orders (signature-first, infrastructure-first as in fikry/chen, T-first, R-first, T+R), with Y_q curves.
- **R2.** Years of residual exposure after migration, as a function of preload horizon and legacy-OBU share. Also the CRQC runs needed per de-anonymised device.
- **R3.** Provisioning cost table for P0–P3 and T: bytes per vehicle per year, RA/ACA CPU, and the PCA signature octets carried on air.
- **R4 (secondary).** Break points for each signature profile: the density × PQ fleet share where PDR falls below 90%. Alongside, the invariance test: privacy exposure does not move with the signature scheme.

### 2.4 Experiment matrix, sized to the engine

| Exp | Grid | Runs | Where, time |
|---|---|---|---|
| X1 ledger validity | I1–I8 trace on a 100-vehicle scenario, plus **fault injection** (mark R as migrated but leave cocoon ECIES; mark TLS end-to-end when it is not) to show the metric can go red | ~10 | local, <1 h |
| X2 exposure (no radio) | migration year {2028, 30, 32, 35} × order {5} × Y_q {2035, 40, 45} × preload {1, 3 yr} × legacy {0, 20, 40%} × batch {20, 60/wk}; 10k–100k vehicles, fast-forward calendar | 720 configs, post-processed over a few backend runs | local, ~10–20 h |
| X3 provisioning cost | {P0, P1, P2, P3, T} × KEM {ML-KEM-768, hybrid} × sig {ML-DSA-44, Falcon-512} × batch {20, 60} × fleet {1k, 10k} × 3 seeds | 240 | local, ~20 h |
| X4 channel (secondary) | PQ share {10, 30, 60, 100%} × 2 densities × LTE Mode 4 × {ECDSA, infra-first hybrid PC, ML-DSA-44 everywhere, PHA} × 3 seeds; NR Mode 2 only if time allows | 96 | local or cloud, ~40–80 h |
| X5 timings | Published ePrint 2026/2149 (RPi 4 PQ TLS) and the existing liboqs RPi5/MK6 figures. Own ARM boards are optional | n/a | desk |

Dropped from the old plan: E4 hybrid-verification attacks (now future work), the 720-run E2 grid, and the full abrar E1 reproduction (kept only as a 20-run sanity check inside X4).

### 2.5 The empty cell against competitors

| Work | Order | Privacy under PQ | Retroactive linkage of archived provisioning, by year | Fleet/calendar |
|---|---|---|---|---|
| twardokus2024 / RIT thesis / Rahbari CAREER | ● / ◐ | ○ | ○ | ◐ |
| fikry2026post | ● | ○ (its harvest-now argument is really forge-later) | ○ | ○ |
| chen2026pseudonym + series | ● | ● (design property) | ○ | ○ |
| abrar2026, TraCR 2531081 / 2696971 | ○ / ◐ | ○ / ◐ | ○ | ○ |
| ePrint 2024/946, qSCMS | ○ | ● (proofs for new schemes) | ○ | ○ |

The fourth column is empty in every row. Second-emptiest: SCMS provisioning cost at fleet scale across P0–P3.

### 2.6 Engine build list, in order (no code yet)

1. **Harvest-now archive ledger and exposure metric**, including A_RA, with the fault-injection test. ~0.4–0.6k LOC, mostly post-processing over backend events (`crates/v2xw-metrics`). This alone gives X1 and a first X2.
2. **Per-vehicle crypto capability and model-year fleet mix** (`security.fleet[]`, receiver rules, metrics by class; `schema.rs:2409-2416`). ~0.8–1.2k LOC.
3. **Hybrid ML-KEM TLS arm T** (`TlsVersion::Tls13HybridMlkem`; `ip_transport.rs:396`, `backend.rs:277`), plus one flight-size measurement script. ~150–250 LOC.
4. **Provisioning families P0, P2, P3 and the R arm** beside P1 (`crates/v2xw-proto/src/hybrid.rs`). Also fix the l.24 "no PQ butterfly" comment. ~0.5–0.7k LOC.
5. (Secondary, for X4) J3161 §7.3.3 SPS+one-shot and SPS_004 (`sps.rs:172`). ~0.3–0.5k LOC; needs the J3161/1 text.
6. (Optional) symmetric plus periodic-signature arm, ~300 LOC. Cloud runner only if X4 grows.

Items 1–4 (~1.9–2.75k LOC) are enough for the paper.

## 3. Venue and timeline

| Target | Fit | Deadline |
|---|---|---|
| arXiv note: C1 + C2 + first X2 | Stakes the claim before Chen/RIT/TraCR | Dec 2026 to Jan 2027 (own choice) |
| USENIX VehicleSec '27 (short or full) | Good: V2X security, mechanism plus quantification | late Feb 2027, *inferred* |
| PoPETs 2028 (issue 1 or 2) | **Best fit** once privacy is the headline; Boldyreva's proof appeared there | ~end May or end Aug 2027, *inferred* |
| NDSS 2028 summer cycle | Possible; needs C1–C4 complete plus the proof sketch | late Apr to early May 2027, *inferred* |
| USENIX Security '27 | Too early | 26 Jan 2027, confirmed |
| Journal extension: IEEE TIFS or TDSC; IEEE 1609 WG contribution (the R ask) | — | rolling |

Recheck every inferred date when the call is posted.

## 4. Biggest risks and mitigations

1. **Reviewers call it obvious ("ECDH is Shor-breakable; just use hybrid TLS").** Lead with the two standards bodies that concluded the opposite, the A_RA case that T cannot close, the concrete 1609.2.1 ask, and the quantified tail (pre-migration archives plus the 3-year preload).
2. **Scooped.** Chen's group already has the design and could add a harvest-now framing; RIT and TraCR are active but have no SCMS work. Read the fikry full text and the ePrint 2024/946 introduction this month, and post the arXiv note by January.
3. **Factual assumptions turn out wrong** (TLS termination, CUBK formula, PLV key agreement). Make each a parameter and report both branches; check CAMP122 §5.1.8. If the LA–ACA key is ECDH, A_bc gains a channel: report it as a finding.
4. **"Why a simulator?"** Answer honestly: the engine supplies per-vehicle download schedules, connectivity, batch timing and fleet mix for C3, and the cost and channel results for C4. Do not oversell co-simulation.
5. **The CRQC date is speculative.** Report every result conditional on Y_q.

## 5. Five sources the owner must read personally

1. **Boldyreva et al., PoPETs 2024** (butterfly key mechanism proofs): Thm 6.1, Thm 7.2 and the p. 13 future-work line. This is the formal basis of C1.
2. **ETSI TR 103 949 V1.1.1**, Table 3 (p. 16), p. 18 and p. 23, with **5GAA/GSMA PQC report** p. 10 and p. 17 alongside. These are the statements the paper contradicts.
3. **fikry2026post** (IEEE ASSIC 2026, DOI 10.1109/assic70037.2026.11717405). Paywalled, so use a library. It is the closest ordering paper, and its harvest-now framing must be rebutted.
4. **chen2026pseudonym** (arXiv 2606.14008), p. 6 and pp. 9–10. It is the closest design and already closes I2–I4. Use it as the R arm.
5. **Eaton–Lamontagne–Matsakis, ePrint 2024/946**, introduction (it returned 403 to the agents). It is the last novelty check, and also the P2 cost source.

Also check personally: CAMP SCMS docs §5.1.8 (where TLS terminates) and the 1609.2.1 recipient types for `AcaEeCertResponsePrivateSpdu`.
