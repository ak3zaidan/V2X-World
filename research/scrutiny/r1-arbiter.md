# R1 arbiter: "harvest now, link later" in the US SCMS

Date: 2026-10-18. Role: arbiter over five independent round-1 inputs. Inputs, all read in full:

| Tag | File | Independence |
|---|---|---|
| [BLIND] | `r1-blind-cryptanalysis.md` | Derived its own attack model from project docs D02/D03 plus primary sources. It never saw the claim. |
| [PRIOR] | `r1-prior-art.md` | Blind prior-art hunt; 14 full texts, including ePrint 2024/946 and 2018/1247 through Wayback copies. |
| [FACT] | `r1-fact-check.md` | Primary-source check of premises (a)–(j). |
| [REV-A] | `r1-review-crypto.md` | Applied-crypto reviewer. Major revision, confidence 4/5. |
| [REV-B] | `r1-review-systems.md` | V2X-systems reviewer. Major revision, confidence 4/5. |

**Arbiter's own check.** Only one point was in dispute, so I checked only that one against a primary source. I fetched the IEEE 1609.2.1-2022 ASN.1 from ETSI Forge (tag `2022-published`):
- `Ieee1609Dot2Dot1AcaRaInterface.asn` l.182–201: `EncryptedIndividualPLV.encPlv` is `Ieee1609Dot2Data-SymmEncryptedSingleRecipient`. Its note reads "How the ACA obtains the shared symmetric key … [is] outside the scope of this document".
- `Ieee1609Dot2Dot1LaRaInterface.asn` l.12: `LaRaInterfacePdu ::= NULL`.
- `Ieee1609Dot2Dot1LaMaInterface.asn` l.12: `LaMaInterfacePdu ::= NULL`.
- `Ieee1609Dot2Dot1Protocol.asn` l.601–608: the optional single response encryption key (`additionalParams.encryptionKey`) confirms [FACT](d).

Source: https://forge.etsi.org/rep/ITS/asn1/ieee1609.2.1/-/raw/2022-published/

**Caveat on independence.** [BLIND] worked from the project's own design documents D02/D03. Its structural agreement with the claim is therefore corroboration, not fully independent replication. Its departures from the claim carry more weight than its agreements.

---

## 1. Point-by-point comparison: blind cryptanalysis vs. claim (1)–(2)

### 1.1 Where they agree

| # | Claim element | [BLIND] | Others | Arbiter |
|---|---|---|---|---|
| A1 | Recorded broadcast BSMs alone give no quantum linkage gain | §2.1: U = B + cG with fresh uniform c statistically hides B. Shor on pseudonym, ECQV or ACA keys yields forgery only (arXiv:1802.05323 p.10). | [REV-A] S4; [REV-B] §6 ("correct cryptographically") | **Holds.** It is cryptographic only. Classical trajectory tracking still links ([REV-B] W3). |
| A2 | Requests alone do not suffice | §2.2: the request yields A, ck, H, ek and the enrolment certificate, but not c | [REV-A] S4; [FACT] further-finding 2 (arXiv:1802.05323 Fig. 2) | **Holds for an outsider.** It fails for an RA-state holder if LCIs or the LA–ACA key are public-key protected (§1.3, item D1). |
| A3 | Request plus download plus CRQC links years of pseudonyms to the enrolment identity | §2.3: one Shor on H opens every response for the request's lifetime. Path P1 in §3. | [REV-A] §5 table ("Correct"); [REV-B] §6 | **Holds.** [BLIND] adds that the cheapest step is one discrete log on the caterpillar encryption key H, not one per response. |
| A4 | An RA archive suffices and is the strongest harvest target | §2.8: "highest-value harvest target"; one Shor per vehicle on H. Corrupted-RA privacy in Boldyreva Thm 5.1 rests on HDH (Thm 6.1), which Shor breaks. | [FACT](j) confirms the HDH dependence (PoPETs 2024, doi:10.56553/popets-2024-0130, Thms 5.1/6.1). [FACT](e): retention of response ciphertexts is UNVERIFIED. | **Holds conditionally.** The RA must keep caterpillar keys for top-ups (arXiv:1802.05323 pp.7, 15). It is unconfirmed whether it keeps the encrypted zips beyond the re-download window. |
| A5 | No transport fix helps against RA storage; PQ response encryption is needed | §5(c) | [REV-A] W6; [REV-B] §6; Eaton et al. ePrint 2024/946 §2.3.2 p.6 ([PRIOR]) | **Holds as necessary, not sufficient** (see D1, D3). |
| A6 | Signatures do not matter for passive linkability | §3 last line; §5: "Signatures are not in either minimum set" | [REV-A] W3, [REV-B] W2: true, but signatures must still start early | **Holds for privacy.** It must not be read as "signatures can wait" (§2, sub-claim 2a). |

### 1.2 Where they disagree

| # | Claim says | [BLIND] says | Others | Arbiter ruling |
|---|---|---|---|---|
| D1 | "Linkage values (symmetric constructions) remain safe" | **Conditional.** The PRF and hash chain is safe (Grover about 2^64 per chain, §2.1, §2.7). But the LCIs are "e.g., E(pk1, ls1(0))" (arXiv:1802.05323 p.13). If they are public-key in deployment, two Shor runs (LA1, LA2) give every seed for every device (P0′, §3). The LA–ACA PLV key is also unspecified (§2.5). | [REV-A] W1 and [REV-B] W1 both rate this **critical**. [FACT] further-finding 1: the PLV route depends on how the LA–ACA key is set up. | **Claim is wrong as stated; drop the sentence.** Correct statement: the linkage computation is quantum-safe, but the containers that carry seeds and PLVs may not be. Correction to the reviewers: [REV-B]'s "one discrete log on the PCA's PLV-decryption key" and [REV-A]'s "decrypts plv1, plv2 with a CRQC" assume public-key PLV encryption. The 1609.2.1-2022 ASN.1 makes PLVs **symmetric** (arbiter check above). The PLV route therefore exists only if the LA–ACA key is established by ECDH or ECIES and that exchange is recorded or stored. The LCI route rests on Brecht's "e.g." and on the LA–RA interface, which is `NULL` in the standard, so it is deployment-defined. Both routes become **parameters** of the model, not facts. |
| D2 | "Years of certificates" | The response route is bounded by the request's coverage (up to `ra-maxPreloadTime` = 3 years, D03 §6.4). The LCI route covers all periods, past and future. | [REV-A] W1(b); [REV-B] §6: the LCI route covers the whole chain, while for an outsider with small, frequent top-ups the figure may be overstated | **Corrected.** Report the exposure per path. The bound differs: request coverage, download sessions recorded, or the whole chain. |
| D3 | "Hybrid TLS alone stops an outside recorder" | **Agrees only for a recorder on the EE access network.** It also covers downloads and misbehaviour uploads (§5(a)). It fails for a recorder that sees RA↔ACA, RA↔LA or RA↔MA traffic (§5(b), paths P2–P4). | [FACT](a) and further-finding 3: it holds only if TLS runs end to end to the RA, which is UNVERIFIED. Brecht p.15 describes the LOP as NAT-style. [REV-B] W4 adds OEM proxies and load balancers. | **Corrected and narrowed.** It holds iff the recorder sits only on the EE access side **and** TLS is end-to-end to the RA application **and** the request, the download and the misbehaviour upload all ride that TLS. Otherwise the inner ECIES to the RA's static key is the only layer, and one Shor on that key opens every request ever sent to that RA certificate ([BLIND] §2.2). |
| D4 | "The RA-archive adversary needs PQ response encryption (a 1609.2.1 change)" | §5(c): it also needs PQ misbehaviour-report encryption and PQ or symmetric LCIs. §6: data already harvested can be protected only by retiring the exposed caterpillars. | [REV-A] W1(a): PQ PLV and LCI encryption plus re-seeding chains. [REV-B] W1: the LCI and LA interfaces are not in 1609.2.1. | **Corrected.** The fix set needs PQ response encryption (in 1609.2.1), PQ misbehaviour-report encryption (an SCMS Manager spec), and symmetric or PQ LCIs plus a non-ECDH LA–ACA key (CAMP or SCMS-Manager level; `LaRaInterfacePdu ::= NULL`). It also needs **forced re-request and re-seeding at migration**, because "request once, top up" (arXiv:1802.05323 pp.7–8) otherwise carries pre-migration exposure into post-migration certificates. |
| D5 | "Before infrastructure certificates" | §6 orders by confidentiality asset: TLS KEX, then the LCI/LA–ACA key, then response and misbehaviour-report encryption (longest lead time). It never says "before infrastructure certificates". | [REV-A] W3(b): the RA, ACA, LA and MA encryption keys are carried *in* infrastructure certificates, so the certificate format must take ML-KEM keys first. [REV-B] W2: the order mixes assets with deadlines. | **Corrected.** "Infrastructure certificates" must be split into their encryption-key fields, which come first, and their signature algorithms, which can follow. |

### 1.3 Found by the blind analysis and missing from the claim

| # | Finding | Source | Arbiter |
|---|---|---|---|
| N1 | **Misbehaviour reports.** The upload header is signed by the enrolment key. The report is ECIES-encrypted to the MA's static key and carries the reporter's own pseudonym BSM and location. One TLS Shor plus one MA-key Shor (amortised) binds pseudonyms to the enrolment identity. An RA log needs only the MA key. | [BLIND] §2.6, citing SCMS Manager MBR spec v1.0 §3.0, §4.0 (https://scmsmanager.org/wp-content/uploads/2020/01/Misbehavior-Report-and-Application-Specification-v1.0.pdf). [REV-A] W8 lists it as a flow to enumerate. | **Add as a path.** It samples pseudonyms rather than giving whole batches, but it costs O(1) per MA key. Open point: which certificate signs the report ([BLIND] §8 item 7). |
| N2 | **Backend byte-matching defeats the shuffle.** Response ciphertexts pass byte-identical ACA→RA→zip. `EncryptedIndividualPLV` ciphertexts pass byte-identical LA→RA→`RaAcaCertRequest`. A backend recorder can group shuffled requests by device after breaking TLS only. | [BLIND] §2.5, A8 | **Add.** This is the reason backend links belong in the threat model ([REV-A] W1, last bullet; [REV-B] §6). |
| N3 | **Discrete-log counts per path** (P0 to P5): O(1) per fleet (LA, MA and RA static keys), O(vehicles) (H), O(sessions) (TLS) | [BLIND] §3 table | **Adopt as a headline axis.** Both reviewers require it ([REV-A] W5; [REV-B] W4, §8.1). |
| N4 | **Downloads alone give identity and volume, not certificates.** J_ij is not derivable from the download. This flips if any RecipientInfo form exposes J_ij. | [BLIND] §2.3, §8 item 4 | **Add as a "does not suffice" result, conditional on the RecipientInfo form.** |
| N5 | **ACA privacy survives.** ACA records alone give no quantum gain (Boldyreva Thm 5.2, ideal-cipher only). ACA records plus any (A, ck) source give full linkage without touching a response ciphertext. | [BLIND] §2.9; [FACT](j) | **Add.** It sharpens what the attack must reach. |
| N6 | **Migrating pseudonym signatures to lattice butterfly** replaces information-theoretic hiding (c uniform) with a computational MLWE-type assumption. | [BLIND] §5, last paragraph | **Add as a caution.** It is a new linkability assumption to analyse, and it argues against "migrate everything". |
| N7 | **The single-key response option** (`additionalParams.encryptionKey`): one Shor opens a whole request's certificates. | [FACT](d); ASN.1 l.601 | **Add as a provisioning variant to measure.** |
| N8 | **Network address.** A TLS break binds the IP or cellular identity to the enrolment certificate. The LOP hides the address only from the RA. | [BLIND] §2.2 | Minor. Include it in the outsider path. |

---

## 2. Verdict per sub-claim

| Sub-claim | Verdict | Basis |
|---|---|---|
| **1a.** A passive recorder of request plus download, with a CRQC, links the pseudonym certificates to the enrolment certificate, and so links archived BSM tracks | **Holds** | [BLIND] §2.3 and P1; [REV-A] §5; [REV-B] §6; arXiv:1802.05323 pp.10–16 |
| **1b.** An RA archive suffices | **Holds, with corrections.** Response-ciphertext retention is UNVERIFIED ([FACT](e)). Without it, the archive must be combined with recorded downloads or backend traffic. State that RA and PCA together already link classically (shared request hash, arXiv:1802.05323 Table II p.15), so the novelty is that the RA *alone* suffices. | [FACT](e), (j); [BLIND] §2.8 |
| **1c.** BSMs alone do not suffice | **Holds** (cryptographically). Pair it with the classical-tracking caveat. | [BLIND] §2.1; [REV-B] W3 |
| **1d.** Requests alone do not suffice | **Holds for an outsider; corrected for the RA-state holder** (LCI and LA–ACA-key routes) | [BLIND] P0′; [REV-B] §6 row 2 |
| **1e.** Linkage values remain safe | **Dropped.** Replace with: "the linkage PRF chain resists Grover (about 2^64 per chain); the LCI and PLV transport is quantum-safe only if LCIs are symmetric or PQ and the LA–ACA key is not ECDH-established" | §1.2 D1; ASN.1 check |
| **1f.** "For years of certificates" | **Corrected.** Give the bound per path. The LCI route can cover a chain's whole lifetime. | §1.2 D2 |
| **1g.** Missing paths (misbehaviour reports, backend byte-matching, single-key option) | **Added** | §1.3 |
| **2a.** Migrate the provisioning path first, before infrastructure certificates and BSM signatures | **Corrected.** Recast it as a per-key inventory with deadlines (privacy shelf life, lead time, Shor count). Encryption-key fields in infrastructure certificates come first. Signatures run in parallel because of their longer lead time (Twardokus et al. NDSS 2024, doi:10.14722/ndss.2024.24267; ePrint 2022/483). Present "confidentiality before authentication" as a known corollary (Mosca; ETSI TR 103 949 pp.14–15), not a contribution. Note that ETSI TR 103 949 Annex C p.23 recommends a different order ("begin at the bottom of the hierarchy", [PRIOR]). | [REV-A] W3; [REV-B] W2 |
| **2b.** Hybrid TLS alone stops an outside recorder | **Corrected and narrowed.** It stops only an access-network recorder, with end-to-end TLS to the RA covering request, download and misbehaviour upload. A backend recorder also needs PQ component links and non-ECDH LCI and LA–ACA keys. | §1.2 D3 |
| **2c.** The RA-archive adversary needs PQ response encryption (a 1609.2.1 change) | **Corrected: necessary, not sufficient.** Add PQ misbehaviour-report encryption, PQ or symmetric LCIs, a non-ECDH LA–ACA key, and forced re-request plus re-seeding. Only the response part is a 1609.2.1 change; check 1609.2.1-2026, now the active edition ([FACT] intro), and the IEEE 1609 PQC extension work (5GAA/GSMA PQ.08 PDF p.33). The response fix is not a drop-in ML-KEM swap: it needs PQ butterfly (ePrint 2024/946; 2018/1247) or about 3,120 independent ML-KEM-768 keys (about 3.7 MB upstream for 3 years at 20 per week; FIPS 203, 1,184 B per key) with anonymous ciphertexts (ePrint 2021/708, 2022/1696). | [REV-A] W6; [REV-B] Q6 |
| **3a.** ETSI TR 103 949 concludes HNDL is not a main concern | **Corrected.** The TR never uses "harvest". It sets confidentiality "Protection duration: N/A" and "Disclosure impact: None per se", reasoning that PII is protected "by use of pseudonymous keying … rather than by encryption" (Table 3, p.16). It frames the CRQC as masquerade (§4.4, p.14). It also says "Privacy of the user is to be maintained across the lifetime of the ITS-S" (Table 2, p.16). Quote both. It models the EU PKI, not the US butterfly. | [FACT](g); [PRIOR]; [REV-B] S3 |
| **3b.** The 5GAA/GSMA October 2026 report says HNDL is not a main concern | **Holds**, with exact wording. GSMA PQ.08, "Post-Quantum Cryptography for Automotive – Impact Assessment", Oct 2026, §3.3, printed p.07/32 (PDF p.10): "'harvest-and-decrypt' attack … is not the **main** concern for V2X" (https://5gaa.org/post-quantum-cryptography-for-automotive-impact-assessment/). Both reviewers failed to find it, but [FACT](h) and [PRIOR] each located and quoted it independently. Add the nuance that the same report notes HNDL obligations under data-protection law (PDF p.17). A June 2026 internal 5GAA deck says "'Harvest now, decrypt later' already relevant" ([REV-A] W4), but it is not an agreed position. | [FACT](h); [PRIOR]; [REV-A] W4 |
| **3c.** (new) Chen et al. 2026 rates one-week pseudonym certificates "Pass" for quantum resistance | **Add as the primary foil.** arXiv:2606.14008, Table XII, p.16. [PRIOR] places the table in §IV.E.1 and [REV-B] in §VI; confirm the section number. Lonc et al. 2025 p.8 (doi:10.1109/NTMS65597.2025.11076918) makes the same forgery-view argument. | [PRIOR]; [REV-B] S2 |
| **4.** Quantify linkable pseudonym-years and cost (bytes, airtime) with the simulator | **Corrected.** Use a closed-form model first, with the simulator for coverage, the tracking baseline and fleet mix. Report the Shor-count axis and the marginal linkage over classical tracking. Replace airtime with backhaul bytes and compute (provisioning runs over cellular or IP, not sidelink; arXiv:1802.05323 p.7). | [REV-A] W5–W7; [REV-B] W3, W5, W6 |
| **Novelty:** "first to identify this mechanism" | **Narrowed.** Claim the first SCMS-specific statement, path enumeration and measurement of retroactive pseudonym linkage, including the linkage-plumbing (LCI) route. Do not claim the general principle (everlasting privacy, Moran & Naor, doi:10.1007/11818175_22; PQ onion routing, ePrint 2015/008; QADR, arXiv:2511.15272), PQ butterfly (2018/1247, 2024/946), PQ provisioning encryption (Chen arXiv:2606.14008 §III.B.2; Lonc NTMS 2025 §III), or the ordering principle. | [PRIOR] verdict; [REV-A] W2; [REV-B] §5 |

**Overall: the core holds; the claim as written does not.** All five inputs agree on the central mechanism: a CRQC lets the RA's records alone, or a recorder of both provisioning legs, act retroactively as the RA–ACA collusion that butterfly privacy assumes away. None refutes it. Two statements are technically wrong as written: "linkage values remain safe" and "PQ response encryption suffices against an RA archive". Correcting them widens the threat, and the reviewers agree this makes the paper stronger.

---

## 3. Required changes

**Threat model and analysis**
1. **Enumerate every flow that carries c, certificates, seeds, PLVs or (A, ck, H, ek)**, as the [BLIND] §3 table does. For each, give the recorded vantage point, the discrete-log count (per fleet, per vehicle, per session) and the exposure bound. Include misbehaviour reports, backend links (with byte-matching) and enrolment roll-over ([REV-A] W8).
2. **Treat unverified deployment facts as explicit model parameters** and report results under each setting:
   - LCI encryption: PK vs. symmetric.
   - LA–ACA key establishment: ECDH vs. pre-shared or PQ.
   - RA retention of zips, PLV ciphertexts and misbehaviour-report headers.
   - TLS termination: end-to-end vs. front-end vs. LOP.
   - Response RecipientInfo form.
   - Response mode: original, unified or CUBK butterfly, single key, ACPC.
3. **State the step from device to person.** The enrolment certificate is a device credential; the OEM or operator maps it to VIN and owner ([REV-B] minor; [BLIND] §8 item 8).
4. **Dismiss Grover explicitly** with numbers: 128-bit seeds and AES-128 expansion give about 2^64 per chain; 256-bit seeds are a cheap upgrade ([BLIND] §2.1, §5).
5. **Position against Boldyreva et al.** as "a CRQC removes both the private-channel assumption (p.2) and the DH assumption behind corrupted-RA privacy". [FACT] cites HDH (Thm 5.1/6.1); [PRIOR] cites ODH (Thm 7.2). Reconcile which assumption each theorem uses before quoting.

**Remedy and migration**

6. **Replace the asset-class order with a per-key inventory.** For each key give: holder, algorithm, lifetime, privacy shelf life (vehicle lifetime plus retention), migration lead time, Shor count and deadline. Keys to cover:
   - EE↔RA TLS KEX;
   - RA request key;
   - per-certificate response key;
   - MA report key;
   - LA LCI key;
   - LA–ACA key establishment;
   - component TLS;
   - infrastructure signature keys;
   - pseudonym and BSM signature keys.
7. **Make re-request and re-seeding a required migration step**, and measure the exposure tail without it.
8. **Specify the PQ response design** and its unlinkability argument as an extension of the Boldyreva model (ePrint 2024/1674). Compare PQ-BKE (2024/946), independent ML-KEM keys with anonymous ciphertexts, and hybrids. Address PCA-side unlinkability for the naive swap ([REV-B] Q6). Note N6 (a lattice signing half loses statistical hiding).
9. **Check 1609.2.1-2026** for PQ options before claiming a standards change is needed.

**Evaluation**

10. **Closed-form exposure model first**, validated by the simulator. Sensitivity over CRQC date, retention, preload horizon and top-up cadence ([REV-A] W7; [REV-B] W6).
11. **Marginal linkage over a classical tracker** on the same simulated BSM capture, with explicit sniffer coverage. Report three separate quantities: cross-gap and cross-day linking, binding to identity, and certainty (Wiedersheim et al., doi:10.1109/WONS.2010.5437115; Golle & Partridge, doi:10.1007/978-3-642-01516-8_26).
12. **Cost metrics:**
    - provisioning bytes on the cellular or IP bearer;
    - RA, ACA and LA compute and storage at fleet scale;
    - OBU and HSM feasibility;
    - re-keying and key-ceremony events.

    Drop airtime for provisioning, or confine it to the signature track.
13. **Fault-injection checks** proving the metric moves. For example: toggle RA retention of PLV ciphertexts, LCI PK vs. symmetric, and TLS end-to-end vs. front-end. Each toggle must change the exposure ([REV-B] §8.6).

**Citations**

14. Quote ETSI TR 103 949 Tables 2 and 3 (p.16) and §4.4 (p.14) verbatim. Quote 5GAA/GSMA PQ.08 §3.3 (p.07/32) verbatim with "main". Cite NIST IR 8547 as the initial public draft. Use "cocoon encryption key", not "butterfly encryption key" ([FACT](d)). Use "1–3 years", not "156 periods" ([FACT](c)).
15. **Optional scope extension:** a short assessment of the EU CCMS (ETSI TS 102 941) authorization-ticket request path. It is unverified, possibly cheaper (two authority keys), and Lonc 2025 §III is the only source read ([REV-B] W8; [PRIOR]).

---

## 4. REVISED CLAIM

In the US Security Credential Management System, built on IEEE 1609.2.1 butterfly key expansion as profiled for the US deployment, a vehicle obtains pseudonym certificates as follows.

**The provisioning flow**
- The vehicle sends one request, signed with its enrolment certificate and encrypted (ECIES) to the Registration Authority's certificate key, normally over TLS. It carries caterpillar keys and expansion values from which the RA derives per-certificate cocoon keys for every future weekly period, typically 1–3 years ahead, with periodic top-ups.
- The Authorization (pseudonym) CA adds a fresh secret random offset to each cocoon key, which hides the issued certificate information-theoretically. It returns the certificate and offset through the RA, encrypted (ECIES) to a per-certificate cocoon encryption key.
- Linkage values are computed from AES- and SHA-based chains seeded at two Linkage Authorities. Pre-linkage values reach the CA under an LA–CA symmetric key whose establishment the standard leaves out of scope. The RA keeps per-device linkage-chain identifiers, which the SCMS design paper describes as LA seeds encrypted to the LA's own public key.
- Misbehaviour reports are uploaded under an enrolment-signed header, encrypted to the Misbehaviour Authority's static key.

**(1) Threat.** A passive adversary who stores ciphertexts today and later has a cryptographically relevant quantum computer can link a vehicle's pseudonym certificates to one another and to its enrolment certificate, and thereby link archived BSMs signed under them. It can do so only by breaking the confidentiality of a stored flow that carried the offsets, the certificates, the linkage seeds, the pre-linkage values or the caterpillar keys. Recorded BSMs, CRLs, CA records, requests alone or downloads alone give no quantum gain. The linkage chains themselves need about 2^64 Grover work per chain. We enumerate the sufficient paths and their discrete-logarithm cost:
- (a) a recorder on the vehicle's access network that captures provisioning and download sessions: one discrete log per TLS session plus one per vehicle, exposing all certificates the request covers together with the network address;
- (b) a recorder on RA–CA and RA–LA backend links, where byte-identical ciphertexts across hops undo the RA's shuffle;
- (c) an adversary holding an RA archive: about one discrete log per vehicle if encrypted responses are retained. If linkage-chain identifiers or the LA–CA key are public-key protected, the cost is a constant number for the entire fleet, and linkage covers each chain's whole lifetime, including certificates issued after a later fix unless chains are re-seeded;
- (d) misbehaviour-report uploads, which bind sampled pseudonyms and locations to the enrolment certificate for one discrete log per MA key.

The RA and CA together already link classically. The new point is that, once a quantum computer exists, the RA's records alone suffice. The existing proof of privacy against a corrupted RA assumes a private vehicle-to-RA channel and a Diffie–Hellman-type assumption, both of which such a computer removes.

**(2) Consequence for migration.** Pseudonym unlinkability is a confidentiality asset whose shelf life is the vehicle's lifetime plus data retention. The keys that carry it, and so need migration deadlines set by that shelf life, are:
- the TLS key exchanges (vehicle to RA, and between components);
- the RA request-encryption key;
- the per-certificate response encryption;
- the MA report key;
- the linkage-chain-identifier encryption;
- the LA–CA key establishment.

Signature keys do not affect passive linkability.

The fix needed depends on the adversary:
- **Access-network recorder:** end-to-end hybrid post-quantum TLS to the RA suffices.
- **Backend recorder:** additionally needs post-quantum component links, and linkage-chain identifiers and LA–CA keys that do not rely on elliptic-curve key agreement.
- **RA archive:** additionally needs post-quantum response encryption (a post-quantum butterfly expansion, or independent per-certificate KEM keys with anonymous ciphertexts) and post-quantum misbehaviour-report encryption.

Because devices request once and are topped up, migration must also force new caterpillar keys and re-seed linkage chains. Otherwise pre-migration exposure persists into post-migration certificates. Signature migration proceeds in parallel because of its longer lead time. Moving pseudonym keys to lattice-based expansion would replace information-theoretic hiding with a computational assumption.

**(3) Position against existing assessments.** These assessments treat the quantum risk to pseudonymity as forgery only:
- ETSI TR 103 949 assigns C-ITS confidentiality a protection duration of "N/A" and disclosure impact "none per se", on the grounds that privacy comes from pseudonymous keying rather than encryption, while also requiring privacy over the station's lifetime.
- The October 2026 5GAA/GSMA PQC impact assessment states that harvest-and-decrypt "is not the main concern for V2X".
- Recent pseudonym-scheme work rates one-week pseudonym certificates as quantum-resistant.

We argue that this view does not hold for unlinkability, because pseudonymous keying is itself delivered by encryption.

**(4) Measurement.** Using a closed-form exposure model validated against a city-scale V2X simulator with a full SCMS, we will quantify linkable pseudonym-years:
- per attack path and per discrete-log budget;
- under alternative migration schedules and provisioning variants (original, unified and compact-unified butterfly; single response key; ACPC; post-quantum butterfly; independent KEM keys);
- as the increment over a classical trajectory tracker on the same BSM captures;
- together with the cost of each fix in provisioning bytes on the cellular bearer, authority compute and storage, and re-keying events.

**What we do not claim.** We do not claim the general principle of retroactive deanonymisation, post-quantum butterfly constructions, post-quantum provisioning encryption as such, or the confidentiality-before-authentication ordering. We claim the SCMS-specific threat, its path enumeration and its measurement.

### Revised evaluation plan

| Step | What | Output | Needs the simulator? |
|---|---|---|---|
| E1 | Key and flow inventory: every key, its holder, algorithm, lifetime and the flows it protects, from 1609.2.1 ASN.1, Brecht arXiv:1802.05323, and the SCMS Manager specs | Table driving every later step | No |
| E2 | Closed-form exposure model per path (P0–P5 in [BLIND] §3): linkable certificates as a function of migration date T, CRQC date Q, retention R, preload horizon and top-up cadence, and the deployment parameters in §3 item 2 | Formulae plus a discrete-log-budget axis (O(1) per fleet, O(vehicles), O(sessions)) | No |
| E3 | Engine: harvest ledger recording, per flow, what a recorder or archive holder captured; fleet with mixed provisioning variants on the real calendar; hybrid TLS; LCI and LA–ACA modes as switches | Simulated exposure | Yes |
| E4 | Validate E3 against E2 on controlled scenarios; fault-inject each vulnerability toggle and show the metric moves | Agreement table; check results that fail when the fault is injected | Yes |
| E5 | City-scale BSM capture under explicit sniffer coverage; convert linkable certificates into linked trajectories | Linked trajectory-days | Yes |
| E6 | Classical tracker baseline (kinematic or MHT, after Wiedersheim et al.) on the same captures; report the increment in cross-gap linking, identity binding and certainty | Marginal linkage | Yes |
| E7 | Migration schedules: per-key deadline orders vs. the ETSI "bottom of the hierarchy" order vs. signatures-first; with and without forced re-request and re-seeding | Exposure tail per schedule | Mostly E2, checked by E3 |
| E8 | Fix costs: request and response bytes (ML-KEM-768 sizes, ePrint 2024/946 parameters), RA, ACA and LA compute and storage at fleet scale, OBU feasibility, re-keying events; no provisioning airtime | Cost table per variant | Partly (fleet scale) |
| E9 | Sensitivity over CRQC date, retention and capture coverage; report the conditions under which the conclusion flips | Robustness figure | Both |

---

## 5. Open risks

1. **Deployment facts decide which paths exist.** Unverified:
   - LCI encryption (PK vs. symmetric);
   - LA–ACA key establishment;
   - RA retention of response zips, PLV ciphertexts and misbehaviour-report headers;
   - TLS termination point and OEM proxies;
   - response RecipientInfo form;
   - which certificate signs misbehaviour reports.

   Sources: [FACT](a), (e), (f); [BLIND] §8; [REV-B] Q1–Q3. The CAMP EE spec was unreachable (ECONNREFUSED, [FACT]). If LCIs are symmetric and nothing is retained, the O(1)-per-fleet RA path shrinks to O(vehicles) and depends on retention. The thesis then still holds, but its headline is weaker.
2. **"Obvious" critique.** HNDL applied to SCMS may be judged an application of a known principle ([REV-A] W2). Mitigations: the LCI and plumbing path, which appears in no prior work ([REV-A] §4; [REV-B] §5); the backend byte-matching path; the measured increment over tracking; and the explicit foils (Chen Table XII, ETSI Table 3, 5GAA §3.3).
3. **Marginal harm may be small.** If a classical tracker on city-scale captures already links most of what a CRQC adds, the headline weakens to identity binding and cross-gap linking ([REV-B] W3). E6 must be run before any claim of magnitude.
4. **Scoop and standards timing:**
   - the Chen group (arXiv:2606.14008), one paragraph away from the threat;
   - the Mamun/Abrar/Chowdhury group (arXiv:2601.01068, 2608.05087);
   - the PQ-BKE authors;
   - IEEE 1609 PQC extensions, the ETSI TVRA quantum extension and the C2C-CC migration white paper (5GAA/GSMA PQ.08, PDF p.33, [PRIOR]).
5. **Unread or unfound sources:**
   - Fikry et al. 2026 (IEEE ASSIC) was not found by [PRIOR] or [REV-B], so the citation may be wrong.
   - Chouhan 2026 (doi:10.5281/zenodo.19488046), Salman 2022 and Chen arXiv:2501.07028 were read as abstracts only.
   - ETSI TS 102 941 was not fetched.
   - 1609.2.1-2026 normative text is paywalled.
   - [BLIND] read ePrint 2018/1247 and 2024/946 only as abstracts; [PRIOR] read them through Wayback.
6. **Proof gaps.** Boldyreva's proofs are classical ROM and ideal-cipher; there is no QROM lift ([BLIND] §8 item 9). Inputs disagree on whether the corrupted-RA result rests on HDH (Thm 6.1) or ODH (Thm 7.2), so check before citing. A PQ response design needs its own unlinkability proof (ePrint 2024/1674 model).
7. **CUBK and unified variants.** `AcaEeCertResponseCubkSpdu` has no ACA signature on the ciphertext. Whether the offset is still random there is unchecked (1609.2.1 §9.3 is not public), so the "requests alone do not suffice" result may not transfer ([FACT] further-finding 2; [BLIND] §8 item 10).
8. **CRQC realism.** Path rankings depend on discrete-log throughput (Gidney arXiv:2505.15917 gives RSA-scale estimates; ECC-256 figures differ). Report results per discrete-log budget, not as a single date.
9. **Source independence.** [BLIND] built on the project's own D02/D03. Where D02/D03 summarise paywalled CAMP or 1609.2.1 text, [BLIND] and the claim may share an error. Brecht et al. 2018 is the common upstream source for every input's LCI and PLV reading, and it predates the deployed profile.
