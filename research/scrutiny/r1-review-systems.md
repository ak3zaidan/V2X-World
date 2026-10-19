# Review B: "Harvest now, link later" in the US SCMS

**Reviewer B:** vehicular networking and V2X security systems (SCMS, IEEE 1609.2/1609.2.1, C-V2X). Reviewing as a PoPETs program-committee member.
**Material reviewed:** the four-part research claim as stated in the task. No manuscript, simulator output or other project document was read, so this review covers the direction, threat model and evaluation plan, not results.
**Date:** 2026-10-18

---

## 1. Summary of the claim

The proposal says that in the US SCMS, unlinkability of pseudonym certificates depends on the **confidentiality** of the provisioning exchange, not only on the signature algorithm. It makes four claims:

1. A future attacker with a cryptographically relevant quantum computer (CRQC) can recover a vehicle's pseudonym certificates and tie them to its enrolment certificate. It needs either recorded EE–RA provisioning traffic (TLS plus the RA-encrypted request plus the PCA-encrypted responses) or an RA's archive. It can then link years of archived BSM tracks. Recording BSMs alone, or requests alone, is not enough, and linkage values stay safe because they are built from symmetric primitives.
2. So the provisioning path (TLS key exchange and PCA response encryption) must migrate to post-quantum cryptography first. Hybrid TLS stops the outside recorder but not the RA-archive attacker, which needs a 1609.2.1 change.
3. ETSI TR 103 949 and a 5GAA/GSMA report say harvest-now-decrypt-later (HNDL) is not a main V2X concern. The authors disagree for unlinkability.
4. A city-scale simulator with a full SCMS will measure "linkable pseudonym-years" and the cost of each migration order and provisioning design.

## 2. Overall assessment

The core observation is real, it is stated nowhere I could find in the V2X literature, and it suits PoPETs. However, **the attack chain as stated is incomplete in a way that changes the main conclusion.** The RA-archive attacker has a much cheaper route that runs through the linkage-value machinery the claim calls "safe". Migrating PCA response encryption does not close that route. Claim (1) understates the threat and claim (2) prescribes an insufficient fix. Both are fixable, and the corrected version is a stronger paper. The evaluation plan also measures the wrong costs (airtime) and leans on a simulator for a quantity that is mostly closed-form.

**Score: Major revision.** **Confidence: 4/5.** I know the SCMS design documents and 1609.2.1 well. I have not re-read the 2022 consolidated 1609.2.1 text for this review, and I mark below where my statements about it need checking.

## 3. Strengths

- **S1. The right question.** Prior V2X post-quantum work treats the problem as forgery and packet size. Examples: Twardokus et al., NDSS 2024, DOI 10.14722/ndss.2024.24267; Bindel et al., ePrint 2022/483; Chen et al., arXiv 2501.07028 and arXiv 2606.14008. This proposal points out that a privacy property, computational unlinkability against the RA, depends on public-key **encryption**, and so inherits HNDL exposure. Most migration guidance does not model this.
- **S2. A good foil exists in the literature.** Chen et al. (arXiv 2606.14008, §VI, p. 16, Table XII) argue that one-week pseudonym certificates "can mitigate the risk of quantum attacks" because they expire before a key can be extracted. That reasoning only covers forgery and says nothing about retroactive linkage. The proposal is the natural rebuttal.
- **S3. The ETSI framing really is at odds with the claim.** ETSI TR 103 949 V1.1.1 (2023-05), p. 16, Table 3, says C-ITS PII protection is "achieved by use of pseudonymous keying … rather than by encryption". It lists confidentiality "Protection duration: N/A" and gives disclosure impact as "None per se". On the same page (Table 2) it says "Privacy of the user is to be maintained across the lifetime of the ITS-S". That internal tension is exactly the gap the authors exploit, and they should quote both sentences.
- **S4. The insider-versus-outsider split is a good idea.** It correctly shows that hybrid TLS is not a complete answer. As Section 5 explains, the split needs to go further.
- **S5. Feasibility for a master's thesis.** The core claim can be checked against public design documents without new cryptography.

## 4. Weaknesses

### W1 (critical). "Linkage values remain safe" is false against the RA-archive attacker, and the shortest attack path goes through them.

The linkage-value *computation* is symmetric (SHA-256 seed chain and AES-based pre-linkage values; Brecht et al., arXiv 1802.05323, §V-B, p. 12). But in the SCMS design the linkage *plumbing* is protected by public-key encryption, and the RA holds per-device handles to it:

- **Linkage Chain Identifiers (LCIs).** "An LCI is the initial linkage seed ls1(0) … that LA 1 … encrypts to itself, e.g., E(pk1, ls1(0)), where pk1 is the public key of LA 1" (arXiv 1802.05323, p. 13; Fig. 4, p. 14). The RA keeps the LCIs per device, because during revocation "the RA … provides an array of LCIs" to the MA (§VII-D, Step 4, p. 18). It also uses them to request more pre-linkage values for top-ups (§VI, p. 15).
- **Encrypted pre-linkage values (PLVs).** "Pre-linkage values are encrypted (individually) for the PCA but sent to the RA for association with a certificate request" (p. 13). The RA therefore handles, for each device, the PLVs encrypted to the PCA's long-term key.

An RA-archive attacker with a CRQC can do the following:

1. Solve **one** discrete logarithm per LA (on the LA's LCI-encryption key), if LCIs really are public-key ciphertexts as the "e.g." in Brecht et al. suggests.
2. Decrypt every archived LCI to get ls1(0) and ls2(0) for every device.
3. Compute **every** linkage value of each device's chain forward: all periods, including certificates not yet issued or downloaded.
4. Match those values against the linkage values in certificates attached to archived BSMs.

The alternative is one discrete log on the PCA's PLV-decryption key, which yields every PLV the RA ever handled. Both routes:

- **amortise over the whole fleet**: O(1) Shor runs, against roughly one per device (caterpillar key) for the response-decryption route the proposal describes;
- **do not depend on the download horizon**: they cover the whole linkage chain, not just "up to about 3 years" of downloaded batches;
- **are not closed by PQ response encryption or by hybrid TLS**: the fix needs PQ (or symmetric) LCI encryption and PQ LA→PCA PLV encryption. Those are LA and PCA interfaces, and as far as I know they are CAMP/SCMS-specified rather than in 1609.2.1 (please verify).

Two qualifications:

- If deployed LAs build LCIs with a symmetric key-wrap (allowed by "encrypts to itself"), the LCI route closes. The PLV route remains while the RA has retained the PLV ciphertexts. Brecht et al. Table II (p. 15) lists the RA as storing only "Enrollment certificate … hash value of RA-to-PCA certificate request", so retention of the PLV ciphertexts is deployment-dependent.
- The proposal must establish what real RAs actually retain.

As written, claims (1) and (2) are wrong in an important case. As corrected, the attack is *more* severe and *cheaper*, and the paper becomes stronger.

### W2 (major). The migration "order" mixes up assets with deadlines.

- "Provisioning path before infrastructure certificates" does not hold together: the RA's encryption certificate, the PCA's encryption key and the LAs' keys *are* infrastructure keys, and per W1 they carry the HNDL exposure.
- The textbook rule already says key establishment and encryption migrate before signatures, because only confidentiality has retroactive exposure. That is Mosca's x + y > z inequality, and it is the reason NIST and NSA guidance prioritise KEMs.
- A PoPETs reviewer will call ordering-by-asset-class obvious. The defensible, non-obvious contribution is narrower: "in V2X, unlinkability is a confidentiality asset, with a shelf life equal to the vehicle's lifetime plus the archive retention period, carried by these specific keys."
- Recast claim (2) as a per-key list with a deadline for each: shelf life, migration lead time, and number of Shor instances. BSM-signature migration may still need to *start* first because of its long lead time (fleet turnover, the size of the 1609.2 SPDU), even though its *deadline* is later.

### W3 (major). Missing the classical baseline: what does the attack add beyond tracking from the BSMs themselves?

The attack needs archived BSMs at city scale. An attacker with those archives already links pseudonym changes with high success by tracking position and kinematics:

- Wiedersheim et al., "Privacy in inter-vehicular networks: Why simple pseudonym change is not enough", WONS 2010, DOI 10.1109/WONS.2010.5437115.
- The large literature since then, e.g. the Petit et al. survey cited as [21] in arXiv 1802.05323.

So the marginal harm is:

- (a) ground-truth linking across coverage gaps and parking (cold starts);
- (b) binding the whole history to one enrolment certificate, which the OEM can map to a VIN and owner; and
- (c) certainty in place of probabilistic linking.

These are real harms, and Golle and Partridge show that a long trajectory is itself identifying (DOI 10.1007/978-3-642-01516-8_26). But the paper must measure the **increment over a strong classical tracker on the same BSM archive**, not raw "linkable pseudonym-years". Otherwise the headline number overstates the harm.

### W4 (major). The outside-recorder path is underspecified.

- **TLS mandate and endpoints.** Is TLS mandatory on the EE–RA link in the deployed US SCMS, and where does it terminate (cloud load balancer, CDN, OEM proxy)? If TLS is optional, or terminated before an internal plaintext hop, the outsider needs only one discrete log on the RA's long-lived request-encryption key to read *all* requests. Brecht et al. say only that components use "protocols such as those from the TLS suite" (p. 5).
- **Vantage point.** Many OEMs route OBU traffic through their own back end. That changes who can record, and whether hybrid TLS on the device actually protects the RA-facing hop.
- **Download authentication.** How is the download authenticated? If the download request carries an enrolment-certificate signature inside TLS, breaking TLS alone reveals which device fetched which encrypted batch. You then need the caterpillar encryption private key, which takes one more discrete log per device after reading the request.
- **Counting Shor runs.** Write the full chain with the number of Shor instances per linked vehicle for each path. An early, slow CRQC will run the O(1)-instance attacks first, so per-path instance counts matter more for the results than which attack is possible at all.

### W5 (major). The cost metric is aimed at the wrong channel.

- Certificate provisioning runs over cellular or Wi-Fi backhaul, not the PC5/DSRC sidelink. "Airtime" is not a cost of provisioning migration; it is a cost of BSM-signature migration, which the paper argues should come later.
- The relevant costs are:
  - request and response sizes for ML-KEM and PQ butterfly variants (Eaton et al., ePrint 2024/946; Barreto et al., qSCMS, ePrint 2018/1247);
  - RA and PCA compute and storage at fleet scale;
  - OBU constraints (32-bit HSMs; compare Kushwaha and Kaur, ePrint 2026/2149, on 64-bit dependence);
  - the operational cost of re-keying the LA, PCA and RA encryption keys.
- If airtime stays in, justify it explicitly.

### W6 (moderate). Simulation where a formula would do.

- "Linkable pseudonym-years" under a migration date T, a CRQC date Q and an archive policy is, to first order, a closed-form function. Add up the certificates whose provisioning used classical encryption before T, filtered by the retention window. The LCI route in W1 makes it even simpler: the whole chain lifetime of every device whose LCI exists in a pre-migration archive.
- A city-scale traffic simulator adds value only where geography or mobility matter: the BSM-capture coverage needed to turn linkable certificates into linked *trajectories*, and the classical-tracking baseline from W3.
- Say explicitly what the simulator measures that the formula cannot, and validate the formula against the simulator.

### W7 (moderate). Claim (3) rests partly on a source I could not find.

- The ETSI citation checks out, with nuance (S3).
- I could not find a 5GAA/GSMA PQC report dated October 2026. The search found only the March 2025 5GAA–GSMA statement of intent (https://5gaa.org/5gaa-gsma-issue-statement-of-intent-on-the-quantum-threat-and-mitigation-strategies-for-the-automotive-sector/). That statement *supports* early migration because of vehicle lifetimes, and does not dismiss HNDL.
- Give an exact citation and a page or section for the claimed conclusion, or drop it.
- Characterise ETSI accurately. It does not argue against HNDL. It assumes privacy is not an encryption asset, and that assumption is what you refute.

### W8 (moderate). US-only scope misses an easier case.

- In the EU C-ITS PKI (ETSI TS 102 941), as I read it, the authorization-ticket request is encrypted to the AA's long-term key and carries an enrolment-credential signature that may be encrypted to the EA.
- An HNDL attacker who records AT requests and responses might link every AT to its EC with two discrete logs in total (the AA and EA keys), without any butterfly step.
- If true, the threat is cross-jurisdictional and ETSI's dismissal is more consequential. Check this; even a short section would broaden the paper's reach and its PoPETs appeal.

### Minor issues

- **"Link to its enrolment identity."** The enrolment certificate is a device credential, not PII. State the step from device to owner (OEM records) and who holds it.
- **1609.2.1 response modes.** As I recall, 1609.2.1 defines several ACA→EE response forms: plain, signed-then-encrypted, and compact unified butterfly. The plain form, if used for pseudonym certificates in any profile, would make the RA a classical linker today. List which modes the analysis covers. (I have not re-checked the 1609.2.1-2022 text; verify.)
- **Grover.** State and dismiss Grover against 128-bit AES and the truncated v-byte linkage values. Reviewers will ask.
- **Formal model.** Boldyreva, Kumar and Sun (PoPETs 2024, DOI 10.56553/popets-2024-0130) prove privacy against a corrupted RA *or* ACA, under the assumption that "the communication between EEs and the RA is private and authenticated" (p. 2). They name post-quantum security as future work (p. 13). Searching their paper finds no treatment of linkage values. Your observation is precisely that a CRQC breaks the channel assumption and, combined with the RA's view, turns a corrupted RA into an RA–ACA collusion after the fact. Saying it this way positions the work cleanly.
- **Everlasting privacy.** Position against the "everlasting privacy" literature (e.g. Moran and Naor, "Receipt-free universally-verifiable voting with everlasting privacy", CRYPTO 2006). The idea that computationally hidden links fall to a future adversary is well established there. Your contribution is identifying where SCMS falls short of it and quantifying the gap. Do not claim the concept.

## 5. Novelty assessment (own search)

Searches covered arXiv, IACR ePrint, Crossref, PoPETs and general web, with queries combining harvest-now, V2X, SCMS, pseudonym, unlinkability, butterfly and quantum.

- **No prior work found that states the HNDL-against-unlinkability mechanism for SCMS or ETSI provisioning.** The closest works treat quantum risk as forgery only:
  - arXiv 2606.14008 (Chen et al., hybrid certificates; short validity as mitigation, p. 16);
  - arXiv 2501.07028 (Chen et al., hybrid PQC/ECC certificates for SCMS);
  - NDSS 2024, DOI 10.14722/ndss.2024.24267 (Twardokus et al., PQ V2V authentication);
  - ePrint 2026/2149 (PQC in TLS for automotive, no privacy).
- **PQ butterfly constructions exist:** ePrint 2018/1247 (qSCMS) and ePrint 2024/946 (Eaton, Lamontagne, Matsakis, with game-based unlinkability). They supply the *fix* for response encryption but, per their abstracts, do not motivate it with an HNDL privacy threat. The proposal must not claim the PQ butterfly design, and should use 2024/946 as the instantiation it costs.
- **Classical formal analysis:** PoPETs 2024-0130 (see minor issues).
- **I could not find "Fikry 2026"** by title or author search. If it exists, it is the remaining novelty risk and must be read in full.
- **Verdict:** the threat statement appears novel. The *ordering* principle is not (W2). The linkage-plumbing route (W1) appears to be unpublished, and is arguably the most important contribution available here.

## 6. Technical correctness of the threat model and attack chain

| Step in claim | Assessment |
|---|---|
| Butterfly flow (caterpillar → cocoon at RA → PCA randomises and encrypts per certificate) | Correct; matches arXiv 1802.05323 §V-A, pp. 9–11 and p. 15 (Steps 3–5). |
| Requests alone do not suffice | Correct for certificate public keys. The PCA's random offset c (p. 11) is uniform, so even with the discrete logs of A and P, c = p − a − f(i) gives nothing to test against. **However**, if the RA-archive attacker holds LCIs or PLV ciphertexts, the "requests" side alone (with RA state) *does* suffice (W1). |
| BSMs alone do not suffice | Correct cryptographically: linkage values are PRF outputs and pseudonym keys are randomised. Classically false for practical tracking (W3). |
| Linkage values remain safe | **Incorrect** against RA archives (W1); correct only against a pure outsider who never sees RA–LA or RA–PCA traffic. Even there, consider a recorder on cloud inter-component links, which are TLS-only per p. 5. |
| "Years of certificates" | Understated for the LCI route (the whole chain, including future periods). Overstated for the outside recorder if top-ups are small and frequent, which depends on deployment parameters (1 week × ≥ 20 certificates, 1–3 year span: p. 7). |
| Hybrid TLS stops the outsider | Correct only if TLS is mandatory end to end to the RA application and the download request is inside it (W4). |
| PQ response encryption stops the RA archive | **Insufficient**; also needs PQ (or symmetric) LCI encryption and PQ LA→PCA PLV encryption (W1). |

## 7. Does the conclusion follow?

- **Partly.** "Some provisioning-path encryption must migrate before the CRQC, with a deadline set by the privacy shelf life" follows.
- "Response encryption and TLS first, and that suffices against RA archives" does not follow, because of W1.
- "Before BSM signatures" is a statement about deadlines, not about start order (W2).
- "HNDL matters for V2X unlinkability" follows in principle. Its practical weight depends on the increment over classical tracking (W3), which is not yet measured.

## 8. Would the evaluation plan convince me?

Not as described. It would convince me if it:

1. reported linkable pseudonym-years **per attack path**, with the number of Shor instances needed for each (O(1) for the LA/PCA/RA static keys; O(devices) for caterpillar keys; O(sessions) for TLS);
2. reported the **increment** over a state-of-the-art classical BSM tracker on the same simulated capture, with explicit capture-coverage assumptions;
3. took its migration states from actual key inventories (which component holds which key, its lifetime and its algorithm), not from asset classes;
4. replaced airtime with backhaul bytes, RA/PCA/LA compute, OBU feasibility and key-ceremony cost, using 2024/946 parameters for the PQ butterfly variant;
5. validated the simulator's numbers against a closed-form model and included a sensitivity analysis over CRQC date, archive retention and provisioning horizon;
6. included at least one fault-injection sanity check, for example the RA retaining PLV ciphertexts versus not, so the metric is shown to move when the vulnerability is toggled.

## 9. Required changes

1. Add the LCI and encrypted-PLV attack routes (W1). Correct the "linkage values remain safe" claim. Extend the remedy to LA and PCA encryption keys, and settle what deployed RAs retain.
2. Recast the migration "order" as a per-key list with deadlines (W2).
3. Add a classical-tracking baseline and report the marginal linkage (W3).
4. Specify the outsider's vantage point, TLS termination, download authentication and Shor-instance counts (W4).
5. Replace the airtime cost metric or justify it (W5). Say what needs simulation and what is closed-form (W6).
6. Fix the claim (3) citations: give the 5GAA/GSMA source exactly or drop it, and quote ETSI accurately (W7).
7. Assess the ETSI AT-request case, at least briefly (W8).
8. Position against Boldyreva et al. (PoPETs 2024), Eaton et al. (ePrint 2024/946) and the everlasting-privacy literature. Claim the mechanism and its measurement, not PQ butterfly keys or migration-order principles.

## 10. Questions to the authors

1. In the deployed US SCMS, are LCIs public-key or symmetric ciphertexts? Does the RA retain PLV ciphertexts after forwarding them to the PCA?
2. Is TLS mandatory on EE→RA in deployed profiles, where does it terminate, and do OEM back ends sit in the path?
3. Which 1609.2.1 ACA→EE response modes does your analysis cover, and are any pseudonym profiles unencrypted at the ACA layer?
4. What is your adversary's BSM capture coverage, and what linkage rate does a classical tracker already get on it?
5. How many Shor instances does each path need per linked vehicle, and how do your results change under a "slow CRQC" (days per discrete log) model?
6. Does your PQ fix keep RA–PCA unlinkability under PCA corruption? Butterfly expansion of encryption keys is what stops the PCA linking responses; a naive ML-KEM swap breaks it.
7. Can you get the October 2026 5GAA/GSMA report? Where exactly does it say HNDL is not a main V2X concern?
8. Have you checked whether ETSI TS 102 941 AT provisioning has the same or a cheaper exposure?

## 11. Score

**Major revision** (as a PoPETs submission built on this direction). **Confidence 4/5.**

The mechanism is real and, as far as I can find, unpublished, which makes it a good PoPETs topic. As currently scoped, though, the stated attack misses the cheapest path, the fix is incomplete, and the evaluation measures an inflated, partly closed-form quantity against the wrong cost. With W1 to W4 addressed, I would expect to support acceptance.

---

### Sources consulted (verified during this review)

- Brecht et al., "A Security Credential Management System for V2X Communications", arXiv 1802.05323 (IEEE T-ITS 2018): p. 5 (TLS), p. 7 (1 week, ≥ 20 certificates, 1–3 years), pp. 9–11 (butterfly keys, PCA random offset), pp. 12–14 (linkage seeds, LCI = E(pk_LA, ls(0)), PLVs encrypted for the PCA via the RA), p. 15 (Table II, storage), p. 18 (revocation, RA returns LCIs).
- ETSI TR 103 949 V1.1.1 (2023-05), p. 16, Tables 2–3: https://www.etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf
- Boldyreva, Kumar, Sun, PoPETs 2024(4), DOI 10.56553/popets-2024-0130: p. 2 (channel assumption, corruption cases), p. 13 (PQ as future work). https://crysp.petsymposium.org/popets/2024/popets-2024-0130.pdf
- Eaton, Lamontagne, Matsakis, ePrint 2024/946 (abstract).
- Barreto et al., qSCMS, ePrint 2018/1247 (abstract, via search listing).
- Chen et al., arXiv 2606.14008: abstract, p. 9, p. 16 Table XII. Chen et al., arXiv 2501.07028 (search listing).
- Kushwaha, Kaur, ePrint 2026/2149 (abstract).
- Twardokus et al., NDSS 2024, DOI 10.14722/ndss.2024.24267. Bindel et al., ePrint 2022/483 (as cited in PoPETs 2024-0130 [13]).
- Wiedersheim et al., WONS 2010, DOI 10.1109/WONS.2010.5437115. Golle and Partridge, Pervasive 2009, DOI 10.1007/978-3-642-01516-8_26.
- 5GAA–GSMA statement of intent (March 2025): https://5gaa.org/5gaa-gsma-issue-statement-of-intent-on-the-quantum-threat-and-mitigation-strategies-for-the-automotive-sector/
- Not verified, flagged in the text: the contents of IEEE 1609.2.1-2022 (response modes, LA interface scope), ETSI TS 102 941 AT-request encryption details, the October 2026 5GAA/GSMA report, "Fikry 2026", and the Moran–Naor DOI (cited by title).
