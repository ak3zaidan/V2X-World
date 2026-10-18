# Evaluation B: weaponised revocation + stage-decomposed time to neutralise

Date: 2026-10-17. Method: catalog and summary search (trust, bad-mouthing, false-accusation, slander,
collusion terms), full-text keyword reads of the PDFs I could get, and fresh OpenAlex, arXiv and web
searches. Semantic Scholar returned HTTP 429 throughout. I could not get full text for haas2012global,
liu2010limits, willecke2023a4md, abdo2024vehicular, park2024enhancingb, almani2025distributed or SAE
J3287, so for those I used abstracts and secondary quotes.

**Verdict.** N2 (framing) drops from **novel** to **partly novel**. Direction 1 (time to neutralise)
stays **partly novel**. Taken together, the candidate is **partly novel, with a narrow but real gap**.

## 1. The earlier claim "nobody studied framing that gets an honest vehicle revoked" is wrong

| Work | What it already does | Basis |
|---|---|---|
| **bimeyer2014misbehavior** (Bißmeyer, TU Darmstadt PhD, 2014) | A central misbehaviour evaluation authority (MEA) that "filter[s] fake reports that are created by an attacker aiming to hide its malicious behavior or blame benign nodes arbitrarily". It resolves reporter pseudonyms through the PKI (CoPRA). It requires both nodes involved in an overlap to report it, "to avoid blacklisting of benign nodes". This forces colluders to synchronise, and each extra colluder costs the attacker more. Simulated with colluding fake reporters. | full text (keyword read) |
| bimeyer2012central | The central-evaluation scheme. Needs at least 3 benign witnesses. With 37% colluding neighbours, an attack can be hidden. | abstract |
| raya2007eviction (LEAVE / RTC) | Local voting eviction, escalated to the CA. Needs an honest majority. | abstract |
| **Moore et al. 2008, SECON**, "Fast exclusion of errant devices from vehicular networks", doi:10.1109/sahcn.2008.26 (NOT in catalog) | Compares LEAVE and Stinger "while varying attacker capabilities … and the accuracy of misbehavior detection". Finds "LEAVE excludes fewer good devices when the attacker has compromised several devices". This is a quantitative count of honest nodes excluded by colluding accusers. | abstract |
| **Clulow & Moore 2006**, "Suicide for the common good", ACM SIGOPS OSR, doi:10.1145/1151374.1151381 (NOT in catalog) | Revocation by self-sacrifice: an accuser revokes its own credential along with the accused, which makes false accusation costly. | abstract |
| liu2010limits (ACNS 2010) | As cited by Bißmeyer: voting and suicide both fail when one node uses several certificates at once, and global revocation "must also consider false accusations of benign nodes". | secondary |
| haas2012global | Global revocation by an offline CA for the intersection collision warning (ICW) application. Uses non-repudiable retransmitted evidence, is "resilient to Sybil attacks", and lowers false positives. | abstract (Sandia listing) |
| **asavisanu2025cats** (CATS, IEEE TVT 2025) | A central Security Authority bans a vehicle by revoking all its certificates. Explicitly covers the "downvote collusion attack … tricking CATS into falsely banning a benign vehicle". Defences: two downvotes in separate windows, vote rate limits, temporary bans, reinstatement. Measures time to ban at city scale. | full text |
| **turgut2026ipek** (IPEK, arXiv 2604.07532) | "Closed-loop revocation with frozen thresholds". Measures false-positive revocation under 10–40% strategic and colluding false reporters. All methods break down above about one third attackers. | abstract |
| park2024enhancingb (IET ITS 2024, K-City field test) | A global misbehaviour detection server (GMBD) running alongside an SCMS, with defences against "slander attacks, in which vehicles with legitimate certificates may be falsely accused". | abstract; **read in full** |
| twardokus2021targeted | Reactive jamming makes an honest vehicle's signatures fail, so it is discredited within 2 minutes. The paper argues this leads to SCMS revocation but never executes the revocation. | full text (keyword read) |
| almani2025distributed (DRAMBR), wang2016rprep, xu2023mathcal (R-Tracing), lu2018bars, park2010certificate, chen2019tmec, priya2022resisting | Reputation and trust schemes, some with an authority that revokes, tested against bad-mouthing, on-off and ballot-stuffing attacks. Generic models; no SCMS. | abstracts |
| **5GAA 2022 MBD white paper** (NOT in catalog), https://5gaa.org/content/uploads/2022/07/5GAA-Misbehaviour-detection-Final.pdf | Names "slander attacks: false misbehaviour reports … to discredit the victims which may lead to the victims' certificates being wrongfully revoked … combining with Sybil attacks to appear as multiple coordinated reporters". | full text |
| **5GAA 2024 "MBD for V2X: Operational Aspects"** (NOT in catalog), https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf | Graded remediation options: ST0 issuance pause, ST1 partial revocation, ST2 full revocation, plus reinstatement. Says "no specifications of conditions" exist for choosing among them. | full text |
| ETSI TS 103 759 v2.1.1 / TR 103 460 (NOT in catalog as entries) | TS 103 759 requires the reporter's identity to be authenticated "to avoid … false reports", and reports to carry evidence. TR 103 460 says voting needs an honest majority. Neither gives MA decision rules. | full text |

## 2. Time to neutralise (direction 1)

The new searches found no stage-by-stage decomposition. Two works come closest. Scopelliti 2024
starts its clock at the revocation authority's decision, so it does not cover the decision itself.
CATS measures time to ban but not the stages. The fixed parts of the earlier verdict still hold:
F2MD, A4MD and zhang2024simulation already simulate the loop "report → MA → revocation request",
so "first closed loop" cannot be claimed.

## 3. What survives

The surviving claim is the first execution of framing through a **US-SCMS-faithful** pipeline
(J3287 report → MA → LA1/LA2 linkage-seed release → CRL → RSU/V2V dissemination → receiver
enforcement). It would quantify four things:

1. **Attacker cost.** How many insiders, butterfly-key pseudonyms per device, or jamming minutes
   it takes to get an honest vehicle revoked. This uses SCMS-specific Sybil reporting: pseudonyms
   are unlinkable to the MA unless the LAs cooperate.
2. **The privacy trade-off at the MA.** Resolving reporter linkage blocks Sybil framing, but it
   costs reporter privacy.
3. **Victim cost.** The victim's credential outage until re-enrolment, and the safety cost of
   that outage, measured with paired same-seed counterfactuals.
4. **MA policy evaluation.** Witness-diversity rules, 5GAA ST0–ST2 graded remediation and
   reinstatement, scored jointly on false-revocation rate, time to neutralise and safety.

## 4. Strongest reviewer objection

"Making false accusations robust is old: LEAVE and Stinger (2007–08), Bißmeyer (2012/14), CATS
(2025), IPEK (2026). You re-run known witness and threshold defences in a more detailed simulator.
Real MA procedures are unpublished and partly manual, with business-day CRL service levels. Your MA
rules and decision delays are therefore assumptions, so your false-revocation and
time-to-neutralise numbers measure your own parameter choices, not the deployed system."
Rebuttal needed: sensitivity bands over MA policies, rather than point estimates.
