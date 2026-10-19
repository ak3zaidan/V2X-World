# Review D: standards and industry perspective (PoPETs-style)

**Submission under review:** "Store-now, link-later: harvest-now-decrypt-later unlinkability loss in the US SCMS, its paths, and its measurement" (master's thesis direction, reviewed from the claim statement only).

**Reviewer profile:** industry security architect. I have worked on SCMS/CCMS operation and IEEE 1609.2.1, and I serve on the PoPETs PC. I start out sceptical of academic threat models.

**Method:** I read only the claim text. I independently checked the primary sources I could reach:
- the CAMP design paper, Brecht et al., arXiv:1802.05323 (IEEE T-ITS 2018);
- Boldyreva, Kumar and Sun, PoPETs 2024(4):565–582, ePrint 2024/1674;
- Chen et al., arXiv:2606.14008 (also DOI 10.1109/icjece.2026.3692881);
- ETSI TR 103 949 V1.1.1 (2023-05);
- the abstracts of ePrint 2018/1247 and 2024/946.

I also ran literature searches (OpenAlex, web). I could not obtain the IEEE 1609.2.1 text, CAMP's internal specifications or GSMA/5GAA PQ.08. Statements that depend on those are marked **unverified**.

---

## 1. Summary

The authors argue the following:
1. In the US SCMS, pseudonym unlinkability is a *confidentiality* asset.
2. Pseudonyms cannot be linked from broadcast material (BSMs, CRLs, the certificates themselves) even with a CRQC. Two reasons: the ACA's random offset hides the butterfly key information-theoretically, and the linkage chains are symmetric.
3. A store-now adversary that later breaks ECDH/ECIES can link pseudonyms by decrypting *archived provisioning traffic*. The paths are:
   - (a) access-network sessions;
   - (b) backend RA–ACA and RA–LA links;
   - (c) an RA archive (responses, LCIs, LA–ACA key material);
   - (d) misbehaviour-report uploads.
4. For each path they give the number of discrete logarithms required. Some counts are O(1) for a whole fleet.
5. Consequences:
   - The key-establishment and encryption keys need PQ deadlines set by the vehicle's lifetime plus retention, not by certificate validity.
   - Signature migration is orthogonal to this threat.
   - Migration must force fresh caterpillar keys and re-seed the linkage chains.
6. ETSI TR 103 949, GSMA/5GAA PQ.08 and Chen et al. 2026 each treat V2X confidentiality or short-lived pseudonyms as quantum-uncritical. The authors argue this holds for forgery but not for unlinkability.
7. A closed-form exposure model, checked against a city-scale simulator, will quantify "linkable pseudonym-years" across paths, migration schedules and provisioning variants, together with the cost of each fix.

## 2. Strengths

- **S1. The core observation is correct and practically important.** I checked the chain against the design paper.
  - The RA collects ACA responses and bundles them per device (Brecht et al., §V step 6, p. 15).
  - The RA keeps issuing automatically "throughout the life-time of the device" (p. 7).
  - The LCI is described as the initial linkage seed that the LA "encrypts to itself, e.g., E(pk1, ls1(0))" (p. 13; Fig. 4, p. 14).
  - Linkage seeds are hash-chained, so the initial seed yields every later seed (Fig. 4).

  Taken together: if a public-key-encrypted LCI and the RA's device association are exposed, the device's linkage chain can be linked for its whole life. In industry we have treated "RA alone cannot link" as an organisational property. The submission shows it is also a *cryptographic* property, and one with a quantum expiry date. That is worth saying in print.

- **S2. The separation of what does *not* leak is right.** It is a valuable negative result in its own right.
  - The certificate key is the cocoon key plus a fresh offset, as the claim describes. Recovering discrete logs of P and of the cocoon key gives *some* offset for any pairing, so BSMs and certificates alone give the adversary nothing even with a CRQC.
  - Requests alone give cocoon public keys but never the offsets.

  This is exactly where practitioners go wrong in both directions. Some say "PQ doesn't matter for one-week certificates" (Chen et al., Table XII, p. 16). Others say "everything ECC is broken, so all privacy is lost".

- **S3. The conclusion is useful to operators.** The keys that need deadlines are the transport and encryption keys, not the PKI signature hierarchy. That runs against the signature-centric framing of most V2X PQ work, e.g. Yoshizawa and Preneel, VTC2023-Spring, and the ETSI risk summary in TR 103 949 §4.4, p. 14, which centres on "masquerade". Several of the fixes are *server-side*: component-link TLS, LCI and LA–ACA key handling, and RA retention. Those can be deployed without touching the vehicle fleet, which is the fleet's hardest constraint.

- **S4. Concrete standard hooks.** The plan names specific places where IEEE 1609.2.1 leaves something open: LA–ACA key establishment, the LCI format, and the response-encryption options. A concrete standards change raises the paper above "PQ is needed".

- **S5. The standards critique is accurate as far as I could verify.** ETSI TR 103 949, Table 3 (p. 16), says:
  - protection duration "N/A";
  - disclosure impact "None per se";
  - privacy is "achieved by use of pseudonymous keying … rather than by encryption".

  Table 2 on the same page requires that "Privacy of the user is to be maintained across the lifetime of the ITS-S". The tension the authors point out is real and checkable.

## 3. Weaknesses

### W1. The novelty statement is wrong about Chen et al. 2026 (major)

The claim says Chen et al. rate one-week certificates quantum-resistant and that this "holds for forgery but not for unlinkability". That describes their Table XII (p. 16), which is about forgery. However, the same paper:
- states on p. 6 that EC↔PC unlinkability in the IEEE scheme rests on ECDLP hardness for the RA, the PCA and outsiders, and "may become vulnerable in the presence of quantum computing technologies";
- in §"The Anonymity of EEs" (p. 17), models a quantum-capable adversary in the middle of EE–RA and RA–PCA. It encrypts the EE's key to the PCA with ML-KEM and wraps the response under a pre-shared key, precisely so that the RA "cannot access the plaintexts of PC".

So the qualitative point, that a quantum RA or eavesdropper breaks EC–PC unlinkability and that the fix is PQ encryption, is already in the literature. What Chen et al. do *not* do:
- the store-now/link-later temporal framing (shelf life = vehicle life + retention);
- the LCI, LA–ACA, backend-shuffle and misbehaviour-report paths;
- discrete-log counting;
- the information-theoretic argument for what does *not* leak;
- migration-order consequences such as fresh caterpillars and chain re-seeding;
- any quantification.

The claim must be rewritten to credit Chen et al. for identifying the dependency, and to claim the analysis, paths and measurement. As written, a PC member who knows that paper will read the novelty claim as careless.

Two more items for related work:
- The general principle (anonymity needs forward-secure, quantum-safe key exchange because unlinkability outlives the session) is established in anonymous communication, e.g. Ghosh and Kate, "Post-Quantum Forward-Secure Onion Routing", ACNS 2015. The authors already disclaim the principle, so they should cite it.
- Anonymity of PQ KEMs, which the "independent per-certificate KEM keys with anonymous ciphertexts" fix depends on: Grubbs, Maram and Paterson, "Anonymous, Robust Post-Quantum Public Key Encryption", Eurocrypt 2022 (ePrint 2021/708).

My own searches (OpenAlex; "harvest/store now decrypt later" with vehicular pseudonym and linkability terms; post-quantum SCMS privacy) found nothing that quantifies the store-now linkage threat for the SCMS. The rest of the novelty claim survives, but only in its narrowed form.

### W2. Boldyreva et al. is mis-characterised (moderate)

The claim says the corrupted-RA privacy proof "assumes a private EE–RA channel and a DH-type assumption".
- In Boldyreva, Kumar and Sun, the corrupted-RA privacy result rests on two things: ECIES being IND-CPA under additive related-key attack (Hash-Diffie-Hellman), and target-robustness, in the ROM/ICM (Thm 5.1 and Thm 6.1 as summarised on p. 3).
- The "secure channel" assumption appears in the *corrupted-ACA* model: "the adversary does not get to intervene into the EE-RA communication because they talk over a secure channel" (p. 7). In the corrupted-RA model the RA *is* the channel endpoint.
- The paper also notes that strong privacy against a corrupted RA does not hold when an RA splits a device's requests across several ACAs (p. 2).

The conclusion survives: a CRQC breaks Hash-DH, so the corrupted-RA guarantee is gone. But the sentence must be fixed, and the authors should say which theorem each path invalidates:
- path (c) invalidates Thm 5.1, the corrupted-RA case;
- paths (a) and (b) invalidate the secure-channel premise of Thm 5.2 and the outsider case.

Also note that the PoPETs model has no LAs or linkage values. The LCI and PLV paths sit *outside* every published proof, which is a point in the authors' favour.

### W3. Several load-bearing facts rest on an "e.g." or on unverified formats (major for the headline numbers)

- **LCI encryption.** Brecht et al. write "e.g., E(pk1, ls1(0))" (p. 13). That is an illustration, not a specification. An LA operator would more naturally wrap the seed under a symmetric HSM key, or use an opaque database handle; both are quantum-irrelevant. The fleet-wide O(1) result for path (c) depends on this choice. It must be presented as a scenario, and the thesis should check the actual format in IEEE 1609.2.1 or the CAMP LA interface specification. If the standard leaves the format open, the finding becomes a recommendation ("specify a symmetric or PQ LCI wrap") rather than a vulnerability.
- **LA–ACA key establishment.** This is "out of scope" in the standard (per the claim; **unverified** by me). The same caveat applies: it is O(1) only if the key was established by recorded ECDH or ECIES.
- **Misbehaviour reports.** The claim says reports go up "under an enrolment-signed header". The design paper says a report includes "the reporter's pseudonym certificate and corresponding signature" and is "encrypted by the reporter to the MA", sent via the RA behind the LOP (§VI-A, p. 16). If the outer layer is pseudonym-signed only, path (d) links reporter pseudonyms to report contents, not to the enrolment certificate. The authors must cite the clause (1609.2.1-2022? SAE J3287?) that establishes enrolment-certificate authentication of the upload. Until then, (d) is unverified.
- **Where provisioning TLS terminates.** The design places a Location Obscurer Proxy between EE and RA that rewrites the source address "very similar to the masquerading feature" of NAT (p. 15). That is consistent with end-to-end TLS. In deployments I have seen, however:
  - TLS commonly terminates at a cloud load balancer or WAF;
  - many OEMs route provisioning through their own telematics back end.

  In those cases "end-to-end hybrid TLS to the RA" is not one configuration change. It adds a new archive point, the OEM back end or terminating proxy, that the path list does not contain. Boldyreva et al. (p. 6) record that 1609.2.1 lists TLS 1.2, TLS 1.3 and ISO/TS 21177 as secure-channel options. The thesis should state which one it assumes, and whether 1609.2.1 permits provisioning with only SPDU-level encryption, for example via an RSU relay. If it does, that is a path with *zero* TLS sessions to break.

### W4. Request coverage and the role of "top-ups" are internally inconsistent (moderate)

The claim describes one request covering "1–3 years of weekly periods, with top-ups". The design paper says devices "do not need to explicitly request new certificates", and the SCMS issues automatically "throughout the life-time of the device" (p. 7), with the RA fetching new PLVs through the LCI (p. 15). If that is the deployed behaviour, the caterpillar keys in one archived request cover the device's *life*, not 1–3 years. That strengthens the "force new caterpillars" requirement, but the exposure model must use the right coverage. The authors need to state which top-up behaviour they model and cite it:
- re-request with fresh caterpillars, or
- RA-side continuation from the original caterpillars.

### W5. The threat model needs a sharper adversary, and the increment over classical tracking is the paper (major)

A store-now attacker who links *archived BSMs* must also have run a large roadside BSM collection network for years. Such an actor, typically a nation-state or a large carrier, usually also holds:
- ALPR feeds;
- cellular location records;
- for path (a), the access network itself, which already locates the subscriber continuously.

The proposal promises "the increment over a classical trajectory tracker". That is the right instinct, but the baseline must be the adversary's *realistic* classical capability, not just BSM trajectory stitching. Two classical facts matter here:
- Pseudonym change without silent periods is already largely defeated by trajectory linking (Wiedersheim et al., "Privacy in inter-vehicular networks: Why simple pseudonym change is not enough", WONS 2010; Petit et al., IEEE COMST 17(1), 2015, DOI 10.1109/COMST.2014.2345420).
- Home/work location pairs re-identify most drivers (Golle and Partridge, Pervasive 2009).

The genuine quantum increment is therefore concentrated in:
- (i) linking across long gaps such as parking, days and weeks;
- (ii) linking to the enrolment credential, and through RA and OEM records to a VIN;
- (iii) adversaries who lack carrier access, e.g. a foreign service tapping an exchange point or a party that later acquires an RA archive.

The paper should define 2–3 named adversaries with explicit capabilities, and report the increment separately for each. Without this, the PoPETs reviewer response will be "your adversary can already track these cars", and it would be fair.

Path (c) is not "passive store-now" at all. It is a *future compromise or compelled disclosure* of an operator archive. Keep it separate; its policy fix (retention limits) differs from the cryptographic fix.

### W6. "RA records alone suffice" may matter less operationally than it sounds (minor to moderate)

Boldyreva et al. (p. 2) observe that an OEM typically contracts one SCMS provider, so the RA maps one-to-one to an ACA. In practice the RA and ACA are often run by the same provider under policy separation only. The design paper's own collusion table (p. 23) treats RA+PCA as a linking pair. Where one organisation holds both, an *insider* already links classically. The quantum increment for path (c) applies to an *outsider* who obtains only the RA store, or to a setup where RA and ACA are run by different organisations. The authors should say how US deployments are actually organised (SCMS Manager policy), or present both cases.

### W7. Evaluation design: validation risks circularity, and the metric needs definition (moderate)

- Validating a closed-form count against a simulator that implements the same protocol model checks the arithmetic, not the threat model. That is worth doing, but it is not *validation* in the sense a PC expects. What would convince me:
  - message sizes and cadences taken from real 1609.2.1 encodings, ideally captured from a vendor stack or interoperability test-event traces;
  - the parameters that drive exposure (retention periods, top-up cadence, fleet age distribution, fraction of devices that can be updated over the air) taken from public deployment data or operator interviews;
  - a sensitivity analysis over the parameters you cannot source.
- "Linkable pseudonym-years" must be defined precisely:
  - Is it certificates whose linkage class is revealed, or BSM *trajectories* actually joined?
  - Is it weighted by the probability that the BSMs were captured?
  - Is linking to enrolment counted separately from pseudonym-to-pseudonym linking?

  Relate it to established tracking metrics (e.g. tracking success or maximum tracking time in Petit et al. 2015).
- **The legacy fleet dominates.** Many deployed OBUs and aftermarket devices will never receive a PQ TLS stack or new caterpillar logic. For them the only available mitigations are server-side: backend links, LCI and LA–ACA keys, RA retention. My expectation is that this is the main *actionable* result, and the evaluation should be built to show it: which share of exposure can be removed without touching a vehicle.

### W8. Smaller technical points

- **Grover bound.** "About 2^64 Grover work" for linkage chains should be tied to the actual search problem. There are two independent 128-bit LA seeds, linkage values are truncated (IEEE 1609.2 LinkageValue is 9 bytes, from my recollection, **unverified** here), and Grover parallelises poorly. The real cost is far above 2^64 and the conclusion only gets stronger. Say so and move on.
- **Path (b).** Byte-identical ACA-response ciphertexts on the RA–ACA and EE–RA legs undo the shuffle, but the certificate is still inside per-certificate ECIES. The O(1) fleet linkage on backend links really comes from PLVs (LA→ACA encryption) and LCIs, not from the response ciphertexts. State per-sub-path counts.
- **Path (a) address exposure.** The design's LOP hides the source address from the RA (p. 15), so the RA archive (c) has no network identifier, while (a) does. This should show up in the identity-linkage dimension of the metric.
- **Response anonymity under PQ.** "Independent per-certificate KEM keys with anonymous ciphertexts" costs one KEM public key per certificate in the request. Report it concretely: ML-KEM-512 public keys are 800 B (FIPS 203), so roughly 20 × 52 × 3 keys come to about 2.5 MB per request for 3 years. Compare this with lattice butterfly (Eaton, Lamontagne and Matsakis, ePrint 2024/946; Barreto et al., qSCMS, ePrint 2018/1247). The statement that lattice butterfly trades information-theoretic hiding for a computational assumption is an important caveat: under a PQ butterfly, path (c) becomes a *computational* question again. Quantify, or at least state, which hardness assumption then protects unlinkability against an RA archive.
- **ETSI transfer.** TR 103 949 concerns the EU CCMS (ETSI TS 102 941 EA/AA flows), which has no butterfly expansion, no LAs and different AT-request encryption. The criticism transfers in spirit, but either analyse the TS 102 941 flows briefly or say the critique is about the asset classification only.
- **PQ.08.** I could not find GSMA/5GAA PQ.08 (Oct 2026) through public search. I located only the March 2025 5GAA–GSMA statement of intent (5gaa.org) and a 2026 5GAA "PQC4Auto" deck marked internal and not an agreed position. If PQ.08 is not public, quoting §3.3 in a PoPETs paper is a problem. Confirm that it is public and give a stable URL.

## 4. Novelty assessment

- **Established:**
  - HNDL as a general threat;
  - forward secrecy and PQ key exchange for anonymity systems (Ghosh and Kate, ACNS 2015);
  - PQ butterfly designs (ePrint 2018/1247; 2024/946);
  - PQ hybrid certificate schemes for the SCMS;
  - the qualitative statement that SCMS EC↔PC unlinkability depends on ECDLP and needs PQ encryption on EE–RA and the response (Chen et al. 2026, pp. 6 and 17).
- **Apparently new, as far as I found:**
  - treating SCMS unlinkability as a store-now asset with a lifetime-plus-retention shelf life;
  - the full path inventory, including LCI, LA–ACA, backend shuffle and misbehaviour reports, with discrete-log counts and the fleet-wide O(1) cases;
  - the information-theoretic "what does not leak" argument;
  - migration requirements specific to the butterfly design (forced fresh caterpillars and chain re-seeding);
  - quantification across provisioning variants and schedules, including the cost of fixes;
  - the explicit rebuttal of three standards-body positions.
- **Verdict:** novel enough for PoPETs *if* the claim is narrowed to the SCMS-specific analysis and measurement and credits Chen et al. properly. "First to identify this mechanism" is not defensible as written.

## 5. Technical correctness of the threat model and attack chain

- **Information-theoretic hiding (correct).** The offset argument for the original butterfly matches the CAMP design: the PCA expands the cocoon key with a random value and encrypts the result to the cocoon encryption key, as also described in Chen et al. p. 6. The authors should check that the same holds for the *unified* (implicit) butterfly variant and for ACPC, since the claim lists them.
- **Paths (a)–(c) (correct in structure; counts conditional).** As shown in W3, W4 and W8, the counts depend on formats that are not verified (LCI, LA–ACA, MBR authentication, top-up behaviour, where TLS terminates).
- **Path (d) (unverified).** The cited design paper contradicts it.
- **Proof-invalidation statement (conclusion right, attribution wrong).** See W2.
- **"Signatures do not affect passive linkability" (correct).** It should be said explicitly that an *active* CRQC adversary forging enrolment- or RA-level signatures is out of scope.

## 6. Does the conclusion follow?

- **Mostly yes.** The direction is that confidentiality-side keys need lifetime-plus-retention deadlines and signatures are orthogonal for this asset. Given the analysis, that follows.
- **The ordering is not yet supported.** The specific claim that *access-network* hybrid TLS is the first priority is not supported until the adversary increment (W5) is measured. For a carrier-level adversary the increment of path (a) may be small. Meanwhile path (c), and (b) if the formats are public-key, may be both larger and fixable server-side today. The data should decide the ordering, not the argument.
- **"Migration must force new caterpillars and re-seed the chains" follows,** and is a concrete, under-appreciated operational requirement with real cost: re-provisioning traffic, plus LA and RA load. The cost model should include the revocation side, since re-seeding changes the LCIs the MA would use.

## 7. Would the evaluation plan convince me?

Partly. The design (per path, per discrete-log budget, per schedule and per variant, plus cost) is the right shape. To convince me it needs:
- **(i)** named adversaries and a realistic classical baseline (W5);
- **(ii)** parameters sourced from real encodings and deployment data, with sensitivity analysis rather than consistency checks only (W7);
- **(iii)** an explicit legacy-fleet scenario, with the result "share of exposure removable server-side";
- **(iv)** a precise metric definition;
- **(v)** each format-dependent path reported as a scenario, not presented as fact.

## 8. Required changes

1. Rewrite the novelty claim to credit Chen et al. 2026 (pp. 6, 17) for the qualitative dependency, and cite Ghosh and Kate (ACNS 2015) and Grubbs, Maram and Paterson (Eurocrypt 2022).
2. Correct the Boldyreva et al. characterisation, and map each path to the theorem or model it invalidates.
3. Verify against IEEE 1609.2.1 (and SAE J3287 or the CAMP interface specifications) the format of:
   - the LCI;
   - LA–ACA key establishment;
   - misbehaviour-report authentication;
   - top-up behaviour;
   - permitted EE–RA transports.

   Re-label any unverifiable path as a scenario.
4. Define 2–3 adversaries and measure the increment over each one's classical capability, including carrier and ALPR-level data where relevant.
5. Separate path (c), archive compromise or compelled disclosure, from the passive paths, and treat retention policy as a mitigation.
6. Add the legacy, non-updatable fleet scenario and report what share of exposure server-side fixes remove.
7. Define the exposure metric formally and relate it to existing tracking metrics.
8. Replace "validation" with external grounding (real encodings and cadences) plus sensitivity analysis.
9. Confirm that PQ.08 is public, or drop the quotation.

## 9. Questions to the authors

1. Which IEEE 1609.2.1 clause defines the LCI? Does it constrain the encryption? Same for LA–ACA key establishment.
2. Is the misbehaviour-report upload authenticated with the enrolment certificate in any published US specification? Which one?
3. In the deployment you model, does the RA keep issuing from the original caterpillar keys for the device's lifetime (Brecht et al., p. 7), or does the EE re-request with fresh caterpillars?
4. Where does provisioning TLS terminate in practice (LOP, load balancer, OEM back end)? Does 1609.2.1 allow provisioning with no TLS at all (e.g. RSU-relayed)?
5. For an adversary who *is* the access-network operator, what is the increment of path (a) over cell-location records?
6. Under PQ butterfly, which computational assumption protects unlinkability against an RA archive? What is its expected shelf life compared with the information-theoretic original?
7. How many US devices in service today can receive a PQ TLS stack and new caterpillar logic over the air?
8. Do your counts assume one RA request-encryption key per RA? What is the realistic rotation interval of that key?

## 10. Overall assessment

**Score: Major revision.** The core threat is real, correctly separated from the forgery threat, and actionable for operators and for IEEE 1609.2.1. I would like to see it published. But as stated:
- the novelty claim overlooks a 2026 paper that already makes the qualitative point;
- one proof is mis-characterised;
- the headline O(1) fleet-wide results rest on formats the design paper gives only as an example ("e.g.") or that conflict with it (misbehaviour reports);
- the adversary's classical baseline is under-specified.

None of this is fatal, and all of it can be fixed within the thesis plan.

**Confidence: 4/5.** I know the SCMS and 1609.2.1 architecture well. I could not consult the 1609.2.1 text or PQ.08 for this review.

---

### Sources consulted (all claims above cite these)

- Brecht et al., "A Security Credential Management System for V2X Communications", arXiv:1802.05323 (IEEE T-ITS 19(12), 2018). Cited pages: p. 5 (TLS between components; LOP), p. 7 (lifetime auto-issuance, batches), p. 13 (LCI "e.g., E(pk1, ls1(0))"), p. 14 (Fig. 4), p. 15 (provisioning steps 1–6, LOP), p. 16 (misbehaviour report format), p. 18 (revocation via LCI), p. 23 (collusion pairs).
- A. Boldyreva, V. Kumar, J. Sun, "Provable Security Analysis of Butterfly Key Mechanism Protocol in IEEE 1609.2.1 Standard", PoPETs 2024(4):565–582, https://petsymposium.org/popets/2024/popets-2024-0130.php; ePrint 2024/1674. Cited pages: p. 2 (multi-ACA attack; one-to-one RA–ACA in practice), p. 3 (Thm 5.1 and 6.1 assumptions), p. 6 (TLS 1.2/1.3 and ISO/TS 21177 channels), p. 7 (secure-channel assumption in the corrupted-ACA model).
- A. C. H. Chen et al., "Pseudonym Scheme Based on Hybrid Certificates for SCMS in Vehicular Communications", arXiv:2606.14008; DOI 10.1109/icjece.2026.3692881. Cited pages: p. 6 (unlinkability rests on ECDLP; quantum vulnerability), p. 16 (Table XII), p. 17 (quantum MITM anonymity with ML-KEM and PSK).
- ETSI TR 103 949 V1.1.1 (2023-05), https://etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf. Cited clauses: §4.4 (pp. 14–15, masquerade-centred risk; Mosca X with 20-year device life), §5.2 Tables 2–3 (p. 16).
- P. Barreto et al., "qSCMS: Post-quantum certificate provisioning process for V2X", ePrint 2018/1247.
- E. Eaton, P. Lamontagne, P. Matsakis, "Provably Secure Butterfly Key Expansion from the CRYSTALS Post-Quantum Schemes", ePrint 2024/946.
- 5GAA–GSMA statement of intent (Mar 2025), https://5gaa.org/5gaa-gsma-issue-statement-of-intent-on-the-quantum-threat-and-mitigation-strategies-for-the-automotive-sector/. PQ.08 was not found.
- T. Yoshizawa, B. Preneel, "Post-Quantum Impacts on V2X Certificates – Already at The End of The Road", IEEE VTC2023-Spring (Lirias https://lirias.kuleuven.be/retrieve/707117).
- Cited from my own knowledge, not re-fetched for this review: Ghosh and Kate, ACNS 2015 (LNCS 9092); Grubbs, Maram and Paterson, Eurocrypt 2022 / ePrint 2021/708; Wiedersheim et al., WONS 2010; Petit et al., IEEE COMST 2015, DOI 10.1109/COMST.2014.2345420; Golle and Partridge, Pervasive 2009; FIPS 203.
