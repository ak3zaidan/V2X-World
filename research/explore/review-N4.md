# Review N4: "The compromised intersection: an RSU that lies, filters the reports against it and relays the CRL that should revoke it"

**Reviewer role:** senior PC member, **VehicleSec** (the NDSS-co-located Symposium on Vehicle Security and Privacy). It is the best fit: an infrastructure-insider threat on V2I with simulation evidence and a standards ask. USENIX Security, NDSS and CCS would want a real-device attack or a deployed-system measurement, which this does not have.

**Score: Reject as pitched (would move to Major Revision if reframed as in section 7). Confidence: 4/5.**

---

## 1. Summary of the claim

A compromised signalised-intersection RSU attacks on three surfaces at once:
- it broadcasts validly signed false SPaT/MAP;
- it drops the misbehaviour reports (MBRs) that accuse it;
- it controls CRL delivery to nearby vehicles.

The study would measure time-to-neutralise, the share of reports suppressed, incidental bypass, and vehicles exposed before revocation, with and without a cellular path. The hypothesised headline: without path diversity, a compromised RSU can delay its own revocation indefinitely in sparse areas.

## 2. Does it matter? Who changes what?

- **The topic is real and timely.** The SCMS Manager has drafted an *Intersection Validation and Certificate Issuance Policy* and an *Intersection Validation Misbehavior Management* document (titles listed by Utah DOT, https://transportationtechnology.utah.gov/what-were-learning/; marked "Drafts (Jul 2025, not yet policy)" in docs/design/scms-real/01-deployment-and-governance.md line 81). A Utah SMART-grant deck says this work would "initiate development of V2I misbehavior detection procedures" (https://engineering.virginia.edu/sites/default/files/Connected-Vehicle-PFS/Annual%20Meetings/Spring%202024%20-%20Madison%2CWI/CV%20PFS%20Utah%20SMART%20update%2004082024.pdf, per search extract). So there is an active rule-making venue. That is the strongest argument for the paper.
- **RSU compromise is plausible.** The units are physically exposed: Phrack 72 article 16, "Roadside to Everyone (R2E) Phase 1: Physical & Local Vulnerabilities in (C)V2X RSUs", found Kapsch units secured by "just four screws", with keys and certificates in EEPROM (https://archives.phrack.org/issues/72/16.txt). Traffic-signal networks have a history of trivial compromise (Ghena et al., "Green Lights Forever", USENIX WOOT 2014, https://www.usenix.org/conference/woot14/workshop-program/presentation/ghena; this citation is from my own knowledge, not the corpus; see also ning2019understanding, doi:10.1007/978-3-030-22038-9_8).
- **But the proposed adopter ask is mostly already met by the deployed architecture.** "Reports and CRLs must travel by a path that avoids the RSU" is the requested change, and three facts weaken it (section 4):
  - US CRLs come from the RA, not by RSU broadcast.
  - Reports are store-and-forward.
  - About 93% of North American vehicles sold in 2024 had OEM cellular telematics (Berg Insight figure as reported at https://www.collisionrepairmag.com/news/collision-repair/market-trends/article/15751449/ ; this is a secondary source).

  The ask is therefore close to "keep doing what you already do". ITE/AASHTO/NEMA, the USDOT RSU specification and J3287 editors would read the result as confirmatory, not as action-forcing.

## 3. Novelty (searched myself)

- **Corpus (11,315 entries).** These cover RSU-misbehaviour detection by trust or feedback:
  - abhishek2019detecting, doi:10.1109/iccchina.2019.8855952
  - abhishek2021drive (DRiVe), doi:10.1109/jiot.2021.3097809
  - nagaraju2024comparative, doi:10.1109/icves61986.2024.10928101 (a survey)
  - nagaraju2025trust, doi:10.1109/iwcmc65282.2025.11059494
  - arXiv 2208.05680 (trust-based malicious RSU detection)

  On the SPaT side: novo2026spade (arXiv 2609.02741, vehicle-side SPaT attack dataset in MOSAIC, no PKI); chen2018exposing (I-SIG, NDSS 2018, doi:10.14722/ndss.2018.23222); chen2025human (falsified red-light countdown, doi:10.1016/j.trf.2025.103369); and xu2025demo (MitM toolkit at connected intersections, IEEE SPW 2025, doi:10.1109/spw67851.2025.00030). Older work proposed revoking compromised RSUs by cheap CRL broadcast because RSUs are few (search extract; the general CRL-distribution line is papadimitratos2008certificate and laberteaux2008security).
- **arXiv** (export API: "malicious RSU", SPaT+attack, "roadside unit"+compromised+revocation, "misbehavior report"+RSU) returned only 2208.05680, 2609.02741 and 2609.21206 (DoS, not relevant). **OpenAlex** queries on RSU plus report suppression or CRL withholding returned nothing on point.
- **Patents are close prior art for the detection half.** US 11875677, "Detection and reporting of misbehavior in vehicle-to-infrastructure communication" (https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/11875677), checks SPaT against the behaviour of observed vehicles and reports it.
- **Verdict on novelty.** I found no paper that combines false SPaT, suppression of MBRs about the RSU and control of CRLs in one PKI-aware model. The *combination* is novel. But each part is either known (false SPaT, RSU trust detection), contradicted by the standards (CRL relay, section 4b) or a near-tautology (path control, section 4d). Novelty by conjunction is weak at a security venue unless the conjunction produces a non-obvious result.
- **Scoop risk.** The SCMS Manager drafts on intersection validation misbehaviour management could not be read (the Google Drive links require sign-in). They may already specify the reporting and revocation path for RSUs. The authors must obtain and address them.

## 4. Is the threat model or premise sound? (main problems)

**(a) "Filters the reports against it" is not realisable against the standard report format.**
- The engine's own standards-exact MBR is "signed, then encrypted to the MA" (crates/v2xw-proto/tests/mbr.rs:448; crates/v2xw-proto/src/scms/run/report.rs:36).
- Yet the RSU attack reads plaintext `report.subject_cert_digest` to decide what to drop (crates/v2xw-threat/src/attack_rsu.rs:377-399).
- The sibling `PoisonForwardedReports` ("re-target … at innocent subjects", attack_rsu.rs:71) is impossible against a reporter-signed, MA-encrypted report.

A real compromised RSU can only drop *all* SCMS-bound traffic, or drop it by timing. Both are observable to the RA/MA as a volume anomaly at that gateway, and both also starve the top-ups of RSU-dependent OBUs (the NYC pattern in 01-deployment-and-governance.md line 267). The threat model must be rewritten to what the adversary can actually see.

**(b) "Relays the CRL that should revoke it" does not match the deployed US system.**
- The design notes record that CRL broadcast by RSU or satellite is "Not used; CRLs come from the RA" (01-deployment-and-governance.md line 291, citing the USDOT SCMS Technical Primer FHWA-JPO-19-775). Devices fetch a composite CRL on every RA connection, and distrust everything after 2 weeks overdue (00-gap-analysis-and-plan.md line 112).
- RSU application certificates last **1 week + 1 h**, are requested weekly over backhaul and are revoked by hash on series 3 (00-gap-analysis-and-plan.md line 106; 01 line 222). Once the MA decides, the RA simply refuses renewal and the RSU's signing ability dies within about one week, *whatever happens to CRL delivery*.

So "delay its own revocation indefinitely" can only mean "never be *detected*". That is a different and much narrower claim.

**(c) There is no standard report to suppress.**
- The only published US MBR spec covers PSID 0x20 (BSM) only (01-deployment-and-governance.md lines 92-94, citing the MBR & Application Spec v1.0 §1.0-2.3).
- The ETSI TS 103 759 ASN.1 imports only `SaeJ3287AsrBsm` from J3287 (https://forge.etsi.org/rep/ITS/asn1/mrs_ts103759/-/commit/2b06a2f054865a2516854b79883933478d39e513). I could not see any SPaT/MAP-specific report content. J3287_202408 is paywalled (https://saemobilus.sae.org/standards/j3287_202408-v2x-misbehavior-reporting), so this is not fully verified.

The accusing reports the paper's chain depends on therefore have no standard format today. That gap is more important than path diversity, and the pitch never mentions it.

**(d) The headline is close to a tautology.** "If every path from witness to authority passes through the adversary, the adversary controls detection" holds by construction.
- What matters is *how often* that condition holds under realistic mobility, cellular attach rates and store-and-forward upload: reports are stored, weighted and uploaded at the next connection (01 lines 96-97).
- A vehicle that passes a lying intersection and later reaches any other RSU, or its own TCU, bypasses the attacker. The answer is set almost entirely by input assumptions (cellular share, upload policy, RSU density) that the simulator echoes rather than discovers.

**(e) Attribution is ignored.** Wrong SPaT at an intersection can come from:
- a compromised RSU;
- a compromised or faulty signal controller or TMC feed passing through an honest RSU;
- a stale timing plan;
- honest timing inaccuracy. CAMP published a whole assessment of SPaT accuracy for RLVW, listed on the Utah page above.

The honest-error base rate is likely to dominate. Revoking an RSU certificate takes every V2I application at that intersection offline, and is the *wrong* remedy for a controller compromise. The MA's real decision problem (false revocation of infrastructure against harm from false SPaT) is absent from the pitch.

**(f) The operator is missing.** A state DOT has authenticated backhaul to the RSU and can power-cycle it, reflash it or take it out of service. That path is plausibly faster than any SCMS revocation. "Time to neutralise" must include it, or the study is measuring the slower of two remedies.

## 5. Would simulation evidence convince me? What validation I would require

**What the engine can produce today.** Not the core result:
- The engine's own model card says false SPaT/MAP/CRL/CTL are "declared rather than rendered", and that "no safety application and no detector reads SPaT today" (crates/v2xw-threat/src/attack_rsu.rs:568-574).
- The TS 103 759 class-3 checks cannot cross-check a false SPaT because "crate::obs carries no signal-state belief" (crates/v2xw-threat/src/ts103759.rs:1370-1378).
- The false-SPaT magnitude is `TodoCalibrate`, "no anchor for a fabricated signal phase" (attack_rsu.rs:490-497).

Phase 1 as pitched would therefore suppress reports about *BSMs*, not reports that accuse the RSU. The proposed "phase 2, 1-2 weeks" (GLOSA consumes received SPaT) does not add the missing piece, which is a vehicle-side SPaT detector feeding an MBR. Building and validating that detector is the actual research and is not scoped.

**Validation I would require:**
1. **A SPaT/MAP detector with a measured ROC.** Cross-validate it on SPADE (arXiv 2609.02741) or on real SPaT logs. Report false positives at honest intersections under realistic SPaT inaccuracy, with error rates calibrated to the CAMP SPaT-accuracy assessment.
2. **Report-path parameters from deployment evidence, not chosen.** These are the cellular attach rate, store-and-forward upload behaviour (MBR spec weighting and eviction) and the RSU/RA connectivity mix (NYC RSU-gateway versus OEM TCU). Run sensitivity sweeps over them, and show which conclusions survive across the plausible range.
3. **Fault injection proving that every metric can go red.** For example, an honest RSU must give 0% suppression and a cellular-only fleet 100% bypass. Otherwise "share suppressed" cannot be distinguished from a broken counter.
4. **A threat model limited to what the RSU can observe** (encrypted MBRs, TLS to the RA), with the plaintext-subject filter removed or justified.
5. **Comparison against the operator remedy and the 1-week certificate-expiry bound,** not only against "no cellular path".
6. **One hardware or testbed anchor** if aiming beyond VehicleSec: for example, a real 1609.2-signed false SPaT accepted by a commercial OBU, or an RSU's IP-gateway behaviour on SCMS traffic.

## 6. Strongest reasons to reject

1. The premise contradicts the standards it claims to be exact to: encrypted MBRs, RA-delivered CRLs, and 1-week RSU certificates refused at renewal (section 4a-b).
2. The headline is near-tautological, and its quantitative form is set by assumed inputs (section 4d).
3. The engine cannot yet produce an accusing report: there is no SPaT detector and false SPaT is not rendered (section 5). The evidence chain is broken at its first link.
4. The attribution and false-revocation problem, which is the hard and policy-relevant part, is absent (section 4e).
5. The adopter ask is largely satisfied already by cellular TCUs and store-and-forward (section 2).

## 7. What would make it an accept

Reframe it as **"Misbehaviour management for signed infrastructure: detecting, attributing and revoking a lying intersection under the US SCMS"**, with these contributions:
- **C1, a standards-gap analysis.** No US or J3287-visible report type exists for SPaT/MAP, and RSU revocation runs through series-3 hash CRLs plus renewal refusal. Map exactly what an MA can and cannot do against an RSU today, and engage the SCMS Manager intersection-validation drafts.
- **C2, a vehicle-side SPaT/MAP plausibility detector and report format** that fits J3287 (an AsrSpat proposal), evaluated for false positives against honest SPaT inaccuracy.
- **C3, a closed-loop measurement in the engine** of the decision trade-off. On one side: vehicles exposed to false SPaT, and GLOSA/RLVW harm, before action. On the other: the mobility and safety cost of revoking an honest-but-inaccurate intersection. Sweep over penetration, cellular share and RSU density. Path diversity becomes one realistic variable, measured as how many reports *could* be suppressed by a gateway-only adversary, instead of the headline.
- **C4, attribution.** Can the MA tell a compromised RSU from a faulty controller feed, using reports from several intersections and vehicles? This is the novel question, and it uses the engine's ground truth.

With C1-C3 done well and C4 at least attempted, I would rate it a VehicleSec accept and a credible journal article (IEEE T-ITS or TDSC). The engine's closed loop of traffic, radio, SCMS and MA, with ground truth, is exactly what C3 and C4 need. That is a better showcase of the engine than the path-diversity sweep.

## 8. Minor points

- Cite SPADE as an arXiv preprint (v2) and do not overstate it as "vehicle-side detection without PKI only": it is a dataset paper.
- I-SIG (chen2018exposing) is BSM spoofing into signal control, so it is orthogonal and not a baseline.
- "Area density" should be defined as RSU density *and* the density of independent connectivity (TCU share). They are different variables.
- Report time-to-neutralise as a distribution with a censoring rule (never detected within the horizon), not a mean.

---
*Search notes: the corpus was searched by title regex over research/catalog.jsonl, with summaries read for 10 key ids. Also searched: the arXiv export API (four queries, 3 s spacing), OpenAlex (four queries) and WebSearch (four queries). I could not access the SCMS Manager drafts (Drive sign-in), the J3287 full text (paywalled) or the CAMP SPaT accuracy report (Drive).*
