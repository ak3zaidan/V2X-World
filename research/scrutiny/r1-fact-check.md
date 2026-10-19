# R1 — Primary-source fact check of the "harvest now, link later" premises

Date: 2026-10-18. Scope: premises (a)–(j) of the claim under review. I opened only primary or near-primary sources. Status values: CONFIRMED / PARTLY CONFIRMED / REFUTED / UNVERIFIED.

## Sources actually read

| Key | Source | Access |
|---|---|---|
| [ASN] | IEEE 1609.2.1 ASN.1 modules (IEEE-maintained mirror, branch `ieee`), `Ieee1609Dot2Dot1Protocol.asn`, `Ieee1609Dot2Dot1EeRaInterface.asn`, `Ieee1609Dot2Dot1AcaRaInterface.asn`, https://forge.etsi.org/rep/ITS/asn1/ieee1609.2.1 | Full text. The normative prose of IEEE 1609.2.1 is paywalled and was **not** read. |
| [BRE] | Brecht, Therriault, Weimerskirch, Whyte, Kumar, Hehn, Goudy, "A Security Credential Management System for V2X Communications", arXiv:1802.05323v1 (IEEE T-ITS 2018), the CAMP/USDOT SCMS design paper | Full PDF |
| [BKS] | Boldyreva, Kumar, Sun, "Provable Security Analysis of Butterfly Key Mechanism Protocol in IEEE 1609.2.1 Standard", PoPETs 2024(4):565–582, https://petsymposium.org/popets/2024/popets-2024-0130.pdf ; IACR ePrint 2024/1674 | Full PDF |
| [TR949] | ETSI TR 103 949 V1.1.1 (2023-05), "Quantum-Safe Cryptography (QSC) Migration; ITS and C-ITS migration study", https://www.etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf | Full PDF |
| [5GAA] | 5GAA & GSMA, "Post-Quantum Cryptography for Automotive – Impact Assessment", October 2026. Landing page https://5gaa.org/post-quantum-cryptography-for-automotive-impact-assessment/ (dated 6 Oct 2026). PDF via https://5gaa.org/sdc_download/9673/ | Full PDF (35 pp.) |
| [IR8547] | NIST IR 8547 ipd, "Transition to Post-Quantum Cryptography Standards", https://nvlpubs.nist.gov/nistpubs/ir/2024/NIST.IR.8547.ipd.pdf ; status page https://csrc.nist.gov/pubs/ir/8547/ipd | Full PDF + status page |
| [ARC] | ARC-IT, Location Obscurer Proxy physical object, https://www.arc-it.net/html/physobjects/physobj190.html ; flow https://www.arc-it.org/html/triples/s159d190f1199.html | Web |
| [CPS] | AUTOCRYPT V2X Root CA CPS (2026-02-06), https://autocrypt.io/wp-content/uploads/2026/03/AUTOCRYPT-V2XEE-CPS-260206_2.pdf | Full PDF |

**Not reachable:** the CAMP SCMS PoC "EE Requirements and Specifications" (wiki.campllc.org refused the connection, ECONNREFUSED), the SCMS Manager EE requirements and certificate policy (no public copy found), and the IEEE 1609.2.1-2022 and 1609.2.1-2026 normative text (paywalled). IEEE lists **1609.2.1-2026** as the active edition, superseding 2022 (catalogue listings, e.g. https://store.sfs.fi/en/ieee-160921-2026). The thesis should cite the 2026 edition and re-check every clause against it.

---

## (a) Where TLS terminates in the US provisioning path: UNVERIFIED (indirect evidence points to EE-to-RA end to end, through a pass-through LOP)

- No primary source I could reach states where TLS terminates. The CAMP EE requirements and the SCMS Manager profile, which would settle this, were unreachable.
- Indirect evidence:
  - [BRE] p.15, §V-E Step 1: "The device then sends the request to the RA via LOP. The LOP functions as a pass-through device for requests. The LOP obscures the device's identifiers (e.g., IP address) by replacing these identifiers with its own in transit … very similar to the masquerading feature implemented in many Internet routers." NAT-style masquerading implies the LOP does not terminate TLS, but the paper does not say so explicitly.
  - [BRE] p.5, §II-B: "All online components communicate with each other using a protected and reliable communication channel, utilizing protocols such as those from the Transport Layer Security (TLS) suite."
  - [BKS] p.2, §1.2 *assumes* "The communication between EEs and the RA is private and authenticated" and "the EE-RA communication … talk over a secure channel" (p.7, §4).
- **Consequence for the claim.** Whatever TLS does, [ASN] shows the request is *also* application-layer encrypted to the RA (see (d)). An outside recorder therefore has two quantum-breakable layers to strip: TLS ECDHE, then ECIES to the RA's certificate key. Hybrid TLS alone therefore does **not** stop an outside recorder from obtaining the request, unless the RA's request-encryption key is also post-quantum. It does stop a tap that sits behind the TLS endpoint but before the RA, if such a point exists (for example a load balancer that terminates TLS). The claim's statement "hybrid TLS alone stops an outside recorder" needs this qualification, or evidence of where TLS terminates.

## (b) TLS versions and key-exchange groups the US profile allows: PARTLY CONFIRMED (versions only, from a secondary source); groups UNVERIFIED

- [BKS] p.6, §3.3, written with a Qualcomm co-author: "The IEEE 1609.2.1 standard specifies several mechanisms for secure channels including TLS1.2, TLS1.3 and ISO/TS21177."
- A catalogue summary says the 1609.2.1-2022 reference list cites RFC 5246 (TLS 1.2). This is unverified against the text.
- [5GAA] §on ISO 21177 (PDF pp.~26–29) treats the ISO 21177 TLS model as a V2X unicast option. It does not cover SCMS provisioning.
- I found **no** primary source on permitted key-exchange groups (secp256r1, x25519 and so on) in the SCMS Manager or CAMP profile. Treat any statement about specific groups as UNVERIFIED.

## (c) "Request once, top up forever" and the ~3-year / 156-period preload: CONFIRMED for the SCMS design; 156 periods UNVERIFIED

- [BRE] p.7, §II-C: "devices do not need to explicitly request new certificates but the SCMS will automatically issue new certificates throughout the life-time of the device, until the device stops picking-up certificates for an extended period of time. The RA will provide the certificates in batches worth one week … The RA will provide information when to expect new certificate batches (e.g., once per month)".
- [BRE] p.8, §III, "Continuous Generation": "The SCMS will continuously generate certificates once an initial request has been made … facilitates top-up strategies."
- [BRE] p.7, §II-C, parameters: "Certificate validity time period: 1 week", "batch size: minimum 20", "Overall covered time-span: 1−3 years". Also: "in full deployment a device may need up to 3 years' worth of certificates".
- [ASN] `EeRaInterface.asn`, `RaEeCertAck` and `RaEeCertInfo` carry `nextDlTime` ("the time after which the EE should connect to the RA to download the certificates") and `currentI`/`firstI`. In `EeRaCertRequest`, "validityPeriod contains the requested validity period of the **first** batch". This is consistent with a single request followed by repeated downloads.
- **Not confirmed:** "156 periods". That is 3 × 52 weeks, an arithmetic inference from [BRE]'s "1–3 years", not a number I found in a source. [BRE] gives a *range* of 1–3 years, set by the SCMS Manager. What production SCMS providers actually use is UNVERIFIED.

## (d) Response encrypted per certificate to a butterfly-derived key with ECIES, so the RA cannot read it: CONFIRMED, with terminology and option caveats

- [ASN] `Ieee1609Dot2Dot1Protocol.asn`, comment on `AcaEeCertResponsePrivateSpdu`: the ACA uses this form when "it is necessary to protect the EE's privacy from the RA, for example when the certificate being returned is a pseudonym certificate." The response is "a signed SPDU containing an encrypted SPDU", encrypted with:
  - the **cocoon encryption key** derived from `additionalParams.original` ("original" butterfly);
  - or the cocoon key derived from `tbsCert.verifyKeyIndicator` ("unified");
  - or, if `additionalParams.encryptionKey` is present, **a single response encryption key**.
- `AcaEeCertResponseCubkSpdu`, used for "compact unified", is encrypted to the cocoon public key with no outer signature.
- [BRE] p.13, §V-B.2: "To prevent the RA from seeing multiple certificates belonging to the same device, the PCA encrypts each singular certificate to the device … to encrypt each certificate with a different key."
- [BRE] p.12: "Encrypting the certificates to the device prevents the RA from relating certificates with a device."
- The scheme is ECIES with AES-CCM ([BKS] p.6, §3.3: "IEEE BKM uses AES as EXP, ECDSA as DS1, DS2, and ECIES as AE with the CCM mode").
- **Corrections to the claim's wording:**
  1. The key is the per-certificate **cocoon** encryption key, not a "butterfly encryption key".
  2. 1609.2.1 also allows a **single, non-expanded** response encryption key (`additionalParams.encryptionKey`). Under that option one Shor break opens every certificate in the request, which makes the threat worse.
  3. The EE→RA request is "signed then encrypted": signed with the enrolment certificate and encrypted "using the encryptionKey from the RA's certificate" ([ASN], `EeRa1609Dot2AuthenticatedCertRequestSpdu`). This confirms the claim's description of the request.
  4. 1609.2.1 also defines **ACPC** (§9.5, referenced by `acpcTreeId` in `RaEeCertInfo`), a different provisioning family. Any comparison of provisioning designs must include it.

## (e) What the RA stores long-term: PARTLY CONFIRMED; archiving of encrypted batches UNVERIFIED

- [BRE] p.15, Table II "Information stored by SCMS components", RA row: "Enrollment certificate and its validity period, hash value of RA-to-PCA pseudonym certificate request". PCA row: "…linkage value, certificate, and hash of RA-to-PCA pseudonym certificate request".
- For continuous generation ((c)), the RA must keep the caterpillar public keys and expansion values for the device's lifetime. This is an inference from [BRE] p.7 and p.15 ("during pre-generation of additional pseudonym certificates, the RA requests pre-linkage values … associated with that device").
- [ASN] `RaEeCertInfo` contains `requestHash` ("the hash of the corresponding EeRaCertRequestSpdu"). The RA therefore maps every batch file to the enrolment-signed request.
- Whether the RA keeps the **encrypted ACA responses** after the device downloads them, and for how long, is UNVERIFIED. No SCMS Manager certificate policy or RA CPS was found.
- The one CPS I read covers the Root CA only: AUTOCRYPT [CPS] §5.5.2 says retain "all records for at least five (5) years after the corresponding certificate has expired" and §5.4.3 says certificate-lifecycle logs are kept ≥5 years. That is Root CA scope, not RA.
- **Implication.** The RA-archive premise is plausible, since the RA must hold the caterpillar keys and the request-to-enrolment mapping. Whether it also holds the ciphertexts needs a source (an RA CPS or the SCMS Manager CP). Otherwise the adversary must combine an RA archive with a separately recorded download.
- Note also that [BRE] Table II gives the RA and PCA a shared request hash. **RA + PCA collusion already links classically**, by design for revocation. The quantum claim's novelty is that the RA's records *alone* suffice. The thesis should say so explicitly.

## (f) Whether downloads pass through a LOP in practice: UNVERIFIED

- Design intent: [BRE] p.5, Fig. 1 marks which connections "have to go through the Location Obscurer Proxy". [BRE] p.12: "The LOP obscures the physical location of an end-entity device to hide it from the RA and the MA."
- [ARC] describes the LOP only as a CCMS component; "This physical object does not have any functionality defined in ARC-IT, as it exists outside the system functional boundary."
- I found no evidence about deployed US SCMS providers operating a LOP, or about whether *downloads* (as opposed to requests) traverse it.
- The LOP does not change the cryptographic argument either way: it hides IP addresses, not payloads.

## (g) ETSI TR 103 949 on harvest-now and confidentiality duration: PARTLY CONFIRMED; the claim's characterisation is too strong

- The TR **never uses** "harvest" or "store-now-decrypt-later". A full-text search found no occurrence.
- Its relevant statements are in §5, Tables 2–3, p.16:
  - Table 2 "Magnitude": "As CAMs and DENMs are sent in clear there is no risk of information disclosure. The risk is that the trust network is broken."
  - Table 3: "Protection duration — How long does confidentiality need to be maintained for each data class? **N/A**". Also "Exposure — Is encrypted data normally exposed to potential attackers? **N/A**". "Disclosure impact: None per se."
  - **But** Table 2 "Duration" says: "**Privacy of the user is to be maintained across the lifetime of the ITS-S** that is associated to the user." Table 4 gives enrolment-key lifetime "(say) 25 years".
  - §4.4, p.14 frames the CRQC threat as masquerade/impersonation ("collapse of the required trust model").
- **Verdict.** The TR treats confidentiality-based HNDL as not applicable to C-ITS. The thesis may say it "does not identify a harvest-now threat to C-ITS". It must not say the TR "concludes HNDL is not a concern", because the TR never states that.
- The TR also *requires* lifetime privacy, which supports the thesis. Quote both statements.
- Caveat: the TR models the EU C-ITS PKI (EA/AA, TS 102 941), not the US SCMS butterfly path.

## (h) The 5GAA/GSMA PQC report: CONFIRMED, exact quote located

- Title: "Post-Quantum Cryptography for Automotive – Impact Assessment". Subtitle: "An assessment of the potential security impact of the quantum threat on connected vehicles…across PC5 and Uu communications". Cover dated "October 2026" and "Copyright @5GAA and @GSMA". The landing page is dated 6 Oct 2026.
- Exact statement, §3.3 "The quantum threat", printed page 07/32 (PDF p.10): "many V2X use cases, such as safety and mobility use cases, do not use confidential information and make use of information with a very short active or relevant lifetime (i.e., short-lived data), so this 'harvest-and-decrypt' attack, while important for internet commerce and protecting long-lived data such as corporate IP, is not the main concern for V2X."
- The same section names "financial interactions or upload of personal information for insurance purposes" as possible targets. It does not mention SCMS certificate provisioning or pseudonym linkability.
- PDF p.17 notes data-protection frameworks may require protection "against risks associated with 'harvest now, decrypt later' attacks".
- PDF p.16 lists "Privacy and personal data exposure" as a risk "where personal/vehicle data is exchanged externally".
- The report says "1609 Working Group is starting the process to develop extensions to IEEE 1609.2 to support PQC algorithms" (PDF ~p.30). This is a scoop and timing risk for the thesis.
- Note on the claim's wording: the phrase is "not the **main** concern", not "not a concern".

## (i) NIST IR 8547 status and ECDSA/ECDH timelines: CONFIRMED (still a draft)

- Status: the CSRC page fetched 2026-10-18 shows "NIST IR 8547 (Initial Public Draft)", published 12 Nov 2024, comments closed 10 Jan 2025. The document history lists only the ipd; I found no final version. Cite it as a draft.
- [IR8547] §4.1 Table 2 (PDF p.20), ECDSA [FIPS186]:
  - 112-bit security: "Deprecated after 2030, Disallowed after 2035".
  - ≥128-bit security: "Disallowed after 2035". There is no deprecation step, so P-256 stays allowed until 2035.
  - EdDSA and RSA follow the same pattern.
- [IR8547] §4.2 Table 4 (PDF p.21), Elliptic Curve DH and MQC [SP80056A]: same pattern, 112-bit "Deprecated after 2030 / Disallowed after 2035", ≥128-bit "Disallowed after 2035".
- [IR8547] §3 (PDF ~p.17) cites NSM-10's 2035 target.

## (j) Does Boldyreva et al. (PoPETs 2024) prove butterfly unlinkability under computational DH-type assumptions? PARTLY CONFIRMED; it depends on the adversary

- **Corrupted RA** (the case that matters for the RA-archive adversary): Theorem 5.1, p.10. Privacy reduces to ind-cpa-wrka security of the encryption scheme AE, uf-cma of DS2 and target-robustness of AE, with EXP modelled as an ideal cipher and H as a random oracle.
- For the ECIES instantiation, Theorem 6.1, p.11, "To prove ind-cpa-wrka security of ECIES we will need to rely on Hash Diffie-Hellman (HDH) assumption". **Yes: computational, DH-type (HDH), in the ROM/ideal-cipher model.** A CRQC breaks this assumption, which is precisely the claim's mechanism.
- **Corrupted ACA**: Theorem 5.2, p.10, gives Adv ≤ negl(λ) assuming only that EXP (AES) is an ideal cipher. That is a symmetric-only assumption with no DH, so unlinkability *against the ACA* is not broken by Shor.
- Both: p.2 says "If both ACA and RA are corrupted, then no privacy can be achieved."
- [BKS] assumes a private EE–RA channel (p.2). The privacy proof says nothing about an adversary who later breaks that channel. This supports claim (1)'s "outside recorder" path as outside the proven model, not contradicted by it.

---

## Further findings that affect the claim

1. **Linkage values "remain safe": CONFIRMED only conditionally.** In 1609.2.1 the pre-linkage values travel LA → RA → ACA as `EncryptedIndividualPLV` = `Ieee1609Dot2Data-SymmEncryptedSingleRecipient` "using the shared secret key". [ASN] `AcaRaInterface.asn` adds: "How the ACA obtains the shared symmetric key and how the RA associates the encPlv1 and encPlv2 with the correct certificate request are outside the scope of this document." If that LA–ACA key was itself set up with ECDH or ECIES, an RA archive of encPlv values plus the RA's mapping of encPlv sets to the device would yield lv = plv1 ⊕ plv2. That linkage value is printed in every pseudonym certificate, so this second path links without the ACA response at all. The thesis must either bound this path or state that the LA–ACA key is pre-shared or post-quantum. [BRE] p.13 describes the plvs only as "encrypted for the PCA", without naming a scheme.
2. **"Recording only the requests does not suffice": CONFIRMED in principle.** The butterfly public key is cocoon key + ACA-random offset ([BRE] p.10, Fig. 2: "Inform OBE on random contribution c1 to cn. This data is sent through the RA, but encrypted to the device"; [ASN] `AcaEeCertResponsePrivateSpdu`). Without the response ciphertext, the request does not determine the certificate. One exception is unverified: the **compact unified** variant, `AcaEeCertResponseCubkSpdu`, has no ACA signature on the ciphertext, and I could not check whether the offset is still random there, since §9.3 is not public.
3. **"Hybrid TLS alone stops an outside recorder" is incomplete.** See (a): the request and the response are also application-layer ECIES. An outside recorder who strips TLS still needs Shor against the ECIES layers, and *with* a CRQC that is feasible. Hybrid TLS stops the outside recorder only because they never see the ECIES ciphertexts. That holds only if TLS runs end to end to the RA, which is UNVERIFIED (see (a)). If TLS ends at a front-end, the leg behind it is an additional exposure.
4. **Edition risk.** 1609.2.1-2026 is now the active edition. The 5GAA report says IEEE 1609 has started PQC extensions. Check whether the 2026 edition changed response encryption or added PQ options before claiming "a 1609.2.1 change is needed".

## Summary table

| Item | Status | Key citation |
|---|---|---|
| (a) TLS termination point | UNVERIFIED (indirect: LOP is a NAT-style pass-through) | [BRE] p.15 §V-E Step 1 |
| (b) TLS versions / groups | Versions PARTLY CONFIRMED (TLS 1.2, 1.3, ISO 21177, via [BKS]); groups UNVERIFIED | [BKS] p.6 §3.3 |
| (c) Request once, top up; ~3 years | CONFIRMED (1–3 years, weekly batches, ≥20/week); "156" UNVERIFIED | [BRE] p.7 §II-C, p.8 §III; [ASN] `nextDlTime` |
| (d) Per-certificate ECIES response, RA blind | CONFIRMED (cocoon key, not butterfly key; single-key option exists) | [ASN] `AcaEeCertResponsePrivateSpdu`; [BRE] p.13; [BKS] p.6 |
| (e) RA long-term storage | PARTLY CONFIRMED (enrolment cert, request hash); ciphertext retention UNVERIFIED | [BRE] Table II p.15; [ASN] `RaEeCertInfo.requestHash` |
| (f) LOP on downloads in practice | UNVERIFIED | [BRE] p.5, p.12; [ARC] |
| (g) ETSI TR 103 949 | PARTLY CONFIRMED (no "harvest" text; confidentiality duration "N/A"; but "privacy … across the lifetime of the ITS-S") | [TR949] Tables 2–3, p.16 |
| (h) 5GAA/GSMA report | CONFIRMED; "not the main concern for V2X" | [5GAA] §3.3, p.07/32 |
| (i) NIST IR 8547 | CONFIRMED, still ipd; ECDSA/ECDH ≥128-bit disallowed after 2035, 112-bit deprecated after 2030 | [IR8547] Tables 2 and 4, PDF pp.20–21 |
| (j) Boldyreva et al. assumptions | CONFIRMED for corrupted-RA privacy (HDH, ROM); corrupted-ACA privacy is symmetric-only | [BKS] Thms 5.1, 5.2, 6.1, pp.10–11 |
