# Hostile critique of Direction 1, "Framed and revoked"

Date: 2026-10-17. Role: the harshest USENIX Security reviewer. Target: §1 of `top-three-directions.md`, read together with `eval-B-revocation.md` and `eval-traction.md`. Ids resolve to `summaries/<id>.md`. **NEW** marks a source not in `catalog.jsonl`.

**Verdict: keep with changes, and the changes are major.** As written, the paper would be rejected. Its MA model is a strawman, two of its five claims are already stated in the literature, and a third rests on a misreading of the 5GAA paper. One core does survive: *which kinds of misbehaviour evidence can frame an honest vehicle, at what attacker cost, and what corroboration rule stops it*. That core is standards-anchored and decision-relevant, and it suits the engine. If the reframe in §4 is not adopted, replace the direction.

---

## 1. Kill search: prior work the direction missed

| Source | What it already does | What it kills |
|---|---|---|
| **kamel2020detection** (Kamel PhD thesis 2020, p56; full text) | F2MD ships an "MA Stress" attack: "sending a large number of fake reports … contain[ing] the identities of the attacker's neighboring vehicles. The attacker could choose to change its identity for every report." | Sybil fake reporting with rotating pseudonyms already exists as an attack in the open-source F2MD simulator (kamel2020simulation). RQ1.1's "Sybil reporter" attacker role is not new. |
| **Intel patent US12489779B2** (NEW, priority 2021-09-23), https://patents.google.com/patent/US12489779B2/en | Detects "a slandering attack against other vehicles". Requires the misbehaviour to be "reported by enough independent observers … with different pseudonyms". Asks the linkage authority to "determine if two or more pseudonyms are linked to the same entity". Requires "an evidentiary threshold for a misbehavior type". | The defence family of RQ1.3 (reporter linkage through the LA, plus independent-observer corroboration) is a published industrial design. |
| **Qualcomm patent US12260747B2** (NEW, priority 2022-07-05), https://patents.google.com/patent/US12260747B2/en | An attacker's MCM falsely claims that another vehicle misbehaved. The MA identifies the true source by cross-checking reports and asking an LA whether identifiers are linked. | Shows that MA-side slander defences are being designed per message type. |
| **5GAA MBD white paper 2022**, p12 (NEW; full text read), https://5gaa.org/content/uploads/2022/07/5GAA-Misbehaviour-detection-Final.pdf | Slander attacks "may lead to the victims' certificates being wrongfully revoked … combining with Sybil attacks to appear as multiple coordinated reporters". It also says "reporter accountability may help mitigate against slander attacks … balanced with concerns that reporters' privacy is not significantly compromised". | RQ1.3 (the reporter-privacy price) is industry's own stated open problem. Measuring it is new; the insight is not. |
| **brecht2018security**, p7 (full text) | "Multiple certificates valid simultaneously … would enable masquerading as multiple devices by compromising a single device (the so-called Sybil attack)". The batch size is at least 20 per week. | Contribution 2's "policy-independent bound" (H1a) is this sentence restated (see §2.2). |
| asavisanu2025cats, p6 (full text) | Defends against "downvote collusion … falsely banning a benign vehicle". "The SA always process[es] votes and revocations with actual identities". | The trade "resolve reporters to their real identity to stop Sybil framing" has been built and evaluated, with time to ban, at city scale. |
| **simplicio2018privacy**, simplicio2019acpc, **correia2023range** (CCS 2023) | Temporary linking or revocation of SCMS pseudonyms without permanent tracking; activation-code revocation that publishes no seeds; backward unlinkability on revocation. | H2's "privacy payload" is a known property of linkage-seed CRLs, and published mitigations exist (§2.3). |
| zoccoli2023vanets, escher2021well, wiedersheim2010privacy | Pseudonym changes alone are easy to track from kinematics. | H2 must be measured as the *marginal* privacy loss over kinematic tracking, which the plan never mentions. |
| **arXiv 2606.20215** (NEW, 2026), "GNSS Spoofing Threat for V2X communications" | SDR spoofing of real Commsignia OBUs makes their CAMs carry implausible values (≈20 m/s² acceleration, off-road positions). | The building block of a *credible* framing attack (§4), which the direction does not consider. |
| **ISS Misbehavior Authority Service** (NEW), press release 2019-01-07, https://ghs.com/news/20190107_CES_ISS_misbehavior_authority_service.html; TFHRC operational test 2020, https://www.automotiveworld.com/?p=309776 | A commercial US MA exists. It was tested end to end (OBU/RSU report → MAS → CRL) at Turner-Fairbank with Noblis, Qualcomm, Savari and Siemens. | "Real MAs are unpublished" is only half true: one exists, and ISS said it would publish its detection criteria and report format to vendors. A reviewer will ask why the paper did not model it. |

Searches that found nothing new:
- **arXiv.** The API works over HTTPS; plain HTTP failed TLS, and one burst got HTTP 429. Queries: slander; false accusation with revocation; misbehaviour reports; bad-mouthing with vehicular; collusion with revocation; trust management with false reports. Nothing beyond VeReMi, the threshold VPKI paper (2308.02711), the MCM/CPM standardisation notes (2211.02579, 2112.02184) and IPEK (2604.07532, already in the catalog).
- **OpenAlex and web**, on framing plus SCMS; theses on "misbehavior authority" plus false reports; VehicleSec, NDSS and USENIX 2024–26. Nothing new beyond the table above.

**No end-to-end SCMS framing study was found.** The gap is real but narrow: an execution with numbers, not an idea.

## 2. Do the key assumptions hold?

### 2.1 The MA model is a strawman (fatal as written)

Every normative and industry description of the MA is **proof-based**. Reports carry the accused vehicle's own signed messages, and the MA re-verifies them.
- ETSI TS 103 759 v2.1.1 §4: an MR "shall include evidence … so that the MA is able to verify the observations independently. This evidence shall include the original received message". Full text: https://www.etsi.org/deliver/etsi_ts/103700_103799/103759/02.01.01_60/ts_103759v020101p.pdf
- C2C-CC white paper 2092, p24: "the MA should be able to recompute the same misbehaviour checks". https://www.car-2-car.org/fileadmin/documents/General_Documents/C2CCC_WP_2092_MisbehaviourDetection_and_Reporting_V1.0.pdf
- brecht2018security p16: a report includes "reported … BSMs, associated pseudonym certificates, as well as the reporter's pseudonym certificate and corresponding signature".
- kamel2020detection p103 says the same.

Under a proof-based MA the number of reporters is irrelevant when the evidence is the victim's honest signed messages. RQ1.1's "k insiders" and "p pseudonyms" axes, and H1a, therefore measure an MA that no standard describes.

The only opening for count-based framing is the USDOT factsheet's loose wording ("devices that have crossed a specific misbehavior threshold", https://www.its.dot.gov/factsheets/pdf/V2X_Security_factsheet.pdf). That is too thin to carry a headline result.

### 2.2 H1a is a tautology, not a result

"w distinct certificates are defeated by one insider holding at least w concurrent pseudonyms" follows from the definition. It is stated in brecht2018security p7, in liu2010limits (as cited by Bißmeyer; no full text found this session), in 5GAA 2022 p12, and as the design rationale of the Intel patent. The direction's own §1 calls it "a hypothesis until proved". A PC member will call it obvious.

### 2.3 H1b (framing by jamming alone) collapses under a proof-based MA

twardokus2021targeted (full text, https://rahbari.csec.rit.edu/papers/V2Verifier_ICC21.pdf) attacks "a basic misbehavior detection scheme … [that] decrements … reputation by one point" per unverifiable signature (p5). The authors themselves say the attack works against schemes "that rely solely on … 1609.2 signature failures".

A message whose signature does not verify is not attributable to the certificate holder, because certificates are public. It cannot be MA-verifiable evidence. H1b holds only for an MA that counts raw signature failures. No standard specifies one, and J3287 could not be checked because it is paywalled.

### 2.4 H2 (wrongful revocation as a privacy weapon) is designed-in, and its marginal harm is unmeasured

Publishing the linkage seeds revokes "all future device certificates" by design (brecht2018security p12). Linkability after revocation is the intended price, and simplicio2018privacy, ACPC and correia2023range already offer revocation without permanent linkage.

Two things are missing:
- **A baseline.** How much does CRL linkage add over kinematic tracking (zoccoli2023vanets, escher2021well)? The plausible answer: little for a dense listener, and a lot for linking a vehicle across days through coverage gaps. That has to be measured, not asserted.
- **The deployed revocation path.** The 5GAA 2025 certificate policy allows ACPC certificate locking and enrolment "blocking" (§4.10.3.4–5, https://5gaa.org/content/uploads/2025/07/credential-management-supporting-v2x-commercial-deployments.pdf), and neither publishes seeds. Whether deployed MAs use the linkage CRL at all determines whether H2 exists.

### 2.5 Contribution 4 misreads 5GAA 2024

The quoted sentence is real (p4): "V2X systems being deployed have not yet specified the conditions…". But the next sentence says the paper "attempts to describe what conditions might be used". Its §6–7 do give a severity rating (impact, detection difficulty, footprint and duration) and a mapping from remediation to misbehaviour. Its conclusion says it provides "the conditions (misbehaviour classes) under which each of the remediation options may be applied".

Two more corrections:
- The ladder is R0 evaluation, R1 update and R2 suspension; under R2 come ST0–ST3 (not ST0–ST2), with reinstatement classes RC1–RC4.
- The paper never mentions false reports. The keywords slander, Sybil and false report get 0 hits.

Copy: https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf. So "the first evaluation of the 5GAA options" must become "an evaluation of the 5GAA §7 mapping under adversarial reports". That is still fine, and it is honest.

### 2.6 The time-to-neutralise decomposition is fixed by policy text, not discovered

The 5GAA 2025 certificate policy for US-style commercial deployments sets these service levels:
- the ACA "shall process the revocation request, under normal conditions, within 5 days";
- CRLs are published "within 24 hours";
- ECA blocking happens "within 5 days" (§2.2 (49), §4.10.3.4–5).

The decomposition is therefore arithmetic, and H3′ (CRL and service-level stages dominate) is almost certainly true before any run. Contribution 5 adds little. The "business-day" service level in the direction is uncited; the policy text says days and hours.

### 2.7 The victim's safety cost will probably be close to zero

One honest vehicle dropping out of V2X awareness at today's penetration rates is a tiny safety change. Paired counterfactuals will measure it precisely, and it will be near zero. That is a fine null inside Direction 2, but it cannot be a headline here.

### 2.8 Venue fit

USENIX Security favours attacks on real systems. A simulation-only attack on an assumed MA, while a real MA service exists (ISS MAS), invites "not a real system". VehicleSec, VNC or IEEE TDSC fit better unless the MA model is validated with an operator.

### 2.9 Not checked in full

- park2024enhancingb: gold OA, but the Wiley PDF returned 403. The abstract confirms slander defences in a K-City SCMS deployment, so it is the most dangerous unread overlap.
- SAE J3287 (paywalled).
- liu2010limits (no copy found).
- ETSI TS 103 759 v2.2.1 (2026-01); only v2.1.1 was read.

## 3. Would anyone change a decision? Is a null publishable?

- **As written: no.** Operators would dismiss witness-count and signature-failure results as not their MA. Standards editors already know about Sybil reporting and the reporter-privacy trade-off (5GAA 2022, the Intel patent). A null result here ("framing needs insiders and the LA can link them") restates the SCMS design. It would not get into a top venue.
- **Reframed (§4): yes, for named readers.** The J3287 and TS 103 759 editors could use, per detector, which evidence classes are framable and the minimum corroboration rule. TS 103 759 is under active revision (v2.2.1, 2026). The ISS MAS and the SCMS Manager could use the same table as a policy input. Its null form, "only Class 4/5 detectors are framable, and two LA-distinct reporters close it at a query cost of q", is still a short standards-input paper (VehicleSec, VNC).

## 4. Fatal versus fixable, and the fixes

| # | Issue | Fatal? | Concrete fix |
|---|---|---|---|
| 1 | The MA is modelled as witness counting, which no standard describes (§2.1) | **Fatal as written** | Model a proof-based MA that re-runs the TS 103 759 Annex D detectors on the evidence. Parameterise only what the standards leave open: how to weigh Class 3/4 observations that the MA cannot re-verify, the corroboration rule and the LA query budget. |
| 2 | The H1a bound is a tautology (§2.2) | Fatal as a contribution | Drop it as a claim. Keep it as one sentence motivating reporter linkage. |
| 3 | H1b (jamming) targets a naive reputation scheme (§2.3) | Fatal as a headline | Replace it with framing that produces *valid* evidence: (a) GNSS spoofing of the victim, whose own signed BSMs then fail Class 1/2 checks (arXiv 2606.20215); (b) relaying the victim's genuine BSMs out of range, which triggers the Class 3 detector "position outside of own communications coverage area" (TS 103 759 Table D.1); (c) Class 5 conflicts, where the attacker's pseudonyms send positions that collide with the victim's; (d) Class 4 slander based on sensor claims that only an insider can make. |
| 4 | H2 has no baseline and may not exist on the deployed path (§2.4) | Fixable | Measure re-identification over kinematic tracking with sparse sniffers and coverage gaps. Run it under linkage-CRL versus ACPC or enrolment blocking. Cite simplicio2018privacy and correia2023range as the mitigations being tested. |
| 5 | The 5GAA paper is misread (§2.5) | Fixable | Evaluate the 5GAA §6–7 severity-to-remediation mapping and the RC1–RC4 reinstatement classes under adversarial reports. Correct the claim in `top-three-directions.md` and `eval-traction.md`. |
| 6 | The time-to-neutralise decomposition is set by service levels (§2.6) | Fixable | Cut it to one paragraph that cites the 5GAA 2025 certificate policy. Simulate only the stages that are not fixed: detection, report upload and MA decision. |
| 7 | F2MD's MA Stress attack, the Intel and Qualcomm patents, and CATS identity resolution are not cited (§1) | Fixable | Add them to the prior-work table. Port the Intel corroboration rule and the CATS two-window rule as baselines. |
| 8 | The victim's safety cost is near zero (§2.7) | Fixable | Report it as a secondary null. The headline becomes attacker cost and corroboration cost. |
| 9 | Unread overlaps: Park 2024, J3287, TS 103 759 v2.2.1 (§2.9) | Fixable, but blocking | Get them before committing, through a library or interlibrary loan for J3287 and Park. If Park already scores slander defences per detector class, the reframe shrinks to its SCMS-plus-radio extension. |
| 10 | Venue (§2.8) | Fixable | First target VehicleSec, or NDSS with an operator-validated MA. USENIX Security only if the MA model is confirmed with ISS or the SCMS Manager. |

**The reframed direction:** *"Which evidence can frame you? Framability of standard V2X misbehaviour detectors under a proof-based SCMS MA."*
- Output 1: for each TS 103 759 / J3287 detector, the cheapest attacker capability (outsider radio, GNSS spoofing, relay, insider pseudonyms) that makes an honest vehicle produce MA-verifiable evidence against itself.
- Output 2: the corroboration rule and LA query budget that close each framable class.
- Output 3: the resulting false-revocation rate against time to neutralise under the 5GAA §7 mapping.

The engine fits this well because it has joint ground truth across radio, positioning and PKI. Confirm that it can model GNSS input spoofing and relay; `PROTOCOL.md` does not list either.
