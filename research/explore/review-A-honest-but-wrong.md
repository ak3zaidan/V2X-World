# Review A: "Honest but wrong" (benign GNSS error and defective device classes as misbehaviour evidence)

Reviewer role: senior PC member, **USENIX/NDSS VehicleSec 2027**, the venue with the closest fit (deployment-facing V2X security with an operator audience). The fallback is IEEE VNC, where the bar for novelty is lower and the audience includes the people who write the F2MD/VeReMi work. PoPETs does not fit, because the privacy cost is a secondary measurement. TDSC/TIFS would want a mechanism with proofs, not a policy measurement.

Date of review: 2026-10-18. Per the instructions, I did not open anything under `research/synthesis/`, `research/explore/` or `research/scrutiny/`.

---

## 1. Summary of the submission (as pitched)

The proposal claims three things:

1. Under the only published US MBR rules (SCMS Manager CV-pilot MBR v1.0), an honest fleet produces a large volume of misbehaviour reports.
2. Those benign reports crowd real-attack evidence out of the on-device store and the RSU-gated upload.
3. Per-sender MA rules give honest vehicles in urban canyons, or in a defective device class, revocation-grade evidence before the MA can attribute the cause.

It would then price, in bytes and in linkability, one extra report field (or MA-RA cooperation) that restores cause attribution. The evidence would come from the project's city-scale simulator with a standards-exact US SCMS.

## 2. Premise check: what holds, and what does not

| Pitch claim | Verdict | Evidence |
|---|---|---|
| SCMS Manager MBR v1.0 has a 1 m `gpsDrift` parameter | **True, but mis-described** | SM-MBR v1.0 §2.3.1.2.2 (PDF p.6), https://scmsmanager.org/wp-content/uploads/2020/01/Misbehavior-Report-and-Application-Specification-v1.0.pdf: "Two consecutive BSMs are inconsistent if [(position in the first) + (velocity in the first)*(time between)] differs from (position in the second) by more than this parameter", default 1 m. This is a **dead-reckoning residual between consecutive BSMs**, not a threshold on absolute GNSS error. |
| "Production receivers err by about 3 m in open sky", so the rule fires | **Non sequitur** | The 3 m figure is an *absolute* 68th-percentile error (Reid 2019, used in the engine's `crates/v2xw-mobility/src/gnss/gauss_markov.rs:21-24`). The rule only sees the *change* in error over about 100 ms. Absolute GNSS error is dominated by slowly varying components (the engine itself models a bias with τ ≈ 999.5 s, `gauss_markov.rs:33-36`). Automotive receivers also output filtered fixes. A 3 m absolute error says almost nothing about the 100 ms jump distribution, which is the only quantity that matters here. |
| Storage quota, weights and RSU-gated upload exist | **True** | SM-MBR v1.0 §2.2 (PDF pp.4-5): daily ×0.9 multiplier at 5 am Eastern, highest weight uploaded first, lowest weight evicted when over a device-specific maximum. §4 and the project's notes (`docs/design/scms-real/03-protocols-and-operations.md:299-300`) cover RSU-gated upload. |
| 5GAA MBD-Op says remediation should depend on cause | **True** | 5GAA, *Misbehaviour Detection for V2X: Operational Aspects* v1.0 (2024), §5.3.1, PDF p.13 lists "accidentally malfunctioning EE ... e.g., GPS sensor", "design error in HW or SW", "cybersecurity incident" and "misuse", https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf. The same document (p.12) says "How the MBMS determines the existence and source of a misbehaviour is out of scope", which genuinely supports the gap. Section 5.2.1 (p.12) also says that for "easily detectable" misbehaviour, "doing nothing ... may be the suitable response". A competent MA may therefore already ignore single-sender RandomPosition reports, which undercuts the threat. |
| "Every MA design judges one pseudonym at a time" | **Overstated** | Kamel et al.'s global MA work classifies Sybil *types* from report streams (kamel2019misbehavior, DOI 10.1109/uemcon47517.2019.8993045). DARE (haidar2020dare, DOI 10.1109/vtc2020-spring48590.2020.9128687) exists precisely to evaluate global MA algorithms. F2MD (kamel2020simulation, DOI 10.1109/tvt.2020.2984878) already discusses reacting to a faulty component, e.g. "a vehicle with a faulty sensor would re-calibrate upon a warning from the MA" (kamel2020simulation, PDF p.8). What is true is that no *published deployed* MA rule aggregates across senders by cause. |
| "Under the privacy split, the MA cannot see a sender's device class or GNSS conditions" | **Largely false** | (a) Every BSM's core data carries a `PositionalAccuracy` ellipse (SAE J2735 BSMcoreData; the engine's own belief type cites it at `crates/v2xw-core/src/belief.rs:36`). MBR evidence is the full `SignedBsmsWithCertificate` (SM-MBR ASN.1 v1.1; `03-protocols-and-operations.md:296`), so the MA already receives the sender's self-reported GNSS quality. (b) Providers may run separate ECAs and PCAs "per region, manufacturer or device type" (SCMS Manager Primer pp.4-5, recorded at `docs/design/scms-real/01-deployment-and-governance.md:170`). The issuer of the pseudonym certificate inside the evidence can therefore already reveal a coarse device class. (c) BSM content, such as vehicle size, Part II extensions and timing idiosyncrasies, is a well-known class fingerprint. The "one added field" may already exist, or may be one issuer lookup away. |
| "Neither pilot data nor VeReMi has these cause labels" | **Partly false** | F2MD ships "6 attacks and 9 faulty behaviors" (kamel2020simulation, IEEE TVT 2020, DOI 10.1109/tvt.2020.2984878, PDF p.2). VeReMi Extension (kamel2020veremi, DOI 10.1109/icc40277.2020.9149132) is built on F2MD's faulty-versus-attack taxonomy and a realistic sensor-error model. What *is* absent is per-cause labels for *environment* (canyon) and *device class*, plus an SCMS layer. |
| "The engine supplies GNSS error by location on real US maps" | **False today** | The engine builds every receiver's `GnssEnv` as `..GnssEnv::OPEN_SKY` with only the weather overridden (`crates/v2xw-engine/src/run.rs:3699-3702`). `SkyView::DeepCanyon`/`CanyonMitigated` and `faulty_sensor: true` appear only in unit tests (`gauss_markov.rs:929` begins `#[cfg(test)]`; uses at `:1123`, `:1201`, `:1219`). No building-geometry-to-sky-view mapping exists, so the 2-3 week estimate leaves this out. |

**The decisive technical problem:** the engine's GNSS model draws an independent white term of **σ = 1.6 m per horizontal axis at every 10 Hz estimate** (`gauss_markov.rs:29-30`, `:239`, `:541-560`). That value is the MathWorks `gpsSensor` default, which the code itself records as an assumed 1 Hz parameter (`:33-36`). Under that model, the residual between two consecutive BSMs has per-axis σ ≈ 1.6·√2 ≈ 2.26 m, so the SM-MBR 1 m test fires on about **91 % of consecutive BSM pairs from every honest car in open sky**, and on about 99.9 % with the deep-canyon ×10.1 scale (`crates/v2xw-mobility/src/gnss/common.rs:63-71`). My calculation: P(Rayleigh(2.26) > 1 m) = exp(−1/(2·2.26²)) = 0.907.

The proposal's headline ("honest fleets flood the MA") would therefore be produced by an uncalibrated noise parameter, not discovered. This is exactly the "check that cannot fail" pattern. Until the 100 ms jump statistics are calibrated against real OBU output, every downstream number (report volume, crowd-out, revocation exposure) is an artefact.

## 3. Does it matter (who changes what)?

Potentially yes, and this is the proposal's strongest point.

- **SCMS Manager** owns `gpsDrift`, the weights, multipliers and the quota (SM-MBR §2.1, p.4: the parameters are explicitly adjustable per (PSID, type, subtype)). A measured trigger-rate curve against `gpsDrift`, BSM interval and packet loss is directly actionable.
- **5GAA MBD4V2X** has an open item: cause determination is "out of scope" (MBD-Op p.12), yet remediation depends on cause (p.13), and the MBMS is told to watch for "similar misbehaviour ... in a different class of EEs" (p.13). A study that shows *how much* cause signal the MA can recover from evidence it already receives would feed straight into that work.
- **MA operators and OEMs** face a real recall-or-revoke decision (MBD-Op §5.3, pp.13-15).

The impact claim is weaker than pitched for two reasons. First, if the MA already discounts single-sender kinematic reports (MBD-Op p.12, "easily detectable"), honest vehicles are not revoked in practice, and the result becomes "wasted bandwidth and storage", a much smaller story. Second, the "revocation-grade evidence" threshold is the engine's own invented rule: k = 3 trusted reporters, 4 distinct seconds, a 3 s span and a 15 s window (`crates/v2xw-threat/src/ma.rs:1-12`, `:102-127`, `:244-246`). The US MA procedure is not public, so "N honest vehicles reach revocation grade" measures the authors' MA, not the deployed one.

## 4. Novelty (own search)

**Already done, and must be cited and positioned against:**
- Faulty versus malicious nodes as a revocation problem: Raya et al. 2007 (raya2007eviction, DOI 10.1109/jsac.2007.071006). Root-cause MBD: Ghosh et al. 2009 (ghosh2009misbehavior, DOI 10.1145/1614269.1614292). Rule-based faulty-versus-malicious mining: Rezgui and Cherkaoui 2011 (rezgui2011detecting, DOI 10.1109/lcn.2011.6115558).
- Sensor-fault versus attack classification on BSMs: Rathore et al. 2023 (rathore2023distinguishing, DOI 10.1109/mobisecserv58080.2023.10329092), and Das, Griffith and Rathore 2026 on **Tampa CV-pilot BSMs** with injected drift and hard-over faults (das2026distinguishing, DOI 10.1109/ccnc65079.2026.11366546).
- False positives caused by ignoring sender confidence: Kamel's CaTch ("confidence range tolerant" MBD), motivated by "a high number of false positives in a realistic scenario" when position confidence is ignored (Kamel PhD thesis, kamel2020detection, PDF p.19, https://hal.science/tel-03102396v1). This is the closest prior work. It already shows that sender-reported accuracy fixes local false positives. **The novelty must be that the deployed US rule does not use it.**
- Global MA evaluation and report datasets: DARE (haidar2020dare) and the MA Sybil-type classifier (kamel2019misbehavior). Report format and evidence: kamel2018misbehavior (DOI 10.1109/vnc.2018.8628407), which is the basis of ETSI TS 103 759.
- Newer benchmarks: VeReMi NextGen (hermann2026veremi, DOI 10.18725/oparu-60763) and VeReMi Extension (kamel2020veremi).
- Industry IP on GNSS faults inside MBD: Qualcomm US 12,470,954 B2 (priority 2022-03-16, granted 2025-11-11, https://patents.google.com/patent/US12470954B2/en) covers a UE that detects a fault in its *own* local V2X source (GNSS is the main example) and conditions its misbehaviour decisions on it. That is the reporter-side half of this proposal's "GNSS-quality field".
- Urban-canyon GNSS degradation in a real US pilot is documented. NYCDOT built "V2X-locate" RSU ranging because GPS in Manhattan was degraded or lost (NYCDOT CV Pilot case study, https://www.itskrs.its.dot.gov/sites/default/files/2025-03/case-study/NYCDOT%20CV%20Pilot%20Case%20Study_FINAL_508_v2_03042025_0.pdf).

**What I could not find (genuinely open, as far as my search went):**
- (i) A measurement of the *SCMS Manager MBR v1.0* rule set's benign trigger rate, on simulated or real data.
- (ii) Any analysis of the SM-MBR storage-weight, quota and upload policy, meaning crowd-out of attack evidence by benign evidence under constrained upload.
- (iii) MA-side *cross-sender* cause attribution (device class or location cluster) under the US SCMS privacy split, with its linkability price.

Searches covered the local catalogue (11,315 entries, title regex for faulty, root cause, MBR, J3287, 5GAA, false positive, GNSS), arXiv (export API: misbehavior ∧ faulty/GNSS/authority), Semantic Scholar and web search. OpenAlex was rate-limited during this review, so coverage is incomplete. SAE J3287_202408 and ETSI TS 103 759 content was not checked (J3287 is paywalled, see `03-protocols-and-operations.md:28`). Either may already define a cause or quality field, and the authors must rule that out before claiming (iii).

Net: items (ii) and (iii) are novel. Item (i) is novel only as a measurement of a specific deployed parameter. The broader "benign faults look like attacks" framing is 15+ years old and must not be the headline.

## 5. Threat model and premise soundness

1. **Wrong error statistic.** Section 2 covers this. The proposal must model and validate the *100 ms innovation* distribution of OBU-reported positions (multipath switches, satellite set changes, RTK or SBAS mode transitions, filter resets), not absolute CEP.
2. **The more likely benign trigger is missing.** The SM-MBR test uses the first BSM's velocity only (no acceleration or yaw term) and "time between" consecutive *received* BSMs. During a turn, the residual grows roughly as ½·v·ω·Δt². With packet loss or J2945/1 congestion-controlled intervals, Δt grows from 0.1 s to around 0.5-1 s, and the residual passes 1 m from kinematics alone. For example, at v = 10 m/s, ω = 0.3 rad/s and Δt = 1 s, the residual is about 1.5 m. This is the closed-loop, radio-coupled effect the engine is actually good at, and the pitch does not mention it.
3. **The "cannot see" assumption is weak** (Section 2). The accuracy ellipse is already in the evidence, and issuer CAs may encode device class. The real research question is narrower: *are those signals sufficient and honest, and what does the MA gain by aggregating them across pseudonyms?*
4. **Adversarial abuse is not addressed.** If the MA forgives "canyon" or "known-defect class" evidence, attackers will claim poor accuracy or operate in canyons. Any proposed field must be analysed as an evasion channel, or the proposal recreates the relay-attack reason ObsImplausible is "default off" (SM-MBR §2.3.1.4; `03-protocols-and-operations.md:295`).
5. **The "defective class" model is invented.** Nothing in the engine or the literature parameterises realistic OEM firmware defects (prevalence, fault signature, trigger conditions). Results for that axis would be conditional on a free parameter.

## 6. Would simulation evidence convince me, and what validation I would require

As pitched, no. The headline is fixed by the white-noise σ. I would require:

- **V1, real-data calibration (non-negotiable).** Run the exact SM-MBR v1.0 checks (RandomPosition at 1 m, ConstantPosition, MaxSpeed, MaxAcceleration, BrakeAcceleration) over *real OBU-logged consecutive BSMs*, using the public THEA (Tampa) and Wyoming CV-pilot BSM data on USDOT ITS DataHub (https://data.transportation.gov). Report the benign trigger rate by speed, yaw rate, Δt and the reported `PositionalAccuracy`. Das et al. 2026 already use the Tampa BSMs for exactly this kind of work, so feasibility is established. The engine's GNSS innovation model must then be fitted to reproduce that distribution, with held-out validation (for example, fit on Wyoming and test on downtown Tampa).
- **V2, fault injection.** Show that the pipeline produces *zero* RandomPosition reports with a noiseless GNSS and perfect radio, and the analytic rate under a known noise process. Without this, the measurement cannot be trusted to go red.
- **V3, sky-view by location.** Implement and validate a building-geometry-to-SkyView mapping against a published urban drive test (for example the Wen and Hsu data the engine already cites, `common.rs:63-71`), and report sensitivity to it.
- **V4, MA-rule sensitivity.** Because no deployed MA rule is public, present exposure as a surface over (k, persistence, window), plus at least one rule that uses the accuracy ellipse (CaTch-style). Do not report a single "N honest vehicles revoked".
- **V5, quota realism.** Store size is "device-specific" (SM-MBR p.5). Sweep it, and justify RSU density from a real deployment (for example THEA RSU count per corridor).

## 7. Strongest reasons to reject (current form)

1. The core quantitative claim would be an artefact of an uncalibrated 10 Hz white-noise term (σ = 1.6 m per axis, a 1 Hz MATLAB default) that makes the 1 m rule fire about 91 % of the time in open sky (`gauss_markov.rs:29-36`, `:541-560`).
2. The premise conflates absolute GNSS error with consecutive-message residual. The cited 3 m and canyon figures do not support the claimed report volume.
3. The "MA cannot see GNSS quality or device class" premise is contradicted by the standard BSM accuracy field inside the evidence and by provider CA structure (J2735; SCMS Primer pp.4-5). The "one added field" may already exist.
4. "Revocation-grade" depends on an engine-invented MA rule (`ma.rs:1-12`), not on any deployed US procedure.
5. Core claims rest on capabilities the engine does not have today: location-dependent GNSS (`run.rs:3699-3702`), an active faulty-sensor class, the spec's own RandomPosition check (the engine's `positionJump` is a confidence-normalised detector, `crates/v2xw-threat/src/detect.rs:1046`, merely *labelled* with a 1 m drift at `crates/v2xw-engine/src/phase2.rs:6641-6643`), and a defect-class model. The 2-3 week estimate is not credible once V1-V3 are included.
6. The prior work on faulty versus malicious nodes and on confidence-tolerant MBD (CaTch, F2MD faulty behaviours, Rathore/Das on Tampa BSMs) is not acknowledged.

## 8. What would make it an accept

Reframe and narrow the work to the parts that are novel and evidence-backed:

1. **Primary result from real data:** "The benign trigger rate of the deployed US MBR rules, measured on CV-pilot BSMs." This is a direct, checkable contribution to SCMS Manager parameter-setting that no simulation can be accused of inventing.
2. **Engine result, calibrated to (1):** the *radio-coupled* part that real logs cannot give. How do packet loss, congestion-controlled intervals and turning traffic inflate the trigger rate? How does the weight, quota and RSU-upload policy crowd out attacker evidence, at realistic RSU density and with injected attacks that have ground truth? This uses the engine's real strength, the closed loop of traffic, radio and SCMS, and nobody has analysed SM-MBR §2.2.
3. **MA attribution:** test whether *existing* evidence fields (the accuracy ellipse, the issuer CA, BSM content fingerprints) let a cross-pseudonym MA cluster benign causes. Only then price any *additional* field, and include the evasion analysis (an attacker claiming poor accuracy).
4. **Show it can go red:** the zero-noise null runs and the analytic check in V2.
5. **Report sensitivity surfaces,** not single numbers, for the MA rule and the store size.

With (1) and (2) solid, I would take this at VehicleSec as a measurement paper. (3) is a strong bonus, but needs J3287 and TS 103 759 checked first.

## 9. Score

**Major revision, borderline Reject.** Confidence **4/5**.

The question is real: SM-MBR's 1 m rule and quota policy are unstudied, and 5GAA explicitly leaves cause attribution open (MBD-Op pp.12-13). As pitched, though, the headline result is predetermined by an uncalibrated simulator parameter, two premises are factually weak (the error statistic, and what the MA "cannot see"), and the novelty framing ignores 15 years of faulty-versus-malicious and confidence-tolerant MBD work. If the authors anchor on real CV-pilot BSMs and use the engine for the radio-coupled crowd-out and MA-attribution questions, this becomes a solid, adoptable paper.
