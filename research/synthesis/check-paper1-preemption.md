# Check 2: pre-emption risk for Paper 1 ("Honest but wrong")

Written 2026-10-23. Role: critical reviewer. Read-only on engine code; nothing built or run.
The question: is "the benign trigger rate of the deployed US rule set, through the full report → MA → revocation chain, at city scale" still unclaimed, and what exact claim is safe?
Page numbers are PDF pages unless marked "printed".

---

## 0. Verdict

**Not pre-empted on its US-specific core. Pre-empted on every generic part. The headline wording is unsafe as it stands.**

1. **The SCMS Manager MBR v1.0 rule set has never had its benign trigger rate published, on real or simulated data.** No paper, pilot report or industry document found measures it. That holds for gpsDrift 1 m, ConstantPosition, MaxSpeed 90 m/s, MaxAcceleration 10 m/s², BrakeAcceleration, and the weight/quota/upload policy. Check 3 (`check-realdata-and-vesnos.md`) now has the first real-data numbers.
2. **The generic claims are taken:**
   - "Plausibility checks fire on honest vehicles in real traffic" is shown by Bißmeyer 2014: a 76-day FOT with 120 vehicles and 100 RSUs, ≈9.25% message-level false positives, blamed on "unreliable transmission of mobility data and its inaccuracy".
   - "Reports about genuine vehicles reach the MA at city scale" is shown by Kamel 2020: F2MD on LuST, thesis Fig. 3.12.
   - "The MA must tell genuine, faulty and malicious apart" is covered by Kamel 2020 §6.1, Rathore 2023 and Das 2026.
3. **"Deployed" is wrong.** SM-MBR v1.0 is titled "...for Connected Vehicle Pilot Deployment" (2019). No evidence was found that a production OBU runs it in 2026. The CV-pilot MA was a human operator (USDOT primer, 2019). No public document gives any US MA decision rule, including the 2025 5GAA proposed CP. "Revocation" is therefore a property of an MA rule the paper assumes, not of the deployed system.
4. **"Revokes honest vehicles" is not currently supported by the engine.** The 26-vs-0 result (`docs/design/scms-real/testing/usfp.md` §3) comes from the GNSS model (white noise at every fix) and the engine's own k=3 corroborating MA. With a realistic receiver the engine gives 0 honest revocations. Check 3 finds that the real honest trigger comes from a device class (secMark not locked to GNSS epochs), not GNSS noise. A reviewer who reads usfp.md will see that the headline was an artefact.
5. **Residual pre-emption risk** (unread texts):
   - **Park et al. 2024 (IET ITS), full text unobtainable.** The site serves a bot wall to automated access, although the paper is gold OA (CC BY-NC-ND).
   - **SAE J3287_202408 is paywalled.** Its public scope promises "specific report contents for specific instances of misbehavior", which may come with thresholds.
   - **CaTch** is behind HAL's anti-bot wall and IEEE's paywall.
   - **SAE technical papers and CAMP internal reports were not searched.**

Neither Park nor J3287 can be a city-scale benign-rate measurement of SM-MBR. Park is a K-City test-track system. J3287 is a format standard. Each, however, could already define US checks or thresholds that make "the US rule set" ambiguous.

---

## 1. What was read

| Source | Access | Basis |
|---|---|---|
| SCMS Manager, *Misbehavior Report and Application Specification for CV Pilot Deployment* v1.0, 2019-02-14 | scmsmanager.org PDF | full text (11 pp) |
| SCMS Manager, *ASN.1 for Misbehavior Reporting* v1.1, 2019-04-23 | scmsmanager.org PDF | full text (15 pp) |
| USDOT ITS JPO, *SCMS Technical Primer*, FHWA-JPO-19-775, Nov 2019 | ROSA P | full text |
| SAE J3287_202408 *V2X Misbehavior Reporting* | saemobilus.sae.org | **scope only (paywalled)**. No public ASN.1 or slides found. The SAE status report to ITU-T CITS (2023-09-22) lists it only as "In development". |
| 5GAA *Misbehaviour Detection* WP v1.0, 2022-07-13 | 5gaa.org | full text (29 pp) |
| 5GAA *MBD for V2X: Operational Aspects* v1.0, 2024 | 5gaa.org | full text (29 pp) |
| 5GAA TR *Credential Management Supporting V2X Commercial Deployments* v1.0, 2025-07-14 | 5gaa.org | MA, MBR and revocation sections (§1.3.5.8, §4.10) |
| ETSI TR 103 460 V2.1.1 (2020-10) | etsi.org | MBD sections |
| C2C-CC WP 2091/2092 *MBD and Reporting to MA*, 2021-12-17 (kaiser2021white) | car-2-car.org | detection and implementation-results sections |
| Park et al. 2024, IET ITS 18(11):2273-2289, doi 10.1049/itr2.12549 (park2024enhancingb) | Wiley: HTTP 403 bot wall on every route (the full, pdf, pdfdirect and epdf URLs, a reader proxy and the Wayback Machine). No repository copy (OpenAlex `any_repository_has_fulltext: false`). | **abstract + search snippets only** |
| Park, Kim, Lee 2024, IEEE OJITS 5:656-668 (park2024enhancing), the same group's companion paper | IEEE OA PDF | full text |
| Kamel et al., CaTch, WCNC 2019 (kamel2019catch) | HAL (Anubis anti-bot wall, respected) and IEEE (closed) | abstract, plus the thesis account of it |
| Kamel, PhD thesis 2020, tel-03102396 (kamel2020detection) | local `research/papers/` copy | full text, chapters 1, 3, 4 and 6 |
| Bißmeyer, PhD thesis, TU Darmstadt 2014 (bimeyer2014misbehavior) | d-nb.info PDF | full text, chapters 3 and 6 |
| Das, Griffith, Rathore, CCNC 2026 (das2026distinguishing) | IEEE (closed). No PAR or arXiv copy. | abstract |
| Carie et al. 2026, IASC 41(1) (carie2026hierarchical) | techscience HTML | full text (evaluation) |
| CV-pilot documents (THEA, WYDOT, NYC) | ROSA P, ITS KRS (web search) | excerpts |

Searches: arXiv API (2023+, "misbehavior" ∧ {false positive, revocation, SCMS, faulty, "misbehavior report"}), OpenAlex (5 queries, 2023+), web search (11 queries on the CV pilots, J3287, gpsDrift, Tampa BSM MBD, CAMP MBD, real-world CAM traces). Semantic Scholar search returned HTTP 429 throughout, so its coverage is missing. No query contained the owner's identity.

---

## 2. Findings, source by source

### 2.1 SM-MBR v1.0 and ASN.1 v1.1: what "the US rule" actually says

- **The scope is the CV Pilots.** The title reads "...for Connected Vehicle Pilot Deployment" (p.1). "For the current version of the Connected Vehicle Pilot Deployment Misbehavior Detection (CV-PD-MBD) project, only one misbehavior application PSID ... PSID = 0x20" (p.3).
- **The RandomPosition text** (p.6, §2.3.1.2.2): "Two consecutive BSMs are inconsistent if [(position in the first) + (velocity in the first)*(time between)] differs from (position in the second) by more than this parameter", default 1 m.
- **The ASN.1 v1.1 says something different** (p.8, `Psid20MbrParams1VInconsistentRandomPosition`): gpsDrift is "the maximum amount of difference in position between two BSMs, **sent 0.1 seconds apart**, that the reporter believes can be explained by GPS drift. This quantity MAY be 0. The units ... are thousandths of a meter. ... greater than would be indicated by the **speed, acceleration**, and this GPS drift value."
  - The two normative texts disagree. One has velocity only; the other has velocity and acceleration, and a 0.1 s spacing.
  - The engine implements the v1.0 formula with a 0.5 s gap limit (`usfp.md` §1). That gap limit appears in neither text.
  - **A reviewer can attack any single implementation, so Paper 1 must report both variants.**
- **The reporter's threshold is in the report.** gpsDrift travels inside the report as `Uint16` mm. The MA therefore knows each reporter's threshold, which is relevant to MA-side filtering.
- **No false-positive analysis anywhere.** The only false-positive statements cover ObsImplausible:
  - It is off by default "because this test is subject to malicious interference ... This test should only be configured to On if the MA develops tests to avoid revoking innocent BSM senders" (p.9).
  - The ASN.1 says ObsImplausible reports "create a significant risk of false reporting and the MA should use them with caution" (p.6).
  - The documents never say that RandomPosition can fire on honest senders.
- **No MA rule.** The documents define the format, storage and upload (×0.9 at 5 am Eastern; quota "device-specific", pp.4-5), and "How the end-entity obtains the MA certificate is out of scope" (p.10). They say nothing on how the MA decides.

### 2.2 How the US MA actually decided (CV pilots)

- **USDOT SCMS Technical Primer (FHWA-JPO-19-775, p.8):** "an effort to develop a Minimum Viable Product (MVP) version of a Misbehavior Detection system to support the CV Pilot programs. This system utilizes simple algorithms to analyze received BSMs ... **An MA operator will then review those reports**, and in accordance with processes and procedures agreed to by the CV Pilots and USDOT, add misbehaving devices to the CRL."
- **Pilot excerpts found by web search (unverified beyond the snippets):**
  - WYDOT: "misbehavior detection and certificate revocation will not initially be supported by the SCMS". The pilot would "track misbehavior internally and remove devices manually".
  - THEA Phase 2: the "primary means of misbehavior detection ... will entail analysis of OBU or RSU data after the fact".
- **No published pilot report gives MBR counts, false positives or revocations.** Nothing was found for NYC either.
- **Consequence:** the deployed US chain had a human in the MA seat. Any "→ revocation" number is conditional on an assumed automatic MA rule.

### 2.3 5GAA (2022, 2024, 2025)

False positives are acknowledged qualitatively and never measured.

**2022 white paper:**
- p.12: "non-zero false positives and false negatives are to be expected. False positives cause a non-misbehaving device to be falsely accused ... from its messages being dropped to its certificates being revoked wrongfully."
- p.24: the faulty-GPS deny-list use case ("identified as misbehaving due to its faulty GPS (e.g. implausible position displacement over time)").
- p.25: "a reporter could repeatedly report a misbehaving vehicle with a faulty GPS to speed up its revocation."
- p.27 asks: "Should the remediation decision depend on whether it is a malfunction or deliberate misbehaviour?"

**2024 MBD-Op:**
- p.9: "currently there are no specifications of conditions applied to determine when a particular remediation technique is to be used."
- p.12: "How the MBMS determines the existence and source of a misbehaviour is out of scope"; "doing nothing ... may be the suitable response".
- p.13: the "Accidentally malfunctioning EE ... e.g., GPS sensor" category.
- p.27 (next steps): add "impact level, difficulty of local detection, footprint and duration" to reports.
- No quantitative content.

**2025 proposed CP:**
- §4.10.3.4 (259), p.57: "**If an EE is determined by an MA to be not working correctly**, the ECA, RA or supplementary Authorization Server shall change its status to 'blocked'." That is, the CP blocks honest-but-faulty devices by design. This is the strongest policy hook Paper 1 has.
- §4.10.3.5 (261), p.58: the ACA processes revocation "within 5 days".
- §4.10.4 (263), p.58: the MA processes reports if signatures, certificates, CRL and CTL are valid. It gives no evidence or decision rule.

### 2.4 ETSI and C2C-CC (EU side; context, not pre-emption)

- **ETSI TR 103 460:**
  - p.10: "Denigration of benign ITS stations cannot be circumvented (risk of false positive)."
  - p.21: level 1/2 checks "as introduced in the US NPRM and evaluated in the CV Deployment program's test sites e.g. in Tampa". The TR gives no Tampa results.
  - p.24: "the MA should be able to distinguish between false positive and genuine reports."
- **C2C-CC WP (2021), p.23:** "The testing of advanced misbehaviour detection applications in large-scale deployment projects or field-tests (FOTs) has started, but currently there is no sufficient evaluation results."

### 2.5 Park et al. 2024 (K-City), the paper the owner named

**IET ITS paper (park2024enhancingb):**
- **Full text not obtained.** It is gold OA, so this is a bot wall, not a paywall. Read it in a browser before submission.
- **What is known:**
  - Abstract: a combined local and global MBD system "to combat ... slander attacks, in which vehicles with legitimate certificates may be falsely accused, and sophisticated attacks targeting the global MBD system itself", "validated at K-City".
  - From search snippets: an Autocrypt system, HIL with an "SCMS server, a GMBD server, an RSU, and an OBU", OBU-equipped vehicles on the K-City track.
  - Also from a snippet: "Previous field tests, such as those conducted by the SCA project [6,7], operated under the assumption that a misbehaving reporting vehicle is always honest."
- **Assessment:**
  - Its threat is **malicious** false reports (slander), not benign triggers.
  - Its scale is a test track.
  - Its rule set is Korean or ETSI-style, not SM-MBR (inferred from the companion paper, below).
  - It does not pre-empt the benign-rate claim. It does pre-empt any claim to be "first to field-test local + global MBD with an SCMS" or "first slander defence at the GMBD".

**Companion OJITS paper (park2024enhancing), read in full:**
- The local checks are **CaTch**, "the publicly available implementations and operational parameters [23]", applied at RSUs "when two BSMs are received within three-time intervals".
- Evaluation: a 24 h simulation; the FPR is fixed at 10⁻³ by threshold choice.
- Linking across RSUs uses "real ID information, similar to techniques used within the SCMS".
- Its own limitation: fixed thresholds "may not directly translate to optimal performance in real-world scenarios".
- Its reference [4] for "if confirmed, the PCs ... are revoked" is ETSI TS 103 759, not SCMS Manager.

### 2.6 Kamel (CaTch, F2MD thesis): the closest generic pre-emption

- **PDF p.19:** ignoring the confidence range "may result in a high number of false positives in a realistic scenario. Therefore, we created CaTch".
- **PDF pp.71-72, Fig. 3.12** (printed p.64): "average number of reports by pseudonym received by the MA" for **Genuine** and Misbehaving nodes across pseudonym-change periods. "Our current simple MA is still based only on the number of received reports for a certain pseudonym."
  - This is already an end-to-end, in-simulation count of reports about genuine vehicles at an MA.
  - It is city-scale through LuST, with sensor-error models.
  - It is not US rules and has no revocation.
- **PDF p.92** (printed p.85): "the global MA should be designed to withstand a number of False Positive reports ... The effect of the local detection quality on the global MA should also be evaluated." This is a stated open item that Paper 1 answers for the US rules.
- **PDF p.114, §6.1:** the MA classifies each reported station as "Misbehaving ..., Faulty ..., Genuine (false positive)". The model is trained on 24 h LuST and tested on Paris-Saclay, with macro F1 up to 0.98 (Tables 6.2-6.5).
- **PDF p.129, §6.2:** Sybil attacks "cause the local vehicles to simultaneously report other genuine vehicles alongside the attacker".

### 2.7 Bißmeyer 2014: real-data benign rates, 12 years old

- **PDF p.53:** "tested and evaluated with 120 vehicles and 100 RSUs over a period of 76 days" (simTD FOT, no attacks).
- **PDF p.75:** "the outdoor tests with real vehicles produced a false-positive rate of ≈9.25%".
- **PDF p.76, Fig. 3.8:** 88.2% of those are maximum-transmission-delay violations and 6.2% communication-range violations. Plausible-movement violations, which use a **5 m** Kalman-prediction threshold (Table 3.3, p.73), are 1.8%.
- **PDF p.178:** movement-check FPs "can be kept in an acceptable range (i.e. ≈1.6‰)". On the central MEA: "Only if the detected misbehavior is confirmed by a specific number of independent reporters a reaction is initiated. Consequently, the false-positive rate at the nodes is only partly relevant for the final exclusion".
- **PDF p.180:** "the message-based false-positive rate exceeded the expectations. The main reason for this was the unreliable transmission of mobility data and its inaccuracy." The MEA "has only been evaluated in a proof-of-concept manner."
- **Implications:**
  - It pre-empts "real deployments show plausibility checks fire on honest vehicles".
  - It pre-empts the generic form of "radio/transmission effects, not GNSS, dominate benign triggers".
  - It leaves open the end-to-end exclusion count, the 1 m US threshold and US reporting policy.

### 2.8 Real US CV-pilot data, 2024-2026

- **Das, Griffith, Rathore 2026** (abstract; paywalled): faults (drift, hard-over) and attacks (colluding, replay) are **injected** into the Tampa BSM sample, then ML separates them. It reports no benign rate of any standard check on the clean data.
- **No other 2024-2026 paper was found** that runs SM-MBR, J2945/1 or ETSI TS 103 759 checks on THEA, Wyoming, NYC, SPMD or Utah BSMs and reports a benign rate.
- **Carie 2026 does not count:**
  - It is simulated (ns-3, 6-100 vehicles), with no SCMS, MBR, MA or CRL.
  - It reports per-instance FPR (0/1,584).
  - Its "~40 false bans per hour per lane" is an unsourced back-of-envelope figure. Do not cite it as evidence.

---

## 3. Which parts of the claim are taken

| Claim element | Status | By |
|---|---|---|
| Plausibility checks fire on honest vehicles in real traffic | **Taken** | bimeyer2014misbehavior (simTD FOT, 9.25% message FPR) |
| Transmission/reception effects drive benign triggers | **Taken (generic)** | bimeyer2014misbehavior p.180 |
| Reports about genuine vehicles counted at an MA, city-scale simulation | **Taken** | kamel2020detection Fig. 3.12 (F2MD, LuST) |
| Ignoring sender confidence inflates false positives | **Taken** | kamel2019catch; kamel2020detection p.19 |
| MA classifies genuine / faulty / malicious | **Taken** | kamel2020detection §6.1; rathore2023distinguishing; das2026distinguishing |
| Slander / false-report defence at a GMBD with an SCMS, field-validated | **Taken** | park2024enhancingb (K-City); 5GAA 2022 p.12 |
| Benign trigger rate of **SM-MBR v1.0** (RandomPosition 1 m, ConstantPosition, MaxSpeed, MaxAccel, BrakeAccel) on real or simulated data | **Open** | none found. Check 3 has a first real-data measurement. |
| Effect of SM-MBR §2.2 weights, quota and RSU-gated upload on which evidence reaches the MA | **Open** | none found |
| Benign triggers → RA shuffle → MA → LA linkage → CRL, with honest devices revoked counted, under the US SCMS | **Open, conditional on an assumed MA rule** | none found; no public US MA rule exists |
| "The deployed US system revokes honest vehicles" | **Not supportable** | the MA was human in the pilots; no production rule is public; the engine shows 0 with a realistic receiver |

---

## 4. The safe claim

> We give the first measurement of the benign trigger rate of the only published US misbehaviour-reporting rule set, SCMS Manager's CV-Pilot MBR v1.0 and its ASN.1 v1.1.
> - **On public CV-pilot BSMs:** by device class and time base.
> - **In a city-scale closed loop:** reporting, RA, MA, LA and CRL, with both normative readings of RandomPosition.
> - **The resulting MA load and wrongful-revocation surface:** across a family of MA decision rules, because no US MA rule is published and the pilots used human review.
>
> We show which receiver and device properties make honest vehicles indistinguishable from position attackers under that rule set, and what the MA can recover from fields it already receives.

**Words to drop or qualify:**
- "deployed" → "published" or "CV-Pilot".
- "revokes honest vehicles" → "would revoke under MA rule R, given trigger rate T".
- "first city-scale end-to-end false-positive study" → "first for the US SCMS chain" (Kamel and F2MD did a city-scale MA count).
- "first real-data benign rate" → "first for the US rule set" (Bißmeyer 2014).

**Must cite:** bimeyer2014misbehavior, kamel2020detection, kamel2019catch, park2024enhancingb, park2024enhancing, das2026distinguishing, rathore2023distinguishing, the 5GAA 2022/2024/2025 documents, ETSI TR 103 460 and twardokus2021targeted.

---

## 5. What it changes

**Paper 1:**
- **Lead with the rule-text ambiguity and the real data,** not the engine's 26.
  - v1.0 has velocity only; ASN.1 v1.1 has velocity and acceleration and a 0.1 s spacing; gpsDrift is carried in the report.
  - Check 3 shows honest real devices trigger at 0%, 4.5% or 44% by device class. That is new and is not pre-empted.
- **Rebuild the engine's honest-trigger model** on the device-class/time-base mechanism (check 3). Drop the raw-GNSS story.
- **Present revocation as a surface over MA rules,** since no US rule is public. Anchor it to the 5GAA 2025 CP (259) "not working correctly → blocked", which makes benign triggers a policy problem, not a hypothetical.
- **Read before submission:** Park 2024 IET (browser) and J3287 (purchase, or via an SAE member). Either may define US-flavoured checks or report K-City false-report numbers.

**Paper 2:**
- The benchmark's "wrongful revocation" label is genuinely new only because it is tied to the SCMS chain and counterfactual pairs.
- F2MD/LuST already produces genuine-vehicle report counts at an MA (Kamel Fig. 3.12), so the benchmark must not claim the first "false-positive at the MA" labels.
