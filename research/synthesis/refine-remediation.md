# Refinement: Direction 3, evidence-grade misbehaviour remediation

Date 2026-10-18. Refines `big-three.md` §4, using `big-needs.md` need 2–4, `big-anatomy.md`, `final-three-approaches.md` §2, `critique-framed.md` and `critique-methods.md`. Downloads were read in full text in the scratchpad: 5GAA MBD-Op v1.0 (29 May 2024), 5GAA MBD WP 2022, C2C-CC WP 2092, ETSI TS 103 759 v2.1.1 and v2.2.1, 5GAA CP-TR v1.0 (14 Jul 2025), arXiv 2606.20215 and CATS. The patents were read through Google Patents.

## 1. Source check (confirm or correct big-three.md §4)

| Claim in big-three.md | Verdict | Evidence |
|---|---|---|
| "have not yet specified the conditions" (MBD-Op §1) | **Confirmed, but incomplete.** The next sentence reads: "This work does not attempt to specify those but attempts to describe what conditions might be used". §6 adds a severity rating and §7 a mapping, with families A1 (natural progression), A2 (severity based) and A1+A2 | MBD-Op p4, p27 |
| A full cost-benefit is "resource-intensive and unfeasible" (MBD-Op §1 and §5) | **Corrected.** The quote sits in §5.2.1, inside the R0 "Mild" example on p12: "The intent is not to do a full cost-benefit analysis, which may be resource-intensive and unfeasible, but to use expert judgment". It is a choice of method, not a claim that the analysis is impossible. Reword the pitch from "does what 5GAA said could not be done" to "quantifies the cost-benefit 5GAA replaced with expert judgement" | MBD-Op p12 |
| §8 open items (draft spec, reinstatement, suspension without reports, new MR fields, new revocation mechanisms) | Confirmed | MBD-Op p28 |
| Ladder R0/R1/R2, ST0–ST3, RC1–RC4 | Confirmed | MBD-Op p16–19 |
| CP-TR: no NA CP; MA "should cooperate … should support end entity revocation requests from other MAs" | Confirmed: clauses (24)–(26), plus (25) "should provide linking information for reported ACs". (13) says "An SCMS Provider shall cooperate with MAs". SLAs: revocation in 5 days, CRL in 24 h | CP-TR p13, p21–22, p57 |
| TS 103 759 "v2.2.1 revision, 2026" is the future target | **Corrected.** V2.2.1 was **published in January 2026** (History, p43). It is still proof-based: an MR "shall include evidence … so that the MA is able to verify the observations independently … the original received message" (§4.2.4). It adds an MR dissemination protocol (§5.3) and optional MR metadata for pre-processing (§6.3), and says "Multiple instances of the Misbehaviour Authority component may exist". Any input now goes to the next revision | ts_103759v020201p.pdf |
| Annex D evidence | Annex D (informative) gives example detectors per class. **Relay-framable rows:** "Position outside of own communications coverage area" (class 1) and "Position inconsistent with maximum plausible range" (class 4). **Spoofing-framable rows:** the class-2 kinematic rows. The mandatory evidence per class is §6.2: classes 1, 3 and 4 need one message; classes 2 and 5 need two | TS 103 759 v2.2.1 §6.2, Table D.1 |
| C2C-CC: "MA should be able to recompute the same misbehaviour checks" | Confirmed, p24. The WP is dated **2021-12-17**, so cite it as 2021 | C2C-CC WP 2092 p1, p24 |
| 5GAA 2022 p12: slander plus Sybil can cause wrongful revocation; reporter accountability helps | Confirmed. **New hook on p24:** "What is the minimal set of evidence needed by the MA … should the MA's judgement be the same for … an unintentional interferer compared to an intentional interferer?" p25 also proposes a "misbehaviour policy envelope". The direction answers questions 5GAA wrote down | 5GAA 2022 p12, p24–25 |
| Intel US12489779B2 "slander defence design" | Confirmed, with an addition. Granted 2025-12-02, priority 2021-09-23. Independent observers with different pseudonyms, an LA linkage check, an evidentiary threshold per type, and repeat-false-reporter monitoring. It also describes the MA as "a distributed and federated network of LMAs" in which the MA re-verifies so that "a compromised LMA" cannot revoke honest nodes. That **overlaps layer 3 (cooperation) as a design**. No measurements | Google Patents |
| Qualcomm US12260747B2 | Confirmed. Granted 2025-03-25. MCM-specific: the MA cross-checks reports and queries the LA. No evaluation | Google Patents |
| CATS (TVT 2025): central banning, no SCMS evidence model | Confirmed. Built on SUMO at city scale, "230x reduction of bad" data; votes resolved with real identities | asavisanu2025cats p2, p6 |
| jimenez2026gnss: spoofing makes OBUs send implausible CAMs | Confirmed in full text. A HackRF transmits over the air to a Commsignia OBU; acceleration peaks reach "close to 20 m/s²", above the CV-pilot MaxAcceleration threshold of 10 m/s². Speed stays at 200 km/h or below, under the 90 m/s MaxSpeed threshold. "Without being detected" refers to the OBU and RSU, not to any MA | arXiv 2606.20215 §5; `03-protocols-and-operations.md:295` |
| SAE J3287 | Published 2024-08-30, 23 pp, paywalled. Still unread | SAE Mobilus J3287_202408 |
| park2024enhancingb | **Still unread.** The Wiley full-text and PDF URLs return 403 behind Cloudflare. OpenAlex and S2 list only the publisher copy (gold OA, CC BY-NC-ND). Wayback has no snapshot. A search-engine snippet shows two things: it criticises earlier field tests (SCA) for assuming "a misbehaving reporting vehicle is always honest", and it treats flooding the global server "with numerous valid reports" | DOAJ; S2 tldr; web snippet |
| Gap table complete | **Missing:** willecke2023a4md (A4MD, VNC 2023), an open detection → reporting → **reaction** loop on Artery/F2MD; and DRAMBR (Almani et al., Future Internet 2025, doi 10.3390/fi17040174) on false accusations in sparse-RSU areas | summaries; Crossref |

## 2. Competitor watch (2025–2026)

- **TU Braunschweig (Willecke, Wolf).** Published A4MD (2023) and PriMiRe (VTC-F 2024). Likely next: an MA reaction or eviction policy study in ETSI C-ITS using A4MD. *Differentiate* on the US SCMS: LA linkage queries, linkage-seed CRLs, two providers on one CTL, and proof re-verification of CV-pilot MBRs.
- **KETI / Autocrypt (Park et al.).** The K-City field test covering slander and global-server flooding; also an RSU MBD paper (park2024enhancing, OJ-ITS 2024); Autocrypt sells a combined local and global MBD. A Korean RSU radio-fingerprint patent (USPTO 12705976, priority 2024-12-31) is a likely next step towards physical-layer corroboration. *Differentiate* with measured framability per check and a policy frontier, not a single deployed design. Read Park before writing claim C1.
- **UPM/INSIA (Jiménez, Naranjo), arXiv 2606.20215.** The obvious next paper is GNSS-spoofing *detection*, or its effect on MBD, on their real-OBU testbed. That would pre-empt our hardware hook. *Mitigation:* treat their published traces as the validation anchor and offer to collaborate.
- **RIT (Twardokus, Rahbari).** V2Verifier and jamming discreditation. Likely next: framing on SDR hardware.
- **USC (Psounis, Qiu).** CATS; a follow-up may add SCMS pseudonyms.
- **IRT SystemX (Kamel, Kaiser, Haidar).** F2MD, DARE and the MA Sybil work. VeReMi NextGen (hermann2026veremi) is a dataset, not an MA policy.
- **Industry.** The Intel and Qualcomm grants (2025) show that MA designs are being patented, not published.
- **Searches run.** arXiv (sorted by date): MA, MBR, slander and revocation queries return no 2025–26 MA-policy paper. Misbehaviour detection there is all ML detectors (2609.31217, 2605.06833, 2512.15503). Crossref from 2025 finds the VNC 2026 poster "V2X Simulation of C-ITS Message Misbehavior" (Jerbi et al., 10.1109/vnc69225.2026.11629129), not read. OpenAlex was over quota today. **No quantified, adversarial MA-policy evaluation was found.**

## 3. Sharpened contribution

**Thesis.** The evidence that TS 103 759 and the CV-pilot MBR require lets an outsider with a ~$300 SDR GNSS spoofer, or a relay, produce MA-verifiable evidence against an honest US vehicle. A footprint-aware rule that needs LA-distinct reporters across disjoint events blocks this at a bounded, stated cost in time to neutralise and LA queries. That bound holds across two SCMS providers only if the CP's "should cooperate" becomes a "shall", with a deadline.

**Headline result wanted.** "Under every proof-based MA policy family consistent with the standards, the CV-pilot `Implausible/MaxAcceleration` and TS 103 759 class-2 kinematic checks get a GNSS-spoofed honest vehicle suspended as fast as a real attacker. A spoofing-footprint test plus k=2 LA-distinct events removes ≥99% of these framings for ≤1 CRL cycle (24 h) of extra time to neutralise."

**Contribution claims** (each limited by §1 and `critique-framed.md`):
1. **Framability matrix.** The first per-check matrix (CV-pilot MBR types and TS 103 759 Annex D classes 1–5) of the cheapest capability whose evidence *survives MA re-verification*, under a proof-based MA. Validity anchor: ObsImplausible must come out relay-framable (SM-MBR default off).
2. **Policy frontier.** For the 5GAA §7 families (A1, A2, A1+A2) × corroboration rules: wrongful suspensions per 10⁶ vehicle-hours, time to neutralise, LA queries per case and reinstatement load (RC1–RC4). This is the quantified cost-benefit that MBD-Op §5.2.1 replaced with expert judgement.
3. **Split-evidence across providers.** The first measurement of what CP clauses (24)–(26) as "should" cost against an attacker whose evidence falls into two providers' MAs, and the cross-provider query price of making them binding.
4. **Artefact.** An open proof-based MA (MBR evidence verifier, LA query ledger, remediation tiers) and the attacker set, released with a draft policy annex in the form of the 5GAA 2022 "misbehaviour policy envelope" (p25).

## 4. Paper plan

**Research questions.**
- RQ1: which checks are framable, and at what minimum capability?
- RQ2: which rule closes each framable cell, and at what price?
- RQ3: how often do benign correlated GNSS faults produce wrongful suspensions?
- RQ4: what does weak cross-provider cooperation cost?

**Attacker set** (every attacker produces *validly signed* evidence; cheapest first):
- **A0.** Benign spatially correlated GNSS fault (urban canyon).
- **A1.** Outsider GNSS spoofer aimed at the victim, with footprint radius {10 m tailing, 50 m, 200 m}.
- **A2.** Outsider relay or replay of the victim's BSMs at {1 km, 5 km} (ObsImplausible, Annex D "outside coverage").
- **A3.** Insider with *n* concurrent pseudonyms creating class-5 position conflicts with the victim, n ∈ {1, 5, 20} (brecht2018security: at least 20 per week).
- **A4.** Insider making class-4 sensor-slander claims (unverifiable non-V2X evidence), with k colluders ∈ {1, 3}.
- **A5 (control).** A forged MBR with no valid evidence (`report.rs::forge`). A proof-based MA must reject it, and only a counting MA falls for it.

**MA policy families.**
- **P0.** Count-based (`ma.rs` LegacyWindow).
- **P1.** Event-correlated (`ma_corroborate.rs`).
- **P2.** Proof-based re-verification.
- **P3.** P2 plus k LA-distinct reporters over ≥2 disjoint events, k ∈ {2, 3} (Intel rule).
- **P4.** P3 plus a spoofing-footprint test (an implausibility shared by co-located senders counts as environmental).
- **P5.** P2 plus a trusted RSU witness.
- **P6.** The CATS two-window rule.

Each family is crossed with:
- the 5GAA mapping A1, A2 or A1+A2;
- MaMode ManualReview (8 h assumption) or Auto.

**Experiment matrix and cost.** The engine runs about 2.4 h per 600 s at about 450 vehicles, or about 5 min for a small fleet (`docs/validation/test-run-cost.md`).

| Exp | Factors × levels | Scenario | Runs | Cost |
|---|---|---|---|---|
| E1 framability | 12 attacker cells × 2 maps (grid, Manhattan canyon) × 2 densities × 10 seeds | small fleet, one victim, 300 s | 480 | ~40 h local |
| E2 policy frontier | 7 families × 3 mappings × 2 MaModes × 3 k, **backend-only replay** of E1 and E3 MBR streams | replay | 126 × all streams | minutes |
| E3 benign FP | GNSS {Reid-2019 baseline, canyon heavy tail, correlated area fault} × 2 maps × 5 seeds | 6,000 veh/h, 600 s | 30 | ~72 h: **cloud** or 2 weeks of nights |
| E4 multi-provider | cooperation {none, should (p=0.5, 5 d), must (24 h SLA)} × fleet share {50/50, 80/20} × attacker {A3, A4} × 10 seeds | two-provider deployment, small fleet | 120 | ~10 h |
| E5 hardware (optional) | conducted-cable spoofing of a GNSS receiver; NMEA trace injected as the victim's GNSS belief | lab | — | needs an RF lab |

**Baselines.**
- P0 and P1 (the engine's own);
- the F2MD MA Stress attack (kamel2020detection);
- CATS (asavisanu2025cats);
- the A4MD reaction loop as an external comparison point, if available.

**Validation anchors.**
- ObsImplausible relay-framable (SM-MBR; `03-protocols-and-operations.md:295`).
- A1 acceleration peaks reproduce the "close to 20 m/s²" of arXiv 2606.20215.
- GNSS error fitted to Reid 2019 percentiles (`v2xw-mobility/src/gnss/gauss_markov.rs:23,79`).
- CP SLAs: 5 days and 24 h (CP-TR p57).
- MBR thresholds (SM-MBR).

**Figures and tables.**
- T1: attacker set with capability cost.
- F1: framability heatmap (check × capability; cell = survives re-verification / suspended).
- F2: policy frontier (wrongful suspensions per 10⁶ veh-h vs time to neutralise, per family).
- F3: benign wrongful-suspension rate vs the GNSS tail, per rule.
- F4: multi-provider time to neutralise vs cooperation regime.
- T2: the draft MA policy (check → sufficient for R0/ST0/ST1/ST2, corroboration, RC) with its LA query budget.

**Hypotheses.**
- **H1 (decision-changing).** A1 frames `Implausible/MaxAcceleration` and the class-2 kinematic checks under P2 and P3; only P4 separates victim from attacker. *Decision:* J3287 and TS 103 759 editors mark these checks as insufficient alone for ST1/ST2; the SCMS Manager maps them to ST0 with RC1.
- **H2.** P3 with k=2 closes A3 and A4 at ≤5 LA queries per case and ≤24 h extra time to neutralise.
- **H3.** P0 wrongfully suspends at a fleet-visible rate under the canyon GNSS tail; P1 and P2 cut it ≥10×.
- **H4.** Under "should", split evidence never reaches threshold within one MBDR cycle; "must" closes the gap.

## 5. Engine prerequisites mapped to code (build in this order)

1. **Key the report streams by report identity.** Report and ServiceTime are keyed by node with a running index (`v2xw-threat/src/poison.rs:307`; `v2xw-radio/src/cellular.rs:823`). Re-keying makes E2's backend-only pairs exact (`critique-methods.md` line 59).
2. **Bridge reports to verifiable evidence.** The threat-pipeline reports carry only digests and detector fingerprints ("Every identity in a report is a pseudonym certificate digest", `v2xw-threat/src/report.rs:15-20`). **Today's MA therefore cannot re-verify anything.** Route detections into the proto MBR (`v2xw-proto/src/scms/mbr.rs`: `Evidence`, `SignedBsmsWithCertificate`, `EvidenceObsImplausible` at l.207, l.399), then add a P2 plug-in beside `v2xw-threat/src/ma_corroborate.rs` that re-runs the `ts103759.rs` and SM-MBR checks on the evidence.
3. **LA query ledger.** The investigation flow *exists*: `investigate`, `request_link`, `open_case`, LA1/LA2 and the RA blocklist (`v2xw-proto/src/scms/run/revoke.rs:1-117`; `MaMode` in `params.rs:337`). `ma.rs:24-29` says this flow is not wired to the threat MA. Missing: the wiring, a query counter, and the LA-distinct reporter rule (P3).
4. **Attackers.**
   - A1: a victim-targeted override of the GNSS belief with a footprint radius, at the belief hand-off (`v2xw-node/src/runtime.rs:785-787`; `v2xw-mobility/src/gnss/`).
   - A2: a relay (only referenced, at `attack.rs:558`).
   - A3: an insider with conflicting pseudonyms (class 5).
   - A4: insider sensor slander (the `ts103759.rs` class-4 perception cross-check).
5. **Remediation tiers.** ST0 maps to the RA blocklist (exists). Add ST1 (partial revocation), RC1–RC4 reinstatement and the A1/A2 mappings.
6. **Per-provider MA routing and cross-MA requests.** `deployment.rs` already has two providers, each with its own MA and an `ma_mode` (l.300); add the cooperation protocol.
7. **Warning-driven driving** (`critique-methods.md` §2.4). *Not* needed for the headline, which uses fairness and operations metrics; it is needed only for a net-harm appendix.

## 6. Risks and mitigations; fallback

**Biggest new risk: the scoping of claim C1 may not hold up.**
- Park 2024 is still unread. It fields slander defences and global-server flooding at K-City.
- Intel's patent already describes a federated LMA design, which touches claim C3.
- *Mitigation:*
  - Obtain Park through a library or ILL, or ask the authors (do not put the owner's identity in any request this tool makes; the student does this personally).
  - Write C1 as a "per-check *measured* framability frontier", not as "first slander defence".

**Other risks.**
- *Normative risk:* a spoofed device really did send bad data, so suspending it with RC1 may already be the "right" 5GAA outcome. The result must then be a quantity (how many false ST1/ST2 cases, and how long they last), not the existence of framing.
- *Legal risk:* over-the-air GNSS spoofing is unlawful in the US. E5 must use a conducted cable or a shielded chamber, or reuse the UPM traces.
- *Confidential MA policy:* report every policy family (`big-three.md` §4).
- *Engine risk:* step 2 is a real refactor. Budget 4 weeks for it.

**Fallback if H1 is null** (footprint tests are unnecessary because spoofing footprints always hit neighbours):
- Publish E3 + E2 as "Benign correlated GNSS faults and wrongful suspension under standard MA rules", with the LA query budget, at IEEE VNC or VehicleSec.
- Add the E4 cooperation result as a short standards input to 5GAA and the SCMS Manager.

## 7. Six-month plan to first submission

| Month | Work |
|---|---|
| Nov 2026 | Steps 1–3 (RNG keys, MBR evidence bridge, P2 MA, LA ledger); obtain Park 2024 and J3287 |
| Dec 2026 | Attackers A1–A5; E1 small-fleet grid (40 h) |
| Jan 2027 | E2 replay frontier; E3 on cloud; F1–F3 |
| Feb 2027 | **Submit to VehicleSec '27:** E1 + E2 + E3, framability and benign FP. Deadline *inferred* late Feb 2027 (VehicleSec '26 was 24 Feb 2026, usenix.org/node/315631) |
| Mar–Apr 2027 | Steps 5–6; E4 multi-provider; E5 conducted-spoofing lab or UPM traces; MA operator check (ISS MAS) |
| May 2027 | **Full paper to NDSS 2028 summer cycle.** *Inferred* early May 2027; NDSS 2027's summer deadline was 6 May 2026 (ndss-symposium.org/ndss2027 CFP). Alternative: USENIX Security '27 cycle 2, **26 Jan 2027 (confirmed, preliminary CFP)**, but that is too early for E4 and E5. Journal: IEEE TIFS |

Sources not otherwise linked: MBD-Op https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf; MBD 2022 https://5gaa.org/content/uploads/2022/07/5GAA-Misbehaviour-detection-Final.pdf; C2C-CC https://www.car-2-car.org/fileadmin/documents/General_Documents/C2CCC_WP_2092_MisbehaviourDetection_and_Reporting_V1.0.pdf; TS 103 759 v2.2.1 https://www.etsi.org/deliver/etsi_ts/103700_103799/103759/02.02.01_60/ts_103759v020201p.pdf; CP-TR https://5gaa.org/content/uploads/2025/07/credential-management-supporting-v2x-commercial-deployments.pdf; patents https://patents.google.com/patent/US12489779B2/en, https://patents.google.com/patent/US12260747B2/en; USENIX Sec '27 https://www.usenix.org/node/321195.
