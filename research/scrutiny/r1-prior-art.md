# R1: blind prior-art hunt for "harvest now, link later"

Scope: has anyone already said that (1) a quantum adversary who records SCMS/CCMS provisioning traffic, or who gets an RA archive, can link pseudonym certificates (and the BSM tracks signed with them) back to enrolment identity after the fact, or (2) that the provisioning path should therefore migrate to PQC first, for privacy?

Method: about 20 minutes, done independently (no repo synthesis or scrutiny files read).
- Searched: web search with about 15 phrasings, OpenAlex keyword search (9 queries), arXiv, DBLP.
- Read full texts (downloaded PDF, extracted with pypdf, then searched and read the relevant sections) of 14 documents.
- ePrint is behind a Cloudflare challenge. 2024/946 and 2018/1247 were read from Wayback Machine copies (2024/946 snapshot 20251231124434; 2018/1247 snapshot 2020).

## Verdict

**Partly stated. The specific claim appears novel. Its ingredients and parts of its design consequence are already in print.**

- **Claim (1), retroactive linkage from recorded provisioning or an RA archive:** no source found states it.
  - Two ingredients are published:
    - (a) BKE unlinkability against the RA and the CA, and against outsiders, rests on ECIES/ECDLP, which a CRQC breaks (Chen et al. 2026).
    - (b) RA-unlinkability is "enforced through the encryption of the certificates", so PQ BKE needs a PQ KEM for the response (Eaton et al. 2024/946).
  - Generic "HNDL threatens vehicular privacy" statements also exist, with no mechanism given (Mamun 2026, Abrar 2026, Awasthi 2026, Karinsalo 2026).
  - No source connects these into "record now, then link years of pseudonym certificates and BSM tracks to the EC later".
  - Several sources assert the **opposite**: pseudonym certificates are quantum-safe enough because they are short-lived (Chen 2026; Lonc 2025; Twardokus NDSS 2024). That is the forgery view, and it is the foil the thesis needs.
- **Claim (2), migrate the provisioning path first, for privacy:** the *design* is partly anticipated, but not the *reason* or the *ordering argument*.
  - Chen 2026 and Lonc 2025 both put PQ or hybrid KEMs on provisioning encryption while keeping ECDSA in pseudonym certificates and BSMs.
  - Mamun 2026 summarises a "partially hybrid design, where Falcon protects credential material during certificate provisioning and ECDSA signs routine broadcast messages".
  - Their motivation is bandwidth and forgery. They migrate infrastructure certificates at the same time or first.
  - ETSI TR 103 949 recommends a different order: rollout "begin at the bottom of the hierarchy".
  - No source argues "provisioning encryption before infrastructure certificates and BSM signatures, because of unlinkability".
  - No source separates the outside-recorder case (fixed by hybrid TLS) from the RA-archive case (needs PQ response encryption, a 1609.2.1 change).
- **Claim (3), the standards bodies' view:** confirmed for 5GAA/GSMA. For ETSI it is accurate in substance but needs careful wording (details below).
- **Claim (4), quantified linkable pseudonym-years by migration order and provisioning design:** nothing comparable found.

## Closest works, one by one

### Chen, Hwang, Wei, Chang, Lin 2026: hybrid-certificate pseudonym scheme
arXiv:2606.14008; IEEE Canadian J. Electr. Comput. Eng., doi:10.1109/ICJECE.2026.3692881.

This is the closest work and the main scoop risk.

**On quantum and unlinkability:**
- §II.B.3 "Security Discussions of PC" (p.6) walks through the IEEE BKE flow. It concludes that, as long as ECDLP is hard for the PCA, the RA and other EEs, none of them can relate the enrolment public key to the butterfly key. "This property ensures the confidentiality and unlinkability of the PC system. However, since the system's security relies on the hardness of the ECDLP, it may become vulnerable in the presence of quantum computing technologies."
- Its new scheme (§III.B.2, pp.10–11) encrypts the PC public key to the PCA under ML-KEM and returns the PC "encrypted using a pre-shared key and the AES algorithm", so the RA cannot link. §IV.E.2 (p.17) argues EC-to-PC unlinkability against a quantum man-in-the-middle because "the attacker cannot break ML-KEM".
- Appendix C (p.21) adds that BKE privacy rests on ECDLP, while the new scheme gives "privacy protection based on the SVP and LWE problem, thus achieving quantum-safe security".

**What it does not say:**
- Nothing about recording now and decrypting later, nothing about retroactive linkage of BSM tracks, and nothing about an RA archive.
- It contradicts the thesis's threat view. §IV.E.1 and Table XII (p.16) mark the PC (1 week) as "Pass", meaning resistant to quantum attacks under today's IEEE standards, because "short-lived certificates for EEs can mitigate the risk of quantum attacks". Only infrastructure certificates and ECs are marked "Fail".

**On provisioning:** its design does PQ-protect provisioning encryption and keeps ECDSA PCs (§IV.F, p.17: adds `mlkem512` to `BasePublicEncryptionKey`). It also migrates the RCA, ICA, ECA, PCA and RA to pure PQC; it does not order the migration.

**Net:** this paper states ingredient (a) and builds most of the thesis's preferred design, but for different reasons. The thesis must cite it and position against Table XII.

### Eaton, Lamontagne, Matsakis: provably secure BKE from CRYSTALS
IACR ePrint 2024/946.

- **Quantum:** the whole BKE (Kyber and Dilithium with key expansion) is post-quantum.
- **Unlinkability:** §2.3 (p.5) defines UL-CA and UL-RA games and assumes the RA and CA do not collude. §2.3.2 (p.6): "unlinkability against the RA is enforced through the encryption of the certificates using the Kyber expanded public keys … it will not be able to link an expanded Dilithium key with its certificate to the vehicle's identity". §2.3 also notes these models "imply security … against an external adversary not involved in the certificate provisioning process".
- **Provisioning, HNDL, archives, migration:** nothing. There is no harvest or recording adversary, no TLS, no RA archive, and no discussion of migration order. The motivation is generic PQ readiness.

**Net:** this gives the formal basis for "an RA-archive adversary needs PQ response encryption", but does not state the threat.

### Barreto, Ricardini, Simplicio, Patil: qSCMS
IACR ePrint 2018/1247; also Ricardini's USP thesis 2019, doi:10.11606/T.3.2019.tde-17072019-135843; patent US 2021/0211306 / US11165592B2.

- §1 (pp.1–2): ECC "is known to be susceptible to quantum attacks. Such an attack can be devastating, since it would violate not only the system's privacy, but also its authenticity."
- It proposes a lattice-based unified butterfly key expansion (§8 conclusion, p.15).
- Nothing on recording now and linking later, or on migration order.

**Net:** a generic "quantum breaks privacy" sentence.

### Boldyreva, Kumar, Sun: provable security of BKM in IEEE 1609.2.1
PoPETs 2024(4), doi:10.56553/popets-2024-0130.

- §1 threat model (p.2): "The communication between EEs and the RA is private and authenticated." EE–RA traffic is a secure channel the adversary cannot intervene in (§4, p.8). §3 (p.6) says 1609.2.1 "specifies several mechanisms for secure channels including TLS1.2, TLS1.3 and ISO/TS21177".
- Unlinkability rests on Oracle Diffie-Hellman for ECIES with related keys (§1, p.3; Thm 7.2).
- Quantum is mentioned only as future work: "studying ways to achieve post-quantum security of the protocol" (§8, p.13).
- Nothing on recording or archive adversaries.

**Net:** this is the formal home of the thesis's mechanism. It states the classical assumptions that a CRQC would remove, but not the consequence.

### Twardokus, Bindel, Rahbari, McCarthy: "When Cryptography Needs a Hand"
NDSS 2024, https://www.ndss-symposium.org/wp-content/uploads/2024-267-paper.pdf.

- Scope (§I, p.3): "Confidentiality is out of our scope as safety messages are never encrypted … As certificate management and distribution are much broader problems, we consider them only when they directly affect this work."
- The threat is forgery of BSMs within the one-week PC validity.
- The predecessor, Bindel et al. ePrint 2022/483 "Drive (Quantum) Safe!", likewise excludes data encryption and key exchange (related-work section, p.15).

**Net:** forgery only. Explicitly silent on provisioning and privacy.

### Lonc, Braiteh, Bassi: hybrid key exchange and signatures for quantum-safe C-ITS
NTMS 2025, doi:10.1109/NTMS65597.2025.11076918; HAL hal-05074844.

This is the closest CCMS (ETSI TS 102 941) analogue.

- §III (pp.4–6) uses AT (pseudonym) request provisioning under TS 102 941 as the worked example of ECIES. It proposes a hybrid ECIES + ML-KEM key establishment using CatKDF (ETSI TS 103 744).
- It keeps ECDSA "partial hybrid" ATs because "even a quantum attacker cannot forge ECDSA signatures within such short time" (p.8).
- Motivation: long vehicle lifetimes. There is no HNDL, privacy or linkability argument, and no migration-order argument.
- The predecessor, Lonc et al. VNC 2023 (doi:10.1109/VNC57357.2023.10136335; hal-04050027), benchmarks KEM replacements for ECIES, again with no privacy rationale.

**Net:** the hybrid provisioning encryption already exists for CCMS, which weakens the "concrete 1609.2.1 change" novelty. The privacy rationale is absent.

### Mamun, Abrar, Rahman, Salek, Chowdhury: PQC for ITS, implementation-focused review
Vehicular Communications 60:101028 (2026), doi:10.1016/j.vehcom.2026.101028; arXiv:2601.01068.

- Generic HNDL: p.4 "encrypted data intercepted today could be decrypted"; §3.3 p.12 "'harvest-now-decrypt-later' and 'forge-later' attacks against long-lived infrastructure and archived vehicular data".
- p.29 describes the partial hybrid in which "Falcon protects credential material during certificate provisioning and ECDSA signs routine broadcast messages", justified by channel load.
- Research gap RG8 is "pseudonym certificate provisioning strategies" for PQC (p.38), framed as scalability.
- No linkability mechanism.

The same group's Abrar et al. 2026 (arXiv:2608.05087, §1 p.2) says HNDL is a privacy concern because V2X "messages include precise location, speed, and trajectory information". That sentence misattributes HNDL to signed, unencrypted broadcasts, and gives no provisioning mechanism.

**Net:** this group is the second scoop risk. They are one step away from the idea.

### Awasthi 2026: quantum security risks in AVs
IJCESEN 12(1), doi:10.22399/ijcesen.4942.

- §3.1 (p.2) describes HNDL on "encrypted vehicular traffic … V2V safety messages", yielding location histories.
- That is technically wrong for V2V, which is signed and not encrypted, and it gives no pseudonym or provisioning mechanism.

**Net:** generic. Citable only as an example of the imprecise folk version.

### Karinsalo, Nikula, Lehtonen 2026: migrating SWIM and C-ITS PKIs
ICCWS 2026, doi:10.34190/iccws.21.1.4479.

- §1 (p.1) mentions HNDL as a generic reason to plan migration.
- The rest is about signature size and latency. No privacy.

### ETSI TR 103 949 V1.1.1 (2023-05)
https://www.etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf

The term "harvest" does not appear. In substance it treats confidentiality as a non-issue:
- **Table 2 (p.16):** "As CAMs and DENMs are sent in clear there is no risk of information disclosure. The risk is that the trust network is broken." It also says that "Privacy of the user is to be maintained across the lifetime of the ITS-S". That line supports the thesis.
- **Table 3 (p.16):** PII is protected "by use of pseudonymous keying … rather than by encryption". Protection duration is "N/A". Disclosure impact is "None per se". The plan to protect encrypted assets is "N/A".
- **Table 5 (p.17):** the concern is that all issued certificates "will need to be updated", which is the forgery view.
- **Annex C (p.23):** "it is recommended that the rollout of quantum safe crypto capabilities begin at the bottom of the hierarchy". This is a migration-order claim, though about capability rollout rather than privacy.
- Clauses 5.3–5.4 never consider the EA/AA request encryption (ECIES) as an asset with a confidentiality lifetime.

**Wording for the thesis:** do not write that ETSI "concluded HNDL is not a main concern". Write that it assessed disclosure risk as none and protection duration as N/A, and did not treat the provisioning encryption as a confidentiality asset.

### 5GAA/GSMA, "Post-Quantum Cryptography for Automotive – Impact Assessment" (GSMA PQ.08), Oct 2026
https://5gaa.org/post-quantum-cryptography-for-automotive-impact-assessment/

- §3.3 "The quantum threat" (PDF p.10, printed p.7): "many V2X use cases, such as safety and mobility use cases, do not use confidential information and make use of information with a very short active or relevant lifetime … so this 'harvest-and-decrypt' attack … is not the main concern for V2X."
- It names "financial interactions or upload of personal information for insurance" as possible HNDL targets. It does **not** name certificate provisioning, pseudonyms, butterfly keys or SCMS (no textual hits).
- §5.2.3 table (PDF p.29) lists BSMs as "Anti-tracking", with encryption "No".
- Scoop signal (PDF p.33): ETSI TC ITS is extending its TVRA with a cost-benefit analysis of quantum mitigations; the IEEE 1609 WG "is starting the process to develop extensions to IEEE 1609.2 to support PQC"; and the Car-2-Car Communication Consortium "is developing a white paper on the migration process". None of these was found published.

**Net:** claim (3) is verified as quoted for 5GAA/GSMA.

### Other works checked: generic, nothing on HNDL-linkability
- Yoshizawa & Preneel, VTC2023-Spring, doi:10.1109/VTC2023-Spring57618.2023.10199793: certificate format and forward compatibility.
- Chen arXiv:2401.13691 (PQCMC) and arXiv:2607.13554 (multivariate anonymous certificates): about schemes, with no HNDL.
- Mushrall et al., VTC-Fall 2018, doi:10.1109/VTCFall.2018.8690778: abstract says BKE "do[es] not withstand post-quantum cryptanalysis".
- Chouhan 2026, Zenodo doi:10.5281/zenodo.19488046: abstract only, a layer-by-layer migration map with no privacy mechanism in the abstract. The full text was not read, so this is a residual risk.
- Rathweg et al., "Impacts of PQC on Automotive Security" (https://d-nb.info/1316435830/34): no HNDL, unlink or linkab hits.
- Malina et al., IEEE Access 2021, doi:10.1109/ACCESS.2021.3062201: generic post-quantum privacy-enhancing technologies.

## Could not verify

- **Fikry et al. 2026 (IEEE ASSIC, "post-quantum aware SCMS"):** not found in OpenAlex, nor in three web searches by author, venue and title phrasings. The citation may be wrong, or the paper may not be indexed yet. It stays a residual novelty risk.
- **Chen et al. arXiv:2501.07028** (Chinese hybrid SCMS field test; also doi:10.1109/ICCPCT61902.2024.10673280): abstract only. It says ECC keeps certificates anonymous and PQC protects infrastructure.
- **Salman & Blankinship 2022** (doi:10.1007/978-981-19-1607-6_45, PQ BKE): paywalled, abstract empty.
- **ETSI TS 102 941:** the PDF could not be fetched (ETSI deliver returned HTML). The CCMS analogue (AT request ECIES-encrypted to the AA, with the EC signature inside) is therefore taken from Lonc 2025 §III, not checked against the standard.

## What this means for the claim's wording

1. Claim **the threat statement and its quantification** as first, not "PQ provisioning". Chen 2026 and Lonc 2025 already have PQ or hybrid provisioning encryption, and Eaton 2024 already makes RA-unlinkability depend on the response KEM.
2. Use Chen 2026 Table XII, Lonc 2025 p.8 and Twardokus 2024 as the explicit foil: "short PC lifetime makes PCs quantum-safe" holds for forgery and fails for retroactive linkage.
3. Cite Boldyreva 2024 for the assumptions (private EE–RA channel; ODH for ECIES) that a CRQC plus a recording removes.
4. Quote 5GAA/GSMA §3.3 verbatim. Paraphrase ETSI TR 103 949 Tables 2–3 as above.
5. Scoop watch: the Chen group (NIST and Taiwan), the Mamun/Abrar/Chowdhury group (Clemson/USF), and the pending ETSI TVRA update and C2C-CC white paper.
