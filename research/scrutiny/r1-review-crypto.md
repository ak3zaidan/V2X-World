# Review R1: "Harvest now, link later" in the US SCMS

**Reviewer A.** Applied cryptography: post-quantum migration and privacy-preserving credentials. PoPETs PC perspective.
**Date:** 2026-10-18.
**Under review:** the research claim (1)–(4) as stated in the task. It is a thesis direction, not a finished paper, so I review the claim, the threat model and the evaluation plan as they would read if submitted.
**Independence:** I did not consult the project's synthesis or scrutiny files or its git history. Every source below I located and read myself during this review.

---

## 1. Summary

The submission argues that pseudonym unlinkability in the US SCMS faces a store-now/break-later threat. A passive adversary records the vehicle-to-RA provisioning exchange: the butterfly request, signed with the enrolment certificate and encrypted to the RA, and the later download of per-certificate encrypted responses. Alternatively the adversary later obtains an RA's archive. Once a CRQC exists, the adversary breaks the classical ECDH/ECIES layers. That yields the caterpillar keys and expansion seeds, then the PCA responses, and so the pseudonym certificates bound to the enrolment identity. Archived over-the-air BSM tracks signed under those certificates can then be stitched together. The authors conclude:

- provisioning confidentiality (TLS key exchange plus response encryption) must migrate to PQC before infrastructure and BSM signatures do;
- hybrid TLS stops an outside recorder but not an RA-archive adversary;
- two industry/standards studies wrongly dismiss harvest-now-decrypt-later (HNDL) for V2X.

They plan to quantify "linkable pseudonym-years" and the cost of each fix with a city-scale simulator.

## 2. Strengths

- **S1. The core observation is correct and under-discussed.** Butterfly key expansion keeps the RA from linking certificates to a device by encrypting the PCA's response to a key only the device can derive. The PCA in turn cannot see the device's identity. Brecht et al., *A Security Credential Management System for V2X Communications*, IEEE T-ITS 2018, arXiv:1802.05323 (Fig. 2, p. 11; Steps 1–6, pp. 15–16) describe this. Both separations are computational and rest on ECDLP. A CRQC lets one party, or an outside recorder of both legs, take over the cryptographic role of the other party. In effect the CRQC simulates RA–PCA collusion after the fact. The SCMS privacy argument explicitly assumes that collusion does not happen (arXiv:1802.05323, p. 13: PLVs are "encrypted for the PCA to prevent the RA from colluding…"). This is a crisp, true and PoPETs-relevant statement.
- **S2. It exposes a real gap in the standards analysis.** ETSI TR 103 949 V1.1.1 (2023-05) frames the CRQC risk almost entirely as masquerade and loss of trust (§4.4, p. 14). Its data-assessment table (Table 3, p. 16) answers "How long does confidentiality need to be maintained" with "N/A" and "consequences of disclosure" with "None". It reasons that C-ITS privacy is "achieved by use of pseudonymous keying … rather than by encryption". That reasoning misses that pseudonymous keying itself depends on encrypted provisioning. Pointing this out is valuable.
- **S3. The adversary split is the right one.** An outside recorder and an RA-archive adversary need different fixes (transport vs. application-layer). Making that distinction is the most useful engineering contribution the work could make.
- **S4. Several "does not suffice" sub-claims are correct, with the caveat in W1.**
  - **Broadcast only.** The cert key is V = B + cG with the PCA's random c (arXiv:1802.05323, Fig. 2, p. 11). A CRQC that recovers the private key of V still cannot attribute it to a caterpillar key without c and the expansion seed.
  - **Requests only.** For the same reason, knowing A and the expansion seed does not identify V without c.
  - So the outside recorder genuinely needs both legs. This is a non-trivial and correct point.

## 3. Weaknesses

### W1 (critical): the claim that linkage values remain safe is false for the RA-archive adversary, and the proposed fix is therefore insufficient

The claim says linkage values "remain safe" because they are symmetric constructions. The PRF chain is symmetric (AES Davies–Meyer, arXiv:1802.05323, p. 12), but its *transport* is public-key:

- **Pre-linkage values.** "Pre-linkage values are encrypted (individually) for the PCA but sent to the RA for association with a certificate request" (arXiv:1802.05323, p. 13; Step 3/4, p. 15). An RA archive holds these ciphertexts next to the device's request. With a CRQC the adversary decrypts plv1(i,j) and plv2(i,j) and computes lv = plv1 ⊕ plv2. lv is printed in every pseudonym certificate, which is broadcast. Every certificate of that device is then linked to its enrolment identity with **no need to touch the response encryption at all**.
- **Linkage Chain Identifiers are worse.** "An LCI is the initial linkage seed ls1(0) or ls2(0) that LA1 or LA2 … encrypts to itself, e.g., E(pk1, ls1(0))" (arXiv:1802.05323, p. 13). The RA must keep each device's LCIs to request future PLVs. A CRQC applied to the two LAs' static encryption keys (two discrete logs, once) recovers ls1(0) and ls2(0) for every device in the archive. From those, the adversary computes every linkage value for every period i over the device's lifetime.
- **Consequences.** These include certificates issued *after* the response encryption has been migrated to PQC, unless linkage chains are re-seeded. Three things follow:
  - **(a)** The claim's own fix (PQ response encryption, "a 1609.2.1 change") does not stop the RA-archive adversary. The LA→PCA PLV encryption and the LCI encryption must also be post-quantum, and existing chains must be re-seeded at migration.
  - **(b)** The exposure is not bounded by "years of certificates downloaded". It can extend to the entire device lifetime.
  - **(c)** The quantum cost is per *authority key*, not per vehicle. That matters for the realism of a scarce early CRQC; see W5.
- **Backend links.** The same argument applies to an outside recorder positioned on RA–LA and RA–PCA backend links. arXiv:1802.05323 (p. 5) says these use TLS between components plus application-layer encryption when forwarding. The threat model currently ignores those links.

This is the most important correction. It does not kill the paper. It makes the threat larger and the fix broader. But as stated, claim (1)'s "linkage values remain safe" and claim (2)'s "needs post-quantum response encryption" are technically wrong for the stronger adversary.

### W2 (major): novelty is an application of a known principle, so the claim must be scoped accordingly

- **The general mechanism is not new.** Classical-only key establishment today lets a future quantum adversary deanonymise past anonymous traffic.
  - Ghosh & Kate, *Post-Quantum Forward-Secure Onion Routing (Future Anonymity in Today's Budget)*, ACNS 2015, ePrint 2015/008, motivated hybrid Tor handshakes exactly by future deanonymisation.
  - The PETs literature has long had "everlasting privacy" against future unbounded adversaries (Moran & Naor, CRYPTO 2006, DOI 10.1007/11818175_22).
  - Recent anonymous-reporting work explicitly models an adversary that archives traffic so a future quantum computer can "retroactively deanonymize" participants (QADR, arXiv:2511.15272).
- **The V2X-specific prior work is close.**
  - qSCMS, Barreto et al., ePrint 2018/1247, already argues that butterfly expansion's ECC basis is quantum-vulnerable and gives a lattice-based BKE.
  - Eaton, Lamontagne & Matsakis, ePrint 2024/946, give a provably unlinkable PQ BKE from Kyber/Dilithium.
  - Chen et al., *Pseudonym Scheme Based on Hybrid Certificates for SCMS*, arXiv:2606.14008 (2026), already put ML-KEM encryption on the EeRaCertRequest to both RA and PCA (pp. 9–10).
  - Their stated goal is to "eliminate the possibility of inferring any correlation between the public key in a pseudonym certificate and that in an enrollment certificate" (abstract, p. 1).
- **What I did *not* find** is a paper that:
  1. states the retroactive-linkage threat for SCMS explicitly;
  2. derives which recorded flows are necessary and sufficient;
  3. quantifies it.
- In my search none of 2018/1247, 2024/946 or 2606.14008 frames the problem as HNDL against unlinkability. Per the ePrint abstract page, 2024/946 does not mention it.
- **The defensible claim** is therefore "first systematic analysis and measurement of retroactive pseudonym linkage in SCMS", not "first to identify the mechanism". The authors' own phrasing hints at that, but the paper must cite and position against everlasting privacy and PQ-Tor explicitly. Otherwise a PoPETs PC will call it "HNDL applied to SCMS, obvious".

### W3 (major): the migration-order conclusion does not follow as stated

- **(a) The order is the textbook HNDL ordering.** Confidentiality migrates before authentication because authentication only has to hold at verification time. NIST/NSA guidance says this, and ETSI's own Mosca-style X+Y>Z framing (TR 103 949 p. 14–15, citing ETSI GR QSC 004) implies it. The new content is only that provisioning confidentiality *exists and matters* in V2X, so present the ordering as a corollary, not a contribution.
- **(b) "Before infrastructure certificates" hides a dependency.** To encrypt to the RA, PCA and LAs with a PQ KEM, those authorities' *encryption public keys* must be distributed in their certificates, and the 1609.2 certificate format has to carry ML-KEM keys. So the infrastructure certificate format and re-issuance must change first, even if the certificates stay ECDSA-signed. The right statement is roughly:
  - first, the encryption keys in infrastructure certificates;
  - second, the signature algorithms on infrastructure certificates;
  - third, end-entity signatures.
- **(c) "First" in priority is not "first to start".** BSM signatures have the longest migration time Y, because of airtime and packet-size constraints. Twardokus, Bindel, Rahbari & McCarthy, NDSS 2024, ePrint 2022/483, show plain PQ signatures barely fit V2V spectrum. A paper that reads as "deprioritise signatures" would be wrong policy. The conclusion should be phrased as a parallel track with a deadline ordering.

### W4 (major): claim (3) misrepresents or under-cites the standards position

- **ETSI TR 103 949** does not literally say "HNDL is not a main concern". It says confidentiality protection duration is "N/A" and disclosure impact "None" (Table 3, p. 16), and its risk analysis is about masquerade (§4.4, p. 14). Quote that precisely. It is a stronger and more honest target than a paraphrase.
- **The "5GAA/GSMA PQC report of October 2026"** could not be located in my search, so the authors must give an exact citation and quotation.
- **What is visible from 5GAA points the other way.** A June 2026 5GAA presentation at a GSMA MWC26 Shanghai roundtable lists among "Why PQC Matters": "'Harvest now, decrypt later' already relevant" (slide 24). Its "Key Risks" list includes "Privacy breaches" (slide 30). Source: gsma.com, MWC26-Shanghai_PQC-Implementations-Roundtable_23062026.pdf, PDF pp. 23 and 29. That deck is marked "internal discussion only … does not reflect any agreed 5GAA view".
- If the October report really says HNDL is not a main V2X concern, quote it. If it only says broadcast BSMs are unencrypted, then the disagreement is narrower than claimed, and the paper should say so.

### W5 (moderate): the attack realism and cost model are missing

- **Count the discrete logs per path.**
  - *Outside recorder:*
    - the RA's static ECIES key (once);
    - per vehicle, the caterpillar encryption key (1 per request; arXiv:1802.05323, Fig. 5 p. 16 shows separate signing/encryption caterpillars A and J);
    - each TLS session's ECDHE.
  - *RA archive:* the LA1 and LA2 keys, two in total, for the whole fleet (W1).
- **Why it matters.** Early CRQCs will be slow and scarce. Gidney, arXiv:2505.15917, estimates under a week per RSA-2048 instance on less than 1M noisy qubits; ECC-256 estimates differ but are of the same order of concern. A threat that needs O(1) dlogs per *fleet* is much more credible than one needing O(1) per *vehicle-session*. This should be a headline axis, since it changes which adversary matters first.
- **The BSM side is also expensive.** It requires mass roadside capture of BSMs, retained for years.
- **Measure the marginal harm.** Trajectory linking already defeats simple pseudonym change: Wiedersheim, Ma, Kargl & Papadimitratos, *Privacy in Inter-Vehicular Networks: Why simple pseudonym change is not enough*, https://people.kth.se/~papadim/publications/fulltext/privacy-vanet-pseudonyms-enough.pdf. The paper must therefore measure the *marginal* linkability a CRQC adds over a tracking baseline. Two cases matter:
  - linking across coverage gaps and days, where tracking fails;
  - linking to the enrolment identity, which tracking cannot do.
  Without this, a PC member will say the crypto linkage adds little to what a roadside sniffer network already gets.

### W6 (moderate): the PQ response-encryption fix is not a drop-in replacement and is under-specified

- **Why ML-KEM does not drop in.** Unlinkability needs per-certificate encryption keys that the RA can derive publicly but that the PCA cannot link. ML-KEM has no key-homomorphic expansion.
- **The options are:**
  - **(i)** PQ BKE (ePrint 2018/1247; ePrint 2024/946);
  - **(ii)** device-generated independent ML-KEM keys per certificate. For a 3-year preload at 20 certificates per week (arXiv:1802.05323, p. 7), about 3,120 encapsulation keys at 1,184 B each for ML-KEM-768 (FIPS 203), about 3.7 MB upstream, plus about 1.1 KB of ciphertext per response;
  - **(iii)** a hybrid of the two.
- **Ciphertext anonymity.** Option (ii) also needs anonymous (key-private) KEM ciphertexts so the RA cannot cluster responses. Grubbs, Maram & Paterson, EUROCRYPT 2022, ePrint 2021/708, showed this is non-trivial for Kyber. Maram & Xagawa, ePrint 2022/1696, later established post-quantum anonymity of Kyber.
- **Integration with existing proofs.** The only formal analysis of the classical BKM, Boldyreva, Kumar & Sun, ePrint 2024/1674 (PETS), required small protocol modifications to meet its privacy goal. Any PQ variant should be stated within, or as an extension of, that model.
- **The cost metric.** "Airtime" is only meaningful if provisioning runs over RSUs/sidelink. arXiv:1802.05323 (p. 7) describes download over a TCP/IP connection. The authors should state which provisioning bearer they model and why.

### W7 (moderate): the evaluation plan is not yet convincing

- **"Linkable pseudonym-years" is largely analytic.** Under a given migration date, preload horizon, top-up period and recording coverage, it reduces to a closed-form integral. The authors should give that model first and use the simulator only for what needs it:
  - **(a)** BSM-capture coverage by a realistic sniffer deployment;
  - **(b)** marginal linkage over trajectory tracking (W5);
  - **(c)** fleet-mix dynamics on the real calendar.
- A city-scale simulator producing a number that equals a spreadsheet formula will not impress a PoPETs PC.
- **Validate the simulator's SCMS** against the specification and the published design, including the LCI/PLV flows and the LOP (arXiv:1802.05323, p. 15), which the claim currently omits.

### W8 (minor): adversary placement and protocol facts need pinning down

- **Where does TLS terminate?** Device→LOP→RA, a CDN front-end, or end-to-end. If it terminates at a front-end, an outside recorder inside the provider sees classical inner ECIES only.
- **What exactly is signed by the enrolment certificate, and what is visible?** This covers the request and the download request. arXiv:1802.05323 (p. 7) says batches are "named using device information and time period". If download requests carry device-identifying names, the outsider gets identity from the download leg too. What is visible to a *classical* recorder (TLS 1.2 client certificates sent in the clear, file names) should also be stated.
- **Other public-key-protected flows** that could add linkage should be enumerated: misbehaviour report upload, enrolment-certificate roll-over (p. 21), and the RA–PCA request-hash records (Fig. 5, p. 16).

## 4. Novelty assessment

**Search scope:** arXiv, IACR ePrint, web.

| Prior work | What it covers | Relation to this claim |
|---|---|---|
| qSCMS, ePrint 2018/1247 | PQ butterfly expansion (lattice) | Fixes the mechanism; no HNDL or linkage analysis |
| Eaton et al., ePrint 2024/946 | Provably unlinkable PQ BKE | Gives a building block for the fix; no threat or migration analysis |
| Boldyreva et al., ePrint 2024/1674 (PETS) | Classical BKM security model | The model the PQ variant should extend |
| Chen et al., arXiv:2606.14008 | Hybrid ECC/PQC certificates; ML-KEM on requests | Overlaps on the fix (request side); no retroactive-linkage framing or quantification |
| Twardokus et al., NDSS 2024 / ePrint 2022/483 | PQ authentication for V2V | Signature side; shows long Y for BSMs |
| Ghosh & Kate, ePrint 2015/008; QADR arXiv:2511.15272; Moran & Naor 2006 | Future/retroactive deanonymisation, everlasting privacy | The general principle; must be cited |
| ETSI TR 103 949 (2023) | ITS QSC migration study | States confidentiality "N/A" for C-ITS (p. 16); good foil |
| Mamun et al., arXiv:2601.01068; Abrar et al., arXiv:2608.05087 | ITS PQC reviews / feasibility | Mention HNDL generically (e.g., 2601.01068 pp. 4, 12, 22), not SCMS pseudonym linkage |

**Verdict.** Novel as an SCMS-specific threat analysis with measurement, particularly if W1 (LCI/PLV) is incorporated, since that path appears in none of the works above. Not novel as a mechanism. Scoop risk comes mainly from the Chen et al. line and from the PQ-BKE authors, who could add a short motivation paragraph at any time.

## 5. Technical correctness of the threat model and attack chain

| Step | Assessment |
|---|---|
| Outsider records TLS-protected request, breaks TLS ECDHE and RA ECIES to get enrolment cert, A/J and expansion seeds | Correct, given classical TLS and the request structure in arXiv:1802.05323 p. 15, Step 1 |
| Outsider records download and decrypts responses | Correct. Most cheaply via the dlog of the encryption caterpillar plus the seed, rather than per-response ephemerals |
| Links certificates to enrolment identity | Correct |
| Links archived BSM tracks | Correct if BSMs were captured. Certificate digests in BSMs map to the recovered certificates |
| "Broadcast only" / "requests only" insufficient | Correct for the outsider |
| "Linkage values remain safe" | **Incorrect for the RA-archive and backend-link adversaries** (W1) |
| "Hybrid TLS stops the outsider" | Correct only if TLS covers every recorded hop, including backend links and the LOP |
| "RA-archive needs PQ response encryption" | Necessary but **not sufficient**. PQ PLV and LCI encryption plus re-seeding are also needed (W1) |
| "Years of certificates" | Understated for the LCI path: device lifetime |

## 6. Does the conclusion follow?

- **Partly.** "Provisioning confidentiality must be post-quantum early" follows.
- **What does not follow:**
  - "response encryption plus TLS" is the complete set (W1);
  - this ordering is novel (W3a);
  - it precedes all infrastructure-certificate work (W3b);
  - both standards bodies concluded the opposite. Half supported for ETSI, unverified for 5GAA/GSMA (W4).

## 7. Would the evaluation plan convince me?

Not yet. I would be convinced by four things:

1. A closed-form exposure model validated by the simulator.
2. A per-path count of quantum work, per vehicle and per fleet.
3. A marginal-linkability experiment against a trajectory-tracking baseline under realistic sniffer coverage.
4. A cost comparison of concrete PQ provisioning designs (PQ-BKE vs. independent ML-KEM keys vs. hybrid), in bytes on the actual provisioning bearer, against a clear security definition, ideally extending ePrint 2024/1674's model.

## 8. Required changes

1. Add the LCI/PLV transport path (W1). Revise claims (1) and (2), and add re-seeding linkage chains at migration as a required step.
2. Rescope novelty to "first SCMS-specific analysis and measurement". Cite and position against everlasting privacy, PQ-Tor and the PQ-BKE line (W2).
3. Restate the ordering:
   - encryption keys in infrastructure certificates and provisioning KEMs first;
   - signatures in parallel with their longer lead time.
   Drop any implication that signatures can wait (W3).
4. Quote the standards precisely (TR 103 949 Table 3, p. 16; §4.4, p. 14). Give a verifiable citation for the October 2026 5GAA/GSMA report, and reconcile it with the June 2026 5GAA deck (W4).
5. Add a quantum-cost-per-path analysis and a marginal-over-tracking privacy metric (W5, W7).
6. Specify the PQ response-encryption design and its unlinkability argument (W6).
7. Pin down the TLS termination point, the identifying fields on the download leg, and every public-key-protected flow (W8).

## 9. Questions to the authors

1. Does your SCMS model store LCIs and encrypted PLVs at the RA? If so, why do linkage values not give the RA-archive adversary full-lifetime linkage after two LA-key discrete logs?
2. Which exact sentence in the October 2026 5GAA/GSMA report says HNDL is not a main concern for V2X?
3. In the deployed US design, does device TLS terminate at the LOP, a CDN or the RA? Is client authentication done in TLS (and visible to a classical observer in TLS 1.2)?
4. Under unified butterfly keys, where one caterpillar serves both signing and encryption, does your attack chain change?
5. Which provisioning bearer do you model for "airtime"? Is any US deployment provisioning over RSUs?
6. What does the simulator add beyond a closed-form exposure integral, quantitatively?
7. How do you set the adversary's BSM capture coverage, and what baseline tracking attack do you compare against?
8. For the PQ response-encryption design, what is your unlinkability definition, and does it reduce to the model of ePrint 2024/1674?

## 10. Overall score

**Major revision.** The direction is sound and PoPETs-relevant. The central insight, that a CRQC retroactively acts as the colluding second authority, is correct and under-stated in the standards. As stated, though, the claim:

- gets the linkage-value part wrong for the stronger adversary, which also makes the proposed fix incomplete;
- over-claims novelty and the ordering;
- rests one pillar on a source I could not verify.

All of these can be fixed, and fixing W1 makes the paper stronger.

**Confidence: 4/5.** I verified the SCMS flows against arXiv:1802.05323 but not against the paywalled IEEE 1609.2.1 text itself. In particular, whether the deployed RA retains LCIs and encrypted PLVs in the form Brecht et al. describe should be confirmed against the standard and the CAMP specifications.

---

### Sources consulted (all accessed 2026-10-18)

- Brecht et al., arXiv:1802.05323 (IEEE T-ITS 2018). pp. 5, 7, 11–16, 21 as cited.
- ETSI TR 103 949 V1.1.1 (2023-05), https://www.etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf. §4.4 p. 14, Table 3 p. 16.
- Barreto, Ricardini, Simplicio, Patil, qSCMS, ePrint 2018/1247.
- Eaton, Lamontagne, Matsakis, ePrint 2024/946.
- Boldyreva, Kumar, Sun, ePrint 2024/1674 (PETS).
- Chen, Hwang, Wei, Chang, Lin, arXiv:2606.14008. Abstract p. 1; pp. 9–10.
- Twardokus, Bindel, Rahbari, McCarthy, NDSS 2024; ePrint 2022/483.
- Ghosh & Kate, ePrint 2015/008 (ACNS 2015).
- Moran & Naor, CRYPTO 2006, DOI 10.1007/11818175_22.
- QADR, arXiv:2511.15272.
- Grubbs, Maram, Paterson, ePrint 2021/708 (EUROCRYPT 2022); Maram & Xagawa, ePrint 2022/1696.
- Gidney, arXiv:2505.15917.
- Wiedersheim, Ma, Kargl, Papadimitratos, https://people.kth.se/~papadim/publications/fulltext/privacy-vanet-pseudonyms-enough.pdf.
- Mamun et al., arXiv:2601.01068 (pp. 4, 12, 22); Abrar et al., arXiv:2608.05087 (p. 3).
- 5GAA, "PQC4Auto" slides in the GSMA MWC26 Shanghai roundtable deck (23 June 2026), https://www.gsma.com/solutions-and-impact/technologies/security/wp-content/uploads/2019/03/MWC26-Shanghai_PQC-Implementations-Roundtable_23062026.pdf. PDF pp. 22–23, 29; marked internal / not an agreed 5GAA view.
- 5GAA–GSMA statement of intent (March 2025), https://5gaa.org/5gaa-gsma-issue-statement-of-intent-on-the-quantum-threat-and-mitigation-strategies-for-the-automotive-sector/.
- FIPS 203 (ML-KEM-768 sizes: encapsulation key 1,184 B, ciphertext 1,088 B).
