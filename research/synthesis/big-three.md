# The three biggest defensible directions

Written 2026-10-18. Inputs:
- `big-anatomy.md`: what made past contributions big;
- `big-needs.md`: what industry, standards bodies and government say is unsolved;
- `final-three-approaches.md` §2: "which evidence can frame you", the only earlier idea the owner liked;
- `novelty-check.md` and `critique-methods.md`.

Fresh novelty searches were run for this file (§4).

Rules applied:
- Ideas are made bigger, not smaller. Each objection is answered by a better design.
- Every finalist names a decision-maker who would act on the result.
- The engine must be a real advantage.
- Effort is ignored, but engine prerequisites are listed.

---

## 1. Eight ambitious candidates

| # | Candidate | Contribution type | Need or impact pattern it rides | Engine advantage |
|---|---|---|---|---|
| A | **Quantum-safe US V2X: a migration design for the full 1609.2/SCMS stack, with system-scale evidence across the migration years** | New protocol and profile, validated end to end, plus standards evidence | Need 1 (PQC-TR §5.2, §6; NIST IR 8547), need 5 (FCC 24-123 30 MHz), CRL size in need 4. Anatomy §2: top-venue papers fix a deployed standard (twardokus2024cryptography, NDSS 2024) | Very strong. PQC and hybrid signatures already exist, with real 1609.2/1609.2.1, P2PCD, LTE and NR sidelink, and a real calendar |
| B | **SCMS-Bench: an open reference platform and public benchmark for the credential and misbehaviour layer, scored by counterfactual safety effect** | Platform plus benchmark | Anatomy patterns 1–2: VeReMi and its Extension have 492 citations, more than any detector; OPV2V reached 736 and DAIR-V2X 681 in about 4 years. Novelty-check N1, N3 | Strong. It is the only open engine found that joins traffic, radio and a real multi-provider SCMS, and it can rerun the same seed |
| C | **Evidence-grade misbehaviour remediation: a quantified, attack-tested MA policy for the US SCMS** (which evidence suffices, which remediation tier, how MAs of different providers cooperate) | Policy specification plus a definitive study | Needs 2, 3, 7 and 10. MBD-Op §5 calls a full cost-benefit "unfeasible". The CP-TR says MA cooperation is only a "should". Folds in the framing idea | Strong. A proof-based MA, LA linkage, the real CRL and multiple providers |
| D | Multi-provider trust dynamics: CTL, elector and root rollover failure modes | Definitive study | Need 3; USDOT Plan interoperability | Near-unique (5 electors, multi-provider CTL), but the gap is narrower (novelty-check R2: "partly novel") |
| E | Revocation at the edge: a protocol that pushes linkage-seed revocation peer to peer, in the style of P2PCD | New protocol | Need 4. MBD-Op §5.4: the cycle "can take a week" | Good, but crowded (scopelliti2024efficient, NDSS 2024; khodaei2018efficient) |
| F | The security airtime budget at USDOT Plan densities in the 30 MHz band | Policy-question study | Need 5 | Good, but on its own it is a sub-result of A |
| G | Pseudonym supply and backend sizing for the 2028–2036 Plan targets | Policy-question study | Needs 6 and 8 (NYC raised supply from 20 to 60 a week) | Good, but it gives load, not dollars, and decision-makers would see it as engineering |
| H | A learned surrogate or world model of the V2X security stack, trained on paired counterfactual runs | Learned model | Novelty-check N4. It also attacks the engine's 2.4 h run time | Moderate. The value depends on B existing first |

**Selection.** A, B and C each address a written, unmet demand from a named body. Each is a contribution type with an impact precedent (anatomy §2–3), and none collapses under the novelty search.
- D folds into C, as cross-MA cooperation.
- E and F fold into A, as CRL size and the airtime budget.
- G and H become pieces of B: a backend-load track and a surrogate baseline.

---

## 2. Finalist 1: Quantum-safe US V2X (direction A, folding in E and F)

**Pitch.** By NIST's dates, ECDSA in V2X must be deprecated around 2030 and disallowed by 2035 (NIST IR 8547 ipd). Vehicles live 10–15 years, so a mixed fleet is unavoidable (PQC-TR §5.2).
- IEEE 1609 "is starting" PQC extensions.
- ETSI is adding a PQC cost-benefit analysis to its TVRA.
- C2C-CC is writing a migration white paper (PQC-TR §6).

None of them has system-level numbers. This direction proposes a concrete migration profile for the US stack and evaluates it, year by year of the migration, at city scale. The profile covers certificate format, P2PCD and digest policy, how pseudonyms are provisioned without butterfly keys, the CRL format, and the hybrid rules for legacy devices.

**Headline claim (to be tested).** One structural fact drives the design: butterfly key expansion and ECQV implicit certificates have no standardised PQ equivalent. So a fully PQ US SCMS needs a changed pseudonym-provisioning design, not just bigger signatures.

The claim is that, at Plan densities, a specific hybrid profile can keep awareness within a stated margin of today's ECDSA all the way to the 2035 cutoff. The profile is:
- digest-first;
- learned certificate attachment (extending twardokus2024cryptography);
- PQ only in the infrastructure and long-lived certificates;
- a PQ pseudonym batch-issuance scheme.

The obvious alternatives cross the 90% delivery line at named densities and fleet shares. They are PQ-everywhere, and the composite certificates in chen2025 (arXiv 2501.07028) and chen2026pseudonym (arXiv 2606.14008).

**What is built or proposed.**
1. The migration profile, as a candidate IEEE 1609.2/1609.2.1 amendment input. It covers:
   - certificate and signature encodings for hybrid and composite forms;
   - P2PCD rules;
   - a PQ pseudonym-issuance protocol that keeps the unlinkability properties of the SCMS;
   - the CRL and linkage-seed format.
2. A mixed-fleet migration model: per-vehicle crypto capability by model year, and the infrastructure rollout order.
3. Hardware evidence: sign and verify timings for ECDSA, ML-DSA, Falcon and hybrids, measured on an OBU-class ARM SoC and fed into the engine. This answers the anatomy finding that no top-venue paper rests on simulation alone.

**Key experiments and the decision-changing result.**
- **E1. Validity.** Reproduce two published results:
  - abrar2026deployment (arXiv 2608.05087): with one RSU, Falcon-512 reaches 90% PDR only at LOS-A;
  - twardokus2024cryptography (doi:10.14722/ndss.2024.24267): certificate transmissions are up to 93% redundant.
- **E2. Migration trajectory.** Each migration year (2027–2036) is a grid of fleet PQ share × density (USDOT Plan targets) × radio (LTE Mode 4 and NR Mode 2) × profile. Measured outputs:
  - awareness and inter-packet gap;
  - verification backlog;
  - channel busy ratio in 30 MHz;
  - P2PCD learning traffic;
  - pseudonym top-up and backend load;
  - CRL size and distribution time.
- **E3. Hybrid downgrade and attack surface.** During the transition, an attacker can strip the PQ half or force verification exhaustion with large PQ certificates (novelty-check R3). Measure the cost of each hybrid verification rule.
- **Decision-changing output.** For each profile, the year and density at which it breaks. This gives 1609 and 5GAA a defensible order (which certificates migrate first) and a go or no-go on composite certificates over PC5.

**Who adopts it.**
- The IEEE 1609 WG, through its PQC extension of 1609.2. A contribution document carries the profile and the numbers.
- The 5GAA/GSMA PQC follow-up (PQC-TR §6).
- The C2C-CC migration white paper.
- ETSI TC ITS, for the TVRA cost-benefit.
- SCMS providers, which would need to change pseudonym issuance.

**Closest prior work and the exact gap.**

| Work | What it does | Gap |
|---|---|---|
| twardokus2024cryptography (NDSS 2024) | PQC integration for V2V, with certificate-transmission scheduling; hardware | Message layer only. No SCMS backend, no provisioning, no CRL, no mixed fleet across migration years, no NR Mode 2 at city density |
| abrar2026deployment (arXiv 2608.05087) | J3161 transport-block fit; one-RSU co-simulation | Signature sizes only, a single receiver, no certificates or backend |
| chen2025 (arXiv 2501.07028); chen2026pseudonym (arXiv 2606.14008) | Hybrid PQC/ECC certificate designs; field test with an OBU (New Taipei); device benchmarks | Device level. They keep ECC for OBUs and have no fleet-scale channel or backend evaluation |
| fikry2026post (doi:10.1109/assic70037.2026.11717405) | Containerised PQ-aware SCMS that is PQ only for enrolment, CA and DENMs | No radio channel, no traffic, no migration dynamics |
| yoshizawa2026zero (arXiv 2610.11490) | ZK-proof signatures to shrink PQ overhead per message | Message scheme only; a candidate arm for E2 |
| karinsalo2026migrating (doi:10.34190/iccws.21.1.4479); lohmiller2025survey; chouhan2026quantum | Analytical and roadmap studies | No measurement |

**Gap:** no study joins traffic, sidelink, 1609.2.1 provisioning, CRLs and a mixed fleet across the migration years, and no study proposes a PQ replacement for butterfly provisioning evaluated at system scale. The search confirmed this (§4).

**Impact precedent.**
- twardokus2024cryptography (NDSS 2024; 24 citations in 2 years; standards-compatible).
- Anatomy pattern 3: architecture papers that standards cite last decades (whyte2013security, brecht2018security, 325 citations). The PQ layer of the SCMS is the one architecture window still open.

**Venue.**
- Conference: NDSS (Twardokus precedent) or USENIX Security. VehicleSec (co-located with NDSS) is the early workshop step.
- Journal: IEEE TIFS or IEEE TDSC. IEEE TVT is the fallback.
- A parallel IEEE 1609 WG contribution.

**Top three objections and answers.**
1. *"The radio model is not OpenCV2X or a field radio."*
   - E1 reproduces two published numbers.
   - PER versus distance is anchored to the USDOT LTE-V2X field PER (ylmaz2025simulated) and NIST 1609.2 captures (song2024c).
   - Each headline is reported with its robustness across the sensitivity harness's radio settings.
2. *"Simulation-only papers do not land at top venues."*
   - The hardware leg uses measured PQ timings on an OBU-class SoC.
   - The provisioning protocol gets a formal security argument: unlinkability under a non-colluding RA/PCA, as in brecht2018security.
   - The simulator supplies the scale, which hardware cannot.
3. *"Twardokus already integrated PQC into V2V."*
   - His scheme is one arm of the experiment.
   - The contribution is the SCMS layer: provisioning without butterfly keys, the CRL, P2PCD, and backend load. It also covers the migration trajectory and NR Mode 2.
   - Showing where message-level fixes stop being enough is itself the decision input.

**Engine prerequisites.**
- PQ and hybrid encodings must flow through P2PCD, 1609.2.1 download and CRLs, not only through signing.
- A per-vehicle crypto-capability attribute, for the mixed fleet.
- A PQ pseudonym-issuance path, the new protocol.
- NR Mode 2 and LTE Mode 4 checked against field PER.
- Hardware timing tables imported.
- Run time reduced enough for city-scale grids. Channel and backend metrics are distribution-level, so per-event RNG and warning-driven driving are not blocking here.

**Biggest risk.** Being scooped. The Clemson/TraCR team (arXiv 2608.05087; TRB RiP 2531081, https://rip.trb.org/View/2531081) and the Twardokus/Rahbari group are active. Mitigation: post the E2 migration-trajectory result as a preprint and a 1609 contribution early.

---

## 3. Finalist 2: SCMS-Bench (direction B, folding in G, H and novelty-check N1 and N3)

**Pitch.** VeReMi became the misbehaviour field's yardstick because it offered "comparable evaluation" (heijden2018veremi; arXiv 1804.06701). Every V2X security dataset since, including VeReMi NextGen (hermann2026veremi, IEEE VNC 2026), scores detectors as classifiers on message traces. Nothing evaluates the whole credential system, and nothing scores a detector or MA policy by what it does to safety when it runs in the loop.

SCMS-Bench is the open reference platform and public benchmark for that layer:
- frozen city scenarios;
- the real US SCMS;
- tasks with paired counterfactual ground truth;
- baselines;
- a leaderboard.

**Headline claim (to be tested).** Ranking detectors by F1, as today's benchmarks do, gives a different order from ranking them by net safety effect in the loop. Net safety effect means harm prevented by true detections minus harm from false positives and wrongful revocations. The benchmark makes that difference measurable and repeatable.

**What is built.** The open engine, plus:
- **Versioned scenarios:** Midtown and a highway scenario, at three densities.
- **Tracks, each with baselines and metrics:**
  - T1, local detection scored by both F1 and interventional safety effect (N1);
  - T2, MA policy: time to neutralise and wrongful revocations, which is where Finalist 3 plugs in;
  - T3, revocation distribution;
  - T4, PQ migration configurations, Finalist 1's grid;
  - T5, a linkability attack;
  - T6, backend load at Plan scale (G).
- **Exporters:** traces in VeReMi Extension and NextGen format, so existing detectors plug in unchanged.
- **Robustness labels:** each leaderboard ranking states whether it survives the ~300-setting sensitivity harness (N3).
- **Surrogate:** a learned model trained on paired runs (H) as the cheap tier, so users without 2.4 h per run can still compete.

**Key experiments and the decision-changing result.**
- **Rank-flip study.** Run 10+ published detectors (F2MD's set and recent ML detectors on VeReMi) in the loop. Report the Kendall-τ between their F1 ranking and their safety-effect ranking.
- **Decision-changing:** 5GAA MBD-Op §5.2.1 says minimum performance requirements (MPR) for detectors "may need to be considered". SAE J3287 fixes only the report format. The benchmark supplies the metric and the threshold an MPR could be written in.
- **Robustness study:** which detector rankings flip under modelling choices. This becomes the benchmark's validity chapter.

**Who adopts it.**
- The misbehaviour detection research community: the catalog has 173 dataset or benchmark works on misbehaviour, pseudonyms or revocation, most built on VeReMi.
- The Ulm and F2MD groups.
- The 5GAA MBD4V2X work item, for MPR.
- PQ and SCMS researchers who need a system-scale testbed.

**Closest prior work and the exact gap.**

| Work | Gap |
|---|---|
| hermann2026veremi, VeReMi NextGen (doi:10.18725/oparu-60763; MOSAIC, 15 attacks, train/val/test, a generator) | Message-level classification. No SCMS backend, MA, revocation or closed-loop safety scoring |
| kamel2020simulation, F2MD (164 citations); kamel2020veremi | Local detection plus a simple MA. No real SCMS, no counterfactual scoring |
| haidar2020dare, DARE | Offline report dataset for the MA. No traffic or radio loop |
| arXiv 2509.16489, ITS cybersecurity co-simulation testbed | Infrastructure and signal security. No SCMS, no benchmark tasks |
| wehmer2021practical, wehmer2023fast | Call for impact-based metrics, but release no benchmark |

**Gap:** no public benchmark covers the credential lifecycle, or scores detectors and MA policies by closed-loop counterfactual safety. The web search found none (§4).

**Impact precedent.** VeReMi plus its Extension (492 citations), F2MD (164), OPV2V (736) and DAIR-V2X (681). Anatomy pattern 1: later users must cite the tool they ran on.

**Venue.**
- Conference: the NeurIPS Datasets and Benchmarks track, aimed at the ML-for-MBD crowd, or IEEE VNC as the VeReMi home venue. ACM CCS or USENIX artifact tracks are options for the platform.
- Journal: IEEE TDSC or IEEE TIFS. Elsevier Vehicular Communications for the tools paper.

**Top three objections and answers.**
1. *"VeReMi NextGen already exists."*
   - SCMS-Bench exports to its format, so it complements NextGen rather than competing.
   - The new axis is system-level, closed-loop scoring.
   - The rank-flip result shows NextGen-style F1 is not enough on its own.
2. *"Simulated ground truth is not reality."*
   - Each track is anchored to field data: NIST 1609.2 captures, real CAM and pseudonym behaviour (bauder2023cooperative), and USDOT PER.
   - Each ranking carries a robustness label.
   - The benchmark claims comparability, not reality, which is the same claim VeReMi made.
3. *"Nobody will adopt a 2.4-hour-per-run engine."*
   - Three tiers: released traces (zero cost), the surrogate (seconds) and full runs (hours).
   - The engine speed-up is listed as a prerequisite, not waived.

**Engine prerequisites.**
- Per-event RNG keying. Exact counterfactual pairs need it (critique-methods §1).
- V2X warnings that change driving. Without them the FP-harm leg is unmeasurable.
- About a 10× run-time reduction, or the trace and surrogate tiers.
- Frozen, versioned scenarios and a published RNG key contract.
- VeReMi exporters, a placebo audit of pair coupling, and leaderboard hosting.

**Biggest risk.** Adoption is not guaranteed. A benchmark wins only if it is cheap, maintained and seeded with a striking result. Without a rank-flip that surprises people, it is just another dataset.

---

## 4. Finalist 3: Evidence-grade misbehaviour remediation (direction C, folding in the framing idea, D, and needs 7 and 10)

**Pitch.** The deployed systems "have not yet specified the conditions" for when to revoke, suspend or do nothing. A full cost-benefit analysis is called "resource-intensive and unfeasible" (5GAA MBD-Op v1.0, §1 and §5). There is also no North American certificate policy, and cooperation between MAs is only a "should" (5GAA CP-TR, Jul 2025).

This direction does what 5GAA said could not be done. It produces a complete, quantified MA policy for the US SCMS, tested against an adaptive attacker, in three layers:
1. **Evidence sufficiency:** the framability matrix from `final-three-approaches.md` §2. Which detector evidence can be forged against an honest vehicle, and at what attacker cost.
2. **Remediation tiers:** revoke, suspend or ignore, priced by damage prevented against the cost of CRL growth, verification and reinstatement.
3. **Cross-provider MA cooperation:** the linkage queries and revocation requests between providers, and their privacy and latency price.

**Headline claim (to be tested).** Two parts:
- Some standard checks are framable by an outsider with no credentials, for example by GNSS spoofing of the victim (jimenez2026gnss, arXiv 2606.20215). Those checks are not sufficient grounds for revocation.
- A specific rule closes the insider classes and costs less than one CRL cycle in time to neutralise. The rule is k reporters that are distinct at the LA, across two disjoint events, with suspension for "mild" classes. Its LA query budget is stated.

**What is built or proposed.**
- A draft MA policy specification:
  - evidence rules per TS 103 759 class and per CV-pilot check;
  - a remediation table with parameters;
  - a reinstatement procedure;
  - a cross-MA request protocol.
- A proof-based MA in the engine that re-verifies signed evidence.
- A real-target hook: GNSS spoofing of a real receiver or OBU reproduces the implausible-but-validly-signed messages that the framing relies on.

**Key experiments and the decision-changing result.**
1. **Framability matrix** across capabilities (a) to (e), from benign correlated GNSS fault to insider sensor claims. Validity check: the ObsImplausible check must come out relay-framable, matching the CV pilot's decision to turn it off.
2. **Policy frontier:** for each candidate rule, wrongful revocations per million vehicle-hours against time to neutralise and net harm.
3. **5GAA §7 mapping under attack,** including false suspensions and reinstatement delay.
4. **Multi-provider:** an attacker split across two providers' pseudonyms, under "should cooperate" versus "must cooperate". What does weak cooperation cost?

**Decision-changing result:** which checks the J3287 and TS 103 759 (v2.2.1 revision) editors should mark as insufficient for revocation alone, and the corroboration rule and cooperation obligation the SCMS Manager should put into the CP.

**Who adopts it.**
- The SCMS Manager and MA operators, such as the ISS Misbehavior Authority Service (TFHRC operational test, https://ghs.com/news/20200107_CES_ISS_misbehavior_authority_service_testing.html).
- The 5GAA MBD4V2X and MBD-Op follow-up.
- The SAE J3287 committee.
- The ETSI TS 103 759 editors.
- The 5GAA CP authors.
- OEMs, whose warranty exposure comes from wrongful revocation.

**Closest prior work and the exact gap.**

| Work | Gap |
|---|---|
| kamel2020detection, F2MD "MA Stress" | A counting MA. No analysis of which evidence the MA can re-verify |
| Intel US12489779B2; Qualcomm US12260747B2 | Slander-defence designs with no measured attacker cost or query budget |
| 5GAA MBD WP 2022 p12; MBD-Op 2024 | State the problem and propose tiers. No quantification, no false reports |
| twardokus2021targeted | Discredits a victim under a reputation counter. Never followed through to a proof-based MA or revocation |
| asavisanu2025cats (TVT 2025) | Central banning for cooperative perception. No SCMS evidence model |
| willecke2024primire (PriMiRe) | Privacy-preserving report aggregation. No policy evaluation |
| scopelliti2024efficient (NDSS 2024) | The revocation mechanism, not the decision to revoke |
| park2024enhancingb (IET ITS 2024) | **Not yet read (HTTP 403)**; it is the most dangerous possible overlap |

**Gap:** no quantitative, adversarial evaluation of MA remediation policy through a proof-based MA with real SCMS linkage and multiple providers. The searches found none (§5).

**Impact precedent.**
- Pattern 4: evidence on standardised systems changes decisions. chen2018exposing (NDSS 2018) changed how deployed I-SIG controllers were viewed.
- raya2007eviction (459 citations), the founding eviction paper, which this direction updates for the deployed system.

**Venue.**
- Conference: NDSS or USENIX Security, with the GNSS hardware hook; ACM CCS as an alternative.
- Journal: IEEE TIFS or IEEE TDSC.
- Standards input to 5GAA MBD4V2X and SAE.

**Top three objections and answers.**
1. *"Your MA is not the real MA."*
   - The MA is built from every public normative text: TS 103 759, J3287, the CV-pilot report spec and brecht2018security.
   - Results are reported as a policy frontier over the unknown parameters, so any operator can read off its own operating point.
   - Confirmation will be sought from an MA operator.
2. *"Damage cannot be priced without driving that reacts to warnings."*
   - That is a hard prerequisite.
   - Until it exists, layers 1 and 3 (evidence and cooperation) run open loop, and layer 2 reports exposure (attacker-message-hours) as a proxy.
3. *"Victim safety cost is near zero, so who cares?"*
   - The headline is fairness and operations: wrongful revocation rate, time to neutralise, reinstatement load and LA privacy budget.
   - These are exactly the quantities MBD-Op §1 and §8 list as open.

**Engine prerequisites.**
- Warning-driven driving and per-event RNG, for net-harm pairs.
- MA re-verification of evidence.
- LA query accounting.
- A GNSS fault and spoofing model with an urban-canyon error distribution.
- Separate MAs per provider with a cooperation protocol.
- The 5GAA remediation tiers and RC1–RC4 reinstatement.

**Biggest risk.** The real MA policy may be confidential, or may already be settled in ways not visible publicly. In that case the work reads as speculative. A second risk is an unread overlap (park2024enhancingb).

---

## 5. Which to start with

**Start with Finalist 1, quantum-safe US V2X.**
- **Live demand with a deadline.**
  - The 5GAA/GSMA PQC impact assessment was published 12 days ago (6 Oct 2026).
  - IEEE 1609 is starting PQC work.
  - NIST's 2030 and 2035 dates fix the clock.
- **Fewest blocking prerequisites.** Channel, backend and CRL metrics are distribution-level, so neither warning-driven driving nor per-event RNG blocks the headline.
- **The engine already has the pieces.** PQ and hybrid signing, P2PCD, 1609.2.1 and both sidelink radios exist.
- **Competitors are moving.** Clemson/TraCR and RIT are active, so delay costs the most here.

Build its scenario grid as SCMS-Bench track T4 from day one, so Finalist 2 grows out of it. Fix per-event RNG and warning-driven driving in parallel; they unlock Finalists 2 and 3. Finalist 3 is the strongest decision-changer, so it comes next, once closed-loop damage can be measured.

---

## 6. Searches run for this file (2026-10-18)

- **Catalog scripts** (`catalog.jsonl`, 11,315 works) on:
  - PQC × V2X/1609/SCMS: 263 hits; nearest are abrar2026deployment, chen2026pseudonym, fikry2026post and yoshizawa2026zero;
  - hybrid, composite or migration × V2X: 45 hits;
  - benchmark or dataset × misbehaviour, SCMS, revocation or pseudonym: 173 hits, newest hermann2026veremi;
  - remediation, MA or global detection: 22 hits;
  - interoperability, CTL or elector × SCMS: 85 hits, none measuring multi-provider operation.
- **arXiv** (export.arxiv.org, sorted by date, 3 s spacing):
  - "post-quantum" × V2X/V2V/vehicular: newest are arXiv 2610.11490, 2608.05087, 2607.13554, 2606.14008 and 2501.07028; none is a system-scale migration study;
  - misbehaviour × benchmark: nothing new;
  - misbehaviour × revocation: only 2308.02711, 1804.06701 and 1710.08891;
  - SCMS × V2X: performance comparisons (2501.03237, 2311.03360), no policy or PQ migration study.
- **Web searches:**
  - PQ SCMS mixed-fleet system simulation: no match;
  - MA revocation policy benchmark: no match; only scopelliti2024efficient's artifact (https://github.com/EricssonResearch/v2x-self-revocation) and the ISS MAS test;
  - IEEE 1609.2 PQC PAR: not found publicly; only IEEE 802.11bt (PQC, approved 10 Sep 2025, https://www.ieee802.org/11/PARs/P802.11bt_PAR.pdf). The claim that 1609 "is starting" rests on PQC-TR §6;
  - closed-loop safety metric for misbehaviour detection, VeReMi NextGen: no benchmark found.
- **Semantic Scholar:** four queries returned empty (rate limited). OpenAlex was not used, because it was over quota earlier today.
- **Not verified:** the authors of arXiv 2509.16489, and the full text of park2024enhancingb (HTTP 403).
