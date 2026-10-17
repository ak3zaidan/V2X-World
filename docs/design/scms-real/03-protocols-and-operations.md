# SCMS-real 03: protocols and real operating parameters (US SCMS)

Compiled 2026-10-16. This is research only. It records how the US SCMS actually talks: wire structures, transport, cadences, thresholds and published sizing figures. Every row carries a source tag that resolves in §0. Anything not confirmed against a primary text in this session is marked **UNCONFIRMED**. Values I derived myself (arithmetic, not stated in a source) are marked **derived**.

Companion: `docs/design/research/R6-scms-etsi-flows.md` (R6, 2026-09-17) already holds the CAMP Release 1.1 and Brecht flow detail. This file supersedes R6 where the two disagree, because it uses newer primary sources: the 1609.2.1-2022 ASN.1, CAMP Release 1.2.2, the SCMS Manager 1609.2.1 profile and the SCMS Manager specifications. Where a fact comes only from R6's reading of a source I did not re-read, the row says "via R6".

---

## 0. Source key

| Tag | Source | How read |
|---|---|---|
| [ASN-2.1] | IEEE Std 1609.2.1-2022 ASN.1 modules, ETSI Forge mirror, tag `2022-published` (commit 329ff688, 2022-10-28): `Ieee1609Dot2Dot1Protocol.asn` (OID …protocol(17) v3.1), `…EeRaInterface.asn`, `…EcaEeInterface.asn`, `…AcaEeInterface.asn`, `…AcaRaInterface.asn`, `…CertManagement.asn`, `…LaRaInterface.asn`, `…MaRaInterface.asn`, `…EeMaInterface.asn`. https://forge.etsi.org/rep/ITS/asn1/ieee1609.2.1/-/tree/2022-published | full text of the modules (the standard's prose is paywalled; the ASN.1 comments quote its clause numbers) |
| [ASN-2] | IEEE Std 1609.2 ASN.1 at the commit pinned by [ASN-2.1] (880216d3): `Ieee1609Dot2Crl.asn` (crl(3) v3.2), `Ieee1609Dot2CrlBaseTypes.asn`, `Ieee1609Dot2BaseTypes.asn`. https://forge.etsi.org/rep/ITS/asn1/ieee1609.2 | full text |
| [SMP] | SCMS Manager, "IEEE 1609.2.1 Profile", v1.0.3, 2023-03-15. Profiles IEEE 1609.2.1-2022. https://www.scmsmanager.org/wp-content/uploads/2023/07/IEEE1609Dot2Dot1-SCMS_Manager_Profile_1_0_3.pdf | full text |
| [SM-MBR] | SCMS Manager, "Misbehavior Report and Application Specification for Connected Vehicle Pilot Deployment", v1.0, 2019-02-14. https://scmsmanager.org/wp-content/uploads/2020/01/Misbehavior-Report-and-Application-Specification-v1.0.pdf | full text |
| [SM-MBR-ASN] | SCMS Manager, "ASN.1 for Misbehavior Reporting", v1.1. https://scmsmanager.org/wp-content/uploads/2020/01/ASN.1-for-Misbehavior-Reporting-v1.1.pdf | structure names read |
| [SM-REENR] | SCMS Manager, "EE Certificate Rollover (Re-Enrollment) Technical Standard", v1.0, 2020-10-08. https://www.scmsmanager.org/wp-content/uploads/2020/10/SCMS-Manager-EE-Re-Enrollment-Technical-Specification-v1.0.pdf | full text |
| [SM-PROV] | SCMS Manager, "Provider Requirements", v1.1, 2024-08-09. https://www.scmsmanager.org/wp-content/uploads/2024/10/SCMS-Manager-Provider-Requirements-v1_1.pdf | sections 3.1–3.6 read |
| [SM-EESEC] | SCMS Manager, "End-Entity Security Requirements, Design Guidance, and Validation Approach", v2.04, 2023-10-12. https://www.scmsmanager.org/wp-content/uploads/2023/10/SCMS-Manager-End-Entity-Security-Requirements-v2_04.pdf | §5.2 read |
| [SM-PUB] | SCMS Manager publications page (document and CTL file list). https://www.scmsmanager.org/publications/ | fetched |
| [CAMP122] | CAMP VSC5, "SCMS PoC Implementation — EE Requirements and Specifications Supporting SCMS Software Release 1.2.2", 2016-11-15 (posted 2023-07-19), 653 pp. https://pronto-core-cdn.prontomarketing.com/2/wp-content/uploads/sites/2896/2023/07/SCMS-CV-Pilots-Documentation-20230719.pdf | targeted sections read (cited by section and printed page) |
| [5GAA] | 5GAA Technical Report, "Credential Management Supporting V2X Commercial Deployments", v1.0, 2025-07-14. Annex A is a draft North-American CP. https://5gaa.org/content/uploads/2025/07/credential-management-supporting-v2x-commercial-deployments.pdf | targeted sections read. Draft policy, not adopted. |
| [PRIMER] | USDOT ITS JPO, "SCMS Technical Primer", FHWA-JPO-19-775, 2019. https://rosap.ntl.bts.gov/view/dot/43635 | targeted passages read |
| [CVP-P2] | USDOT ITS JPO, "CV Pilot Deployment Program: Driving Towards Deployment — Lessons Learned from the Design/Build/Test Phase", 2018. https://rosap.ntl.bts.gov/view/dot/37681 | items 19–27 read |
| [CVP-LOG] | USDOT ITS JPO, "CV Pilot Deployment Lessons Learned Logbook Synthesis", 2022. https://rosap.ntl.bts.gov/view/dot/66286 | SCMS passages read |
| [BRECHT] | Brecht et al., "A Security Credential Management System for V2X Communications", IEEE T-ITS 2018, arXiv:1802.05323 | via R6 |
| [J3287] | SAE J3287_202408, "V2X Misbehavior Reporting", issued 2024-08-30, 23 pp; ASN.1 set J3287SET_202408. https://saemobilus.sae.org/standards/j3287_202408-v2x-misbehavior-reporting | abstract only (paywalled) |
| [VENDOR] | Vendor capacity claims: Autocrypt (https://autocrypt.io/tag/pki/page/2/), ISS/Green Hills 2020 press release (https://ghs.com/news/20200320_ISS_TDOT_v2x_certificates.html) | search summaries only. Marketing figures, **secondary**. |

---

## 1. Ten facts that matter most for the engine

1. **One envelope type for everything.** Every EE↔SCMS message is an `Ieee1609Dot2Data` carrying an `ScmsPdu { version Uint8(2), content CHOICE {aca-ee, aca-la, aca-ma, aca-ra, cert, eca-ee, ee-ma, ee-ra, la-ma, la-ra, ma-ra} }`, under SecurityMgmtPsid = 35 (0x23). The la-ra, ma-ra, la-ma, aca-la, aca-ma and ee-ma PDUs are `NULL` in the standard, so 1609.2.1 does not define the backend-internal flows. [ASN-2.1] Protocol, LaRa, MaRa, EeMa modules
2. **Certificate requests are signed, then encrypted.** The EE signs `EeRaCertRequest` with its enrollment certificate and encrypts it to the RA certificate's encryptionKey (`EeRa1609Dot2AuthenticatedCertRequestSpdu`). The RA re-signs each per-certificate `RaAcaCertRequest` with its RA certificate. The ACA's pseudonym response is encrypted to the butterfly cocoon key, then signed by the same ACA (`AcaEeCertResponsePrivateSpdu`). The RA cannot read it. [ASN-2.1]
3. **The deployed profile is narrow.** The US profile uses the original butterfly mechanism only, no ACPC, IEEE-format implicit enrollment certificates only (no X.509), no OAuth, TLS 1.2 or 1.3 with server-only X.509 authentication (subjectAltName = component URL), no session resumption, and a TLS session lifetime of at most 5 minutes. Production errors are "coarse": HTTP 500 plus 404, 408 and 412. [SMP] §2.1.6–2.1.20, §3.2, §4.3.1, §6.2
4. **US time grid.** iPeriodLength = 1 week (168 h). iPeriodEpoch = 4 am Eastern, Tuesday 6 January 2015. Overlap = 1 h. Pseudonym certificates are valid 1 week + 1 hour, with 20 concurrently valid (20 + 20 during the overlap hour). [SMP] §2.1.17 Table 2; [CAMP122] Table 3
5. **Provisioning is request once, top up forever.** One request (ra-maxReqs = 1). The RA pre-generates up to ra-maxPreloadTime ∈ [2 weeks, 3 years] ahead, with ra-maxGenDelay = 1 week. The EE polls on its own schedule: download-minWait = 5 min, download-maxReqs = 100, download-maxAge = 1 h. Each batch is one i-period zip `<reqHash16hex>_<i hex>.zip` holding files `i_j`. [SMP] §2.1.21 Table 3; [CAMP122] §6.3.6
6. **Real endpoints (CAMP/CV-Pilot protocol).** RA at `https://<RA>:8892/…`: `/provision-pseudonym-certificate-batch`, `/download/batch`, `/download/info`, `/download/policy/local`, `/download/local-certificate-chain`, `/retrieve-ra-certificate`, `/provision-identity-certificate`, `/provision-application-certificate`, `/process-misbehavior-report`, `/ee-re-enrollment-request` and `-download`. The MA/CRL store is at `https://<MA>:8894/download-crl/`. The body is binary OER with `application/octet-stream`. Authenticated GETs carry a Base64 `Download-Req` header signed by the enrollment key, with a 5 s clock tolerance. [CAMP122] §6.2–6.3, §5.1.7; [SM-MBR] §4; [SM-REENR] §4
7. **CRL series.** 1 = OBU pseudonym (linkage seeds); 2 = SCMS components; 3 = RSU application and OBU identification certificates (HashedId10); 4 = enrollment certificates (rarely used, because the RA blocks them); 256 = root-signed (CRL signer, PG, MA). A CompositeCrl bundles all SecuredCrls plus the home CTL. CtlSeriesId = `03 48 00 00 00 00 00 00`. [SMP] Table 1, §6.1; [ASN-2] Crl; [ASN-2.1] CertManagement
8. **Revocation cadence is policy-bound, not real time.** A provider must publish a CRL within 1 business day of a change and before nextCrl [SM-PROV] §3.4.10. The draft North-American CP says within 24 h, and at least monthly with 4 days overlap [5GAA] (255), (422). CAMP EEs fetch "as often as possible but no later than once a week", with overdue_CRL_tolerance = 2 weeks [CAMP122] §3, Table 67.
9. **Misbehaviour reporting in practice.** The report is COER `EndEntityMaInterfacePDU` (PSID 0x26 for reporting; first application PSID is 0x20 BSM). It is signed with a pseudonym certificate, encrypted to the MA certificate, and POSTed to the RA at `:8892/process-misbehavior-report`, one report per POST. The RA shuffles reports, with a threshold of 10,000 reports or 1 day [CAMP122] §5.2.8.4. On the EE, the upload queue is ordered by weight, and each weight decays by ×0.9 every day at 5 am Eastern. [SM-MBR] §1–4
10. **Published sizing figures (the only "real" throughput numbers).** These are CAMP's design figures for 17 M new vehicles a year: initial provisioning ≈ 0.5 requests/s, .info/LPF/LCCF polling 14 → 267 requests/s (year 1 → year 10), batch downloads 85 → ~337 files/s. RA ACK within 1 s; EE retry after 10 s; at most 10 retries per 60 min. [CAMP122] §6.3.3.4–6.3.6.4, SCMS-521/522/523. No measured end-to-end latency from a live US deployment was found (§12).

---

## 2. Transport and session layer (as deployed)

| Item | Value | Source |
|---|---|---|
| Transport | HTTPS over TCP/IP, IPv4 and IPv6. The QA environment was "publicly available via Internet IPv6 and IPv4" | [CAMP122] §4.1, SCMS-513 |
| Style | REST-like. Payloads are binary ASN.1 (OER), not JSON. Content-Type `application/octet-stream`. "Cacheability … may not be used" | [CAMP122] §6.1, §6.3.1 |
| Ports (CAMP/CV-Pilot) | RA 8892, MA/CRL store 8894; URL form `https://<SERVER>:<port>/<PATH>` | [CAMP122] §6.2.1, §6.3.1 |
| Ports (1609.2.1 production) | The profile only says the ECA/RA operator communicates the `<SERVER>` URL out of band. **UNCONFIRMED** whether production RAs still use 8892 | [SMP] §2.1.2, §2.1.4, §2.1.16 |
| TLS versions | CAMP: TLS 1.2. SCMS Manager: `session-scmsAuth` = tls1.2 or tls1.3. ISO/TS 21177 not yet supported | [CAMP122] §6.3.3.5; [SMP] §2.1.18 |
| TLS suites (EE) | CAMP SCMS-341: EE supports ECDHE-ECDSA with AES-128/256 in CBC-SHA2, GCM and CCM (IANA C0,23 / C0,24 / C0,2B / C0,2C / C0,AC / C0,AD). The RA "supports at a minimum" ECDHE-ECDSA-AES128-SHA256. SCMS Manager: TLS 1.2 limited to ECDHE_ECDSA_WITH_AES_{128,256}_{CBC,GCM}_SHA{256,384}; TLS 1.3 limited to TLS_AES_256_GCM_SHA384 and TLS_AES_128_GCM_SHA256 with ECDHE and ECDSA | [CAMP122] SCMS-341, §6.3.3.5; [SM-EESEC] §5.2.3 |
| Server authentication | X.509 server certificate. Acceptance rule: subjectAltName matches the component's identifying URL, chained to a root the EE was provisioned with | [SMP] §3.2; [SM-MBR] §4 |
| Client authentication | Application layer, not TLS. The EE signs the request (or a download request carrying a Time32 timestamp) with its enrollment key: "TLS/1609.2 hybrid scheme". TLS client auth is "supported" but negotiated per operator | [CAMP122] §5.1.7.1, §6.3.3.5; [SMP] §3.2 |
| Session limits | TLS session resumption not supported. "The max lifetime of a session shall be 5 minutes" | [SMP] §3.2 |
| Request batching | All LPF/LCCF checks plus requests "should be sent within the same HTTPS session" | [CAMP122] §3 step 8 |
| Replay window | RA checks the EE-signed timestamp with a 5 s tolerance (SCMS-1203; config `ra.client-signature-ttl-sec`) | [CAMP122] §5.1.7.1, §6.3.4.1 |
| RA timing | RA sends a TCP ACK within 1 s (SCMS-521). EE retries if no response in 10 s (SCMS-522), at most 10 retries per 60 min (SCMS-523) | [CAMP122] §5.2.8.3 requirement table |
| Errors | CAMP prod: HTTP 500, no body. QA: detailed codes (e.g. 5065 "reschedule"). 1609.2.1 profile: `scmsV3-error` = coarse in production, fine in test; adds 404 (not generated yet), 408 (retry after minWait, up to maxReqs) and 412 (If-Modified-Since precondition: already current) | [CAMP122] §6.3.1, §6.3.4; [SMP] §2.1.20, §4.3.1 |
| Headers | `Range` (RFC 7233, resume certificate downloads; the RA must allow resume "even if EE switches the IP address"), `If-Modified-Since` (RFC 7232, CCF/CTL/CRL), CAMP LPF `If-None-Match: "local_policy_X_Y.oer"` → 304 | [SMP] §4.3.2; [CAMP122] §6.3.5, SCMS-544 |
| RA revocation | The RA's TLS certificate is not on an IEEE CRL. To "revoke" an RA, the operator re-points its DNS name. OCSP stapling optional | [CAMP122] §5.1.7.2 |
| LOP | Strips IP/MAC and forwards. The RA "shall provide downloads only via a LOP interface" (SCMS-517). One LOP can front several SCMS components (Fig. 39). 1609.2.1 keeps a LopSsp "for backward compatibility … in practice … the LOP does not have a certificate" | [CAMP122] §5.1.8, SCMS-517; [ASN-2.1] Protocol LopSsp |

---

## 3. IEEE 1609.2.1-2022 PDU catalogue

### 3.1 Wrapper and parameterised security types ([ASN-2.1] Protocol module)

| Wrapper | Meaning |
|---|---|
| `Ieee1609Dot2Data-Unsecured{T}` | `content.unsecuredData` CONTAINING T |
| `Ieee1609Dot2Data-Signed{T, Psid}` | signedData. headerInfo has psid only: generationTime, expiryTime, generationLocation, p2pcdLearningRequest, missingCrlIdentifier and encryptionKey all ABSENT. Signer = `SignerSingleCert` (exactly one certificate, not a digest) |
| `Ieee1609Dot2Data-Encrypted{T}` | encryptedData of T |
| `Ieee1609Dot2Data-SignedCertRequest{T, Signer}` | `content.signedCertificateRequest` = `SignedCertificateRequest {hashAlgorithmId, tbsRequest ScopedCertificateRequest, signer SignerIdentifier, signature}` |
| `…-SignedEncrypted{T,Psid}` | Encrypted{ Signed{T} } (sign, then encrypt) |
| `…-EncryptedSigned{T,Psid}` | Signed{ Encrypted{T} } (encrypt, then sign) |
| `…-SignedEncryptedCertRequest{T,Signer}` | Encrypted{ SignedCertRequest{T} } |
| `…-SymmEncryptedSingleRecipient{T}` | one RecipientInfo of form symmRecipInfo. Used LA→ACA for PLVs |
| `SecurityMgmtPsid` | `Psid (35)` = 0x23 |
| `CrlPsid` | `Psid (256)` [ASN-2] Crl |

### 3.2 SPDU table (name → content → protection → signer/recipient)

| SPDU | Inner PDU | Protection | Signer / recipient | Source note |
|---|---|---|---|---|
| `EeEcaCertRequestSpdu` | `eca-ee.eeEcaCertRequest` | SignedCertRequest, `SignerSelf` | self-signed with the new key, or with the canonical key if `canonicalId` is present | [ASN-2.1] |
| `EcaEeCertResponseSpdu` | `ecaEeCertResponse` | Signed, PSID 35 | ECA certificate (SSP = EcaSsp) | [ASN-2.1] |
| `EeRaCertRequestSpdu` | `ee-ra.eeRaCertRequest` | choice: `EeRa1609Dot2AuthenticatedCertRequestSpdu` = SignedEncryptedCertRequest, SignerSingleCert; or `EeRaX509AuthenticatedCertRequestSpdu` | signed by the enrollment certificate (profile 7.7.3.5); encrypted to the RA certificate's encryptionKey | [ASN-2.1]. X.509 variant not used in the US profile [SMP] §5.1 |
| `RaEeCertAckSpdu` | `raEeCertAck` | Signed, PSID 35 | RA certificate (7.7.3.9) | [ASN-2.1] |
| `RaEeCertInfoSpdu` (the `.info` file) | `raEeCertInfo` with acpcTreeId ABSENT | **Unsecured** | — | [ASN-2.1] |
| `RaEeCertAndAcpcInfoSpdu` | `raEeCertInfo` with acpcTreeId PRESENT | Signed | RA | [ASN-2.1] (ACPC not used in US) |
| `EeRaDownloadRequestPlainSpdu` | `eeRaDownloadRequest` | Unsecured | — | [ASN-2.1] |
| `EeRaDownloadRequestSpdu` | `eeRaDownloadRequest` | SignedEncrypted, PSID 35 | signed by the enrollment certificate, encrypted to the RA | [ASN-2.1] |
| `EeRaSuccessorEnrollmentCertRequestSpdu` | `eeRaSuccessorEnrollmentCertRequest` (= an `EeEcaCertRequestSpdu`) | SignedEncryptedCertRequest | outer: the current enrollment certificate; inner: self-signed by the new key; encrypted to the RA | [ASN-2.1] |
| `RaEeEnrollmentCertAckSpdu` | `raEeCertInfo` (acpcTreeId ABSENT) | Signed | RA | [ASN-2.1] |
| `RaAcaCertRequestSpdu` | `aca-ra.raAcaCertRequest` | SignedCertRequest, SignerSingleCert | RA certificate. Its certRequestPermissions must cover the requested appPermissions | [ASN-2.1] |
| `AcaRaCertResponseSpdu` | `acaRaCertResponse` | Signed, PSID 35 | ACA certificate (AcaSsp) | [ASN-2.1] |
| `AcaEeCertResponsePlainSpdu` | `aca-ee.acaEeCertResponse` | Unsecured | for non-pseudonym certificates where privacy from the RA is not needed | [ASN-2.1] |
| `AcaEeCertResponsePrivateSpdu` | `acaEeCertResponse` | **EncryptedSigned**: encrypted to the response key, then signed by **the same ACA that issued the certificate** (otherwise invalid) | ACA certificate | [ASN-2.1]. This is the US pseudonym path |
| `AcaEeCertResponseCubkSpdu` | `acaEeCertResponse` | Encrypted only (compact unified butterfly) | — | [ASN-2.1]. Not used in the US profile |
| `CompositeCrlSpdu` | `cert.compositeCrl` | Unsecured (each inner CRL is individually signed) | — | [ASN-2.1] |
| `CertificateChainSpdu` | `cert.certificateChain` (the CCF) | Unsecured | — | [ASN-2.1] |
| `MultiSignedCtlSpdu` | `cert.multiSignedCtl` | Unsecured container holding ≥1 `CtlSignatureSpdu` | — | [ASN-2.1] |
| `CtlSignatureSpdu` | `cert.tbsCtlSignature` | Signed, PSID 35 | Elector certificate (7.7.3.7) | [ASN-2.1] |
| `CertificateManagementInformationStatusSpdu` | `cert.infoStatus` | Signed, PSID 35 | RA (7.7.3.9) or DC (7.7.3.10) certificate | [ASN-2.1] |
| Misbehaviour report | — | the `EeRaInterfacePdu` CHOICE "does not include a PDU type for encrypted misbehavior report upload; see 4.1.5", and `EeMaInterfacePdu ::= NULL` ("being defined outside of this document") | — | [ASN-2.1] EeRa, EeMa |

### 3.3 Field-level structures ([ASN-2.1])

**EeRaCertRequest** `{version Uint8(2), generationTime Time32, type CertificateType (explicit|implicit), tbsCert ToBeSignedCertificate (cracaId '000000'H, crlSeries 0, appPermissions PRESENT, certIssuePermissions ABSENT, certRequestPermissions ABSENT, verifyKeyIndicator = verificationKey), additionalParams AdditionalParams OPTIONAL, …}`
- `AdditionalParams ::= CHOICE {original ButterflyParamsOriginal, unified ButterflyExpansion, compactUnified ButterflyExpansion, encryptionKey PublicEncryptionKey}`
- `ButterflyParamsOriginal ::= {signingExpansion ButterflyExpansion, encryptionKey PublicEncryptionKey, encryptionExpansion ButterflyExpansion}`, where `ButterflyExpansion ::= CHOICE {aes128 OCTET STRING (SIZE(16))}`
- When butterfly expansion is present, `tbsCert.verifyKeyIndicator` is the signing caterpillar public key. `tbsCert.validityPeriod` is the requested validity of the first batch. If `id` is LinkageData, the EE's value is replaced: the RA puts random data in, and the ACA inserts the real linkage value. The RA must store j, because the ACA never sees it but the EE needs it to decrypt.

**RaEeCertAck** `{version(2), generationTime, requestHash HashedId8, firstI IValue OPTIONAL (present for butterfly; used to form the download filename, cl. 8.2.2), nextDlTime Time32}`.

**RaEeCertInfo** (.info) `{version(2), generationTime, currentI IValue, requestHash HashedId8, nextDlTime Time32, acpcTreeId OPTIONAL}`.

**EeRaDownloadRequest** `{generationTime Time32, filename UTF8String (SIZE(0..255))}`.

**EeEcaCertRequest** `{version(2), generationTime, type, tbsCert (linkageData ABSENT, cracaId '000000'H, crlSeries 0, appPermissions ABSENT, certIssuePermissions ABSENT, certRequestPermissions PRESENT), canonicalId IA5String OPTIONAL}`.
**EcaEeCertResponse** `{version(2), generationTime, requestHash HashedId8, ecaCertChain SequenceOfCertificate (up to root), certificate, privateKeyInfo OCTET STRING(32) OPTIONAL}`. privateKeyInfo is the implicit-certificate reconstruction value.

**AcaEeCertResponse** `{version(2), generationTime, certificate, privateKeyInfo OCTET STRING(32) OPTIONAL}`.

**RaAcaCertRequest** `{version(2), generationTime, type, flags BIT STRING {butterflyExplicit(0), cubk(1)} (SIZE(8)), linkageInfo LinkageInfo OPTIONAL, certEncKey PublicEncryptionKey OPTIONAL, tbsCert (cracaId '000000'H, crlSeries 0, appPermissions PRESENT, …)}`
- `LinkageInfo ::= {encPlv1, encPlv2 EncryptedIndividualPLV}`
- `EncryptedIndividualPLV ::= {version(2), laId LaId, encPlv SymmEncryptedSingleRecipient{PreLinkageValue}}`, where `PreLinkageValue ::= OCTET STRING (SIZE(9))`. Each PLV is "encrypted by the LA for the ACA using the shared secret key", referenced by the hash of that symmetric key.
- When linkageInfo is present, tbsCert.id is LinkageData with iCert = the real i and a dummy LV for the ACA to replace.
- certEncKey selects the response type: absent and !cubk → Plain; absent and cubk → Cubk; present and !cubk → Private (encrypt to certEncKey, then sign).

**AcaRaCertResponse** `{version(2), generationTime, requestHash HashedId8, acaResponse CHOICE {plain, private, cubk}}`.

**CertManagementPdu** `CHOICE {compositeCrl, certificateChain, multiSignedCtl, tbsCtlSignature, infoStatus}`:
- `CompositeCrl {crl SEQUENCE OF SecuredCrl, homeCtl MultiSignedCtlSpdu}`
- `CertificateChain {homeCtl, others SEQUENCE OF Certificate}`
- `FullIeeeTbsCtl {type(fullIeeeCtl=1), ctlSeriesId OCTET STRING(8), sequenceNumber 0..65535, effectiveDate Time32, electorApprove/electorRemove SEQUENCE OF HashedId48, rootCaApprove/rootCaRemove SEQUENCE OF HashedId32, …, quorum INTEGER}`
- `ToBeSignedCtlSignature {ctlSeriesId, ctlType, sequenceNumber, tbsCtlHash HashedId48}`
- `CertificateManagementInfoStatus {crl SEQUENCE OF {cracaId HashedId8, series CrlSeries, issueDate Time32}, ctl SEQUENCE OF {ctlSeriesId, sequenceNumber, lastUpdate}, caCcf Time32, ma SEQUENCE OF {psids, updated Time32}, ra Time32 OPTIONAL}`. This is what an EE polls cheaply to decide whether anything changed.

**SSPs** (`SecurityMgmtSsp` CHOICE, all `{version Uint8(2), …}`): elector, root, pg, ica, eca, aca, crl, dcm, la (+`laId Uint16`), lop, ma (+`relevantPsids`: the MA certificate an EE must encrypt reports to for those PSIDs), ra, ee, dc.

### 3.4 CRL wire format ([ASN-2] Crl, CrlBaseTypes, BaseTypes)

- `SecuredCrl` = Signed `CrlContents`, headerInfo psid = 256, with all optional header fields absent.
- `CrlContents {version Uint8(1), crlSeries Uint16, crlCraca HashedId8, issueDate Time32, nextCrl Time32, priorityInfo {priority Uint8 OPTIONAL}, typeSpecific}`.
- `typeSpecific` is one of: fullHashCrl, deltaHashCrl (`{crlSerial Uint32, entries SEQUENCE OF {id HashedId10, expiry Time32}}`), fullLinkedCrl, deltaLinkedCrl, fullLinkedCrlWithAlg, deltaLinkedCrlWithAlg.
- `ToBeSignedLinkageValueCrl {iRev IValue(Uint16), indexWithinI Uint8, individual SEQUENCE OF JMaxGroup OPTIONAL, groups OPTIONAL, groupsSingleSeed OPTIONAL}`.
- `JMaxGroup {jmax Uint8, contents SEQUENCE OF LAGroup {la1Id, la2Id LaId(2 B), contents SEQUENCE OF IMaxGroup {iMax Uint16, contents SEQUENCE OF {linkageSeed1, linkageSeed2 OCTET STRING(16)}}}}`.
- `ExpansionAlgorithmIdentifier`: sha256ForI-aesForJ, sm3ForI-sm4ForJ.
- Consequences: `jmax` is a Uint8, so the number of certificates per i-period is at most 255 (CAMP Table 68 note: "CRL plan means that this can be set no higher than 255"). A pseudonym revocation costs 32 B of seeds plus grouping overhead, which is consistent with Brecht's ≈40 B/entry and ≈400 KB per 10,000 entries (via R6).

---

## 4. EE-facing use cases and endpoints

### 4.1 IEEE 1609.2.1 web-API use cases as profiled for the US ([SMP] §4.4)

| 1609.2.1 clause | Use case | scmsV3 eeAuth (US) | Options | Notes |
|---|---|---|---|---|
| 6.3.4.2 | Enrollment certificate request (ECA) | none or canonical | — | Enrollment certificates get wildcard permissions (certRequestPermissions = one PsidGroupPermissions, subjectPermissions = all). No LinkageData, no OperatingOrganizationId |
| 6.3.5.2 | Authorization certificate request | none or enrollment | — | identification and pseudonym certificates allowed; ACPC no |
| 6.3.5.3 | Authorization certificate download | none or enrollment | Range | "shall use the variation **without the filename in the URL**". butterflyType = original |
| 6.3.5.4 / 6.3.5.5 | Successor enrollment request / download | enrollment | Range (download) | — |
| 6.3.5.6 | Misbehaviour report submission | none or enrollment | — | format in SCMS Manager MBR spec |
| 6.3.5.7 | Certificate chain file incl. CTL | none | If-Modified-Since | US CtlSeriesId. Preferred over separate CTL/CA downloads |
| 6.3.5.8 | Composite CRL incl. CTL | none | If-Modified-Since | preferred over individual CRLs |
| 6.3.5.9–6.3.5.13 | Individual CA certificate, individual CRL, CTL, RA certificate, MA certificate | none | If-Modified-Since | — |
| 6.3.5.14 | Certificate management information status | none | If-Modified-Since | — |
| DC-EE, EE-SAS, OAuth (6.2), ISO 21177, X.509 enrollment | — | not supported in the profile | — | [SMP] §2.1.1, §2.1.5, §4.2, §5.1 |

The exact 1609.2.1 v3 URL path strings (clause 6.3) are **UNCONFIRMED**. The standard text is paywalled and no public copy of the path table was found. The profile states only that the server URL is supplied out of band.

### 4.2 CAMP / CV-Pilot RA and MA services ([CAMP122] §6.2–6.3, Table 79; [SM-MBR] §4; [SM-REENR] §4)

| Service | Method | Path (port 8892 unless noted) | Body / header | Response |
|---|---|---|---|---|
| Request pseudonym batch provisioning | POST | `/provision-pseudonym-certificate-batch` | `SecuredPseudonymCertProvisioningRequest`, encrypted to the RA certificate | `SignedPseudonymCertProvisioningAck`: requestHash (low 8 bytes), certDLTime, certDLURL |
| Download pseudonym batch | GET | `/download/batch` | `Download-Req` header: Base64 `SecuredAuthenticatedDownloadRequest` → `SignedAuthenticatedDownloadRequest` → `AuthenticatedDownloadRequest{filename "[0-9A-F]{16}_[0-9A-F]{1,8}.zip", time}`; optional Range | zip with flat files `X_Y` (X = i hex, Y = j hex, j = 0…j_max−1), each an OER `SignedEncryptedCertificateResponse`. Example `AB09281C9867DE53_F.zip` = i 15 |
| Download .info | GET | `/download/info` | `Download-Req` filename `[0-9A-F]{16}.info` | single Time32 (next batch time). Error 5065 means reschedule |
| Download local policy file | GET | `/download/policy/local` | optional `If-None-Match: "local_policy_<global>_<local>.oer"` | `SignedLocalPolicyFile` or 304 |
| Download local certificate chain file | GET | `/download/local-certificate-chain` | optional If-None-Match | LCCF |
| Retrieve RA certificate | GET | `/retrieve-ra-certificate` | — | RA certificate |
| Request / download identification certificate | POST / GET | `/provision-identity-certificate`, `/download/identity-certificate` | as above | — |
| Request / download RSE application certificate | POST / GET | `/provision-application-certificate`, `/download/application-certificate` | as above | — |
| Submit misbehaviour report | POST | `/process-misbehavior-report` | body `SecuredMisbehaviorReport`; `Download-Req` header with filename "misbehavior-report", signed by the enrollment key | empty |
| Re-enrollment request | POST | `/ee-re-enrollment-request` | `SecuredReEnrollmentRequest` | `SecuredReEnrollmentCertProvisioningAck`: requestHash, certDLTime, certDLURL |
| Re-enrollment download | GET | `/ee-re-enrollment-download` | `Download-Req` filename `[0-9A-F]{16}.zip` | `SignedEeEnrollmentCertResponse` |
| Download CRL (CRL store, MA) | GET | `https://<MA>:8894/download-crl/` | none. "does not authenticate the downloading entity" | OER CompositeCrl |

The ordering rule on every connection is: download LPF and LCCF first, then requests, then batches, then .info, all in one HTTPS session ([CAMP122] §3, §5.2.7.8.4; SCMS-709).

---

## 5. Bootstrap / enrollment at manufacture and re-enrollment

| Item | Value | Source |
|---|---|---|
| Bootstrap topology | Device → DCM (secure environment) → ECA. The DCM returns the enrollment certificate, ECA certificate, RA certificate and RA FQDN, plus elector, root, MA, PG and CRLG certificates | [BRECHT] §V-A via R6; [CAMP122] §5.2.13.3 (RSE: "has root CA, RA and PCA certificates installed … knows the FQDN of the RA") |
| CV-Pilot reality | manual bootstrap (Use Case 2 / 12, "currently both processes are exactly the same"). Automatic enrollment was a future feature | [CAMP122] §3 item 7, §4.2 |
| 1609.2.1 direct authorization (US) | ECA URL given out of band. Canonical identity = HashedId32 of the canonical public key. The EE operator gives the canonical identity and key to the ECA. Key must be made in a "secure environment" on a device meeting the EE Security Requirements | [SMP] §2.1.7 |
| Indirect authorization | supported; details per ECA operator | [SMP] §2.1.7 |
| Registration with the RA | The RA only serves EEs whose enrollment certificate is **registered** with it. The EE operator gives the enrollment certificate to the RA (how is out of scope). The RA must support blocking | [SMP] §2.1.8 |
| Enrollment request validity (US) | region = USA, Canada, Mexico only. Key never used before with that operator. Validity inside the ECA's. crlSeries = 4. id.name present and unique. type = implicit | [SMP] §6.3.1 |
| Enrollment certificate lifetime | CAMP Release 1.2.2: **6 years**, rollover cert available ≤ 3 years before start. CAMP Release 1.1: 7 years max (via R6). 5GAA draft CP: max validity 3 years, preload 3 months | [CAMP122] Table 3, SCMS-1307; [5GAA] (419) |
| Successor enrollment (1609.2.1) | `eeRaSuccessorEnrollmentCertRequest` via the RA. An `EeEcaCertRequestSpdu` self-signed by the new key, wrapped and signed by the current enrollment certificate, encrypted to the RA | [ASN-2.1] |
| Re-enrollment (CAMP protocol, SCMS Manager) | two-phase (request + later download). One per enrollment-certificate lifetime. Recommended 12–24 months before expiry. No validity overlap: the new certificate starts when the old one expires. Same PSID/SSP and region. EE generates a new key (no injection). The RA re-signs and forwards to the ECA, links old and new certificates, and existing pseudonym batches stay downloadable with the new certificate. A second request before download invalidates the first, and blacklists it if already issued | [SM-REENR] §1–3 |
| Pilot practice | "devices must be returned to a secure environment for re-enrollment" | [CVP-P2] item 27 |

---

## 6. Pseudonym provisioning cadence and top-up

### 6.1 Time-period parameters (US) — [SMP] §2.1.17 Table 2 (matches the CV Pilots)

| Parameter | Value |
|---|---|
| iPeriodEpoch | 4 am Eastern Time, Tuesday 6 January 2015. **Derived:** 09:00 UTC, which is 5 am EDT, consistent with Whyte's "rollover at 5 am Eastern on Tuesday" (via R6). i-value on 2026-10-16 ≈ 614 (**derived**) |
| iPeriodInit | 0 |
| iPeriodLength | 1 week (168 h) |
| overlap | 1 hour |
| baseCertNumber | PSID-specific range (1, …). Identification certificates use a "local" i-value |
| overlapCertNumber | = baseCertNumber |
| extraBands | 0 |

### 6.2 Use-case parameters (US defaults absent an LPF) — [SMP] §2.1.21 Table 3

| eca-maxAge | eca-maxReqs | eca-maxWait | eca-minWait | ra-acpcSupport | ra-butterflyType | ra-maxAge | ra-maxGenDelay | ra-maxReqs | ra-maxPreloadTime | ra-minWait | download-maxAge | download-maxReqs | download-minWait |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 week | 100,000 | 1 month | 1 hour | No | Original | 1 week | 1 week | 1 | 2 weeks – 3 years | 1 hour | 1 hour | 100 | 5 minutes |

The exact semantics of each parameter (1609.2.1 Tables 5–7) are paywalled. Reading the names together with the 408 rule ("retry after *-minWait, up to *-maxReqs") [SMP] §4.3.1, *-maxAge is most likely the maximum request age. That reading is **UNCONFIRMED** beyond the names.

### 6.3 CAMP global and local policy options — [CAMP122] §5.2.15.5, Tables 67–68

| Option | PoC value | Note |
|---|---|---|
| i_period | 1 week (10,080 min) | global |
| min_certs_per_i_period | 20 | "No maximum"; ≤ 255 by CRL design |
| certs_per_i_period | 20 (40 tested) | local (per RA) |
| cert_validity_model | concurrent | all 20 valid all week |
| max_available_cert_supply | 3 years | initial plus maintained by top-off |
| max_cert_request_age | 2 days | maximum time an individual certificate request waits in the aggregator to be shuffled (initial requests only) |
| shuffle_threshold | **1000** | "minimum number for full privacy mode" (see §10 for the conflicting 10,000 value) |
| hash_of_request_size | 32 bytes | full hash in the PoC (on the wire it is HashedId8) |
| overdue_CRL_tolerance | 2 weeks | then reject all messages and warn the driver |
| max_gpf_gccf_retrieval_interval | 1 day | for RA, PCA and LA |
| rse_application_cert_validity / _overlap | 169 h / 1 h | — |
| address_la1, address_la2, address_pca, tls_cert_* | local configuration | — |
| shared_key_update_interval | local | LA↔PCA PLV symmetric key rotation |

### 6.4 Cadence facts

| Item | Value | Source |
|---|---|---|
| Initial request size | 3,120 certificates = 20/week × 52 × 3 years | [CAMP-EE R1.1] via R6 |
| Pre-generation | The PCA/RA keeps 3 years generated and adds one week every week. The OBU downloads "however many weeks' worth … the device vendor configures, up to 3 years" | [PRIMER] PDF p.6 |
| Pilot choices | THEA downloaded 3 years at once. NYC loaded **1 week** ahead (because there was no MBD/CRL) and **60 certificates/week** for taxis | [CVP-LOG] PDF p.16; [CVP-P2] items 21, 24 |
| Request once | OBE requests only the initial set, or again when parameters change (SCMS-520). ra-maxReqs = 1 | [CAMP122]; [SMP] |
| Top-up trigger | after the `.info` / `nextDlTime` time: "whenever it suits your pseudonym certificate download strategy". The RA updates .info for every EE even with no new batch (SCMS-576). Top-up is incremental (EE skips files it has) | [CAMP122] §3 item 9, §5.2.7.8 |
| Re-download | allowed while credentials are valid and certificates unexpired (SCMS-549) | [CAMP122] |
| Inactivity | The RA records last contact. Stopping pre-generation for inactive devices is "Not Doing". Generation stops for blacklisted devices (SCMS-509) | [CAMP122] §5.2.7.8.7–.8 |
| First-batch delay | "some delay … to accommodate … shuffling, certificate generation, and certificate encryption". Initial-provisioning shuffle "may be relaxed" | [CAMP122] §5.2.7.6.7.1, §5.2.7.2 |
| Field failure mode | devices that ran out of certificates during shipping needed a reboot to trigger download | [PRIMER] PDF p.9 |
| CV-Pilot certificate operations | NYC ran weekly monitoring of top-off success or failure | [CVP-LOG] PDF p.16 |

---

## 7. Identification and RSU application certificates

| Item | Value | Source |
|---|---|---|
| RSU application certificate | 1 week + 1 h validity, 1 (+1 during the overlap hour) concurrent. **Not pre-generated.** Requested weekly. No shuffle. No butterfly; signed by the enrollment certificate, encrypted to the RA. The PCA signs it, encrypts to the RSE, signs the ciphertext. The RA zips it untouched | [CAMP122] Table 3, SCMS-1213, §5.2.13.2–.4; [CVP-P2] item 26 |
| Operational consequence | RSUs must reach the SCMS at least weekly. Wyoming could not keep application or identification certificates valid in rural areas and used pseudonym certificates (3-year supply) for its distress application | [PRIMER] PDF p.9; [CVP-P2] item 26 |
| OBE identification certificate | 1 month + 1 h, 1 (+1) concurrent. Butterfly keys for signing and response (and optionally certificate encryption). The RA does not shuffle. Revocation by HashedId on CRL series 3 (no linkage values) | [CAMP122] Table 3, SCMS-1370, §5.2.16.2–.4 |
| CRL series and validity rules for requests (US) | butterfly + LinkageData → crlSeries 1. Butterfly without LinkageData → 3. No butterfly → 3, and id must not be LinkageData. type implicit | [SMP] §6.3.3 |
| 5GAA draft limits | pseudonym AC: max preload 3 years, validity 1 w + 1 h, 10–100 parallel per PSID-SSP. RSE AC: same validity, 1–2 parallel | [5GAA] (419) |

---

## 8. Misbehaviour reporting, investigation, revocation

### 8.1 Report format and EE behaviour (US CV-Pilot specification)

| Item | Value | Source |
|---|---|---|
| Reporting PSID | 0x26. Application PSID for the first defined report = 0x20 (BSM) | [SM-MBR] §1 |
| Top level | `EndEntityMaInterfacePDU` CHOICE → `EeMaApplicationSpecificMisbehaviorReport {version(1), generationTime Time32, generationLocation ThreeDLocation, psid, contents}` (CLASS-bound) | [SM-MBR] §1; [SM-MBR-ASN] |
| PSID-0x20 types / subtypes | 1 `1VInconsistent` (1 ConstantPosition, 2 RandomPosition with gpsDrift 1 m). 2 `Implausible` (1 MaxSpeed > 90 m/s, 2 MaxAcceleration > 10 m/s², 3 BrakeAcceleration). 3 `ObsImplausible` (1 Distance > 3000 m + 10 m drift, **default off** because of relay attacks) | [SM-MBR] §2.3; [SM-MBR-ASN] |
| Evidence | `SignedBsmsWithCertificate {signedBsmList SEQUENCE (SIZE(1..MAX)) OF SignedBSM, signingCertificate Certificate}`. ObsImplausible carries suspectBsm + reporterBsm. One report may list several subtypes | [SM-MBR-ASN]; [SM-MBR] §1 |
| Security processing | COER → 1609.2 Signed (headerInfo: generationTime only; signer = full EE certificate, which is a pseudonym certificate in CAMP's design) → Encrypted to the MA certificate (ephemeral key, single recipient, compressed points) | [SM-MBR] §3; [CAMP122] §5.2.8.4 |
| Which MA | the MA certificate whose MaSsp.relevantPsids contains the application PSID | [ASN-2.1] MaSsp |
| Storage and upload priority | Each report has an initial weight (default 1.0), multiplied daily at **5 am Eastern** by a multiplier (default 0.9). Upload highest weight first (ties: newer first). Over quota, delete lowest weight (ties: older first). Delete after the TCP-confirmed upload. Unsent reports > 1 week may be deleted | [SM-MBR] §2.2; [CAMP122] §5.2.8.4 step 5 |
| Upload | per-report HTTPS POST to the RA (§4.2), whenever in range of an RSU offering SCMS connectivity | [SM-MBR] §4; [PRIMER] PDF p.6 |
| Newer formats | SAE J3287_202408 (general MBR format + system-wide misbehaviour-management architecture; content paywalled). 1609.2.1 leaves the format external. Whyte 2022 cites ETSI TS 103 759 as the direction (via R6) | [J3287]; [ASN-2.1] EeMa |

### 8.2 Backend handling

| Step | Mechanism | Source |
|---|---|---|
| RA → MA | The LOP strips identifiers. The RA shuffles and forwards individually. Threshold **10,000 reports or one day**, whichever first (PoC; "needs to be re-evaluated by SCMS manager for production") | [CAMP122] §5.2.8.4, SCMS-765 |
| MA processing preconditions (draft CP) | valid reporter signature, valid elector certificates, valid CRL + CTL, valid MA chain | [5GAA] (263) |
| Investigation | MA → PCA: LV → encrypted (plv1, plv2). MA → one LA: "same device?" boolean. Collaboration sets: {MA, PCA, one LA} link. {MA, PCA, RA, both LAs} revoke. {MA, PCA, RA} blacklist | [BRECHT] §VI-B/C via R6 |
| Revocation | MA → PCA: LV → hash of RA-to-PCA request + RA name. MA → RA: hash → the RA blacklists the enrollment certificate (never revealed) and returns the LCI pair. MA → LA1/LA2: LCI → ls(i) for the current i. CRLG publishes {ls1, ls2, la ids, iRev, jmax} | [BRECHT] §VI-D via R6 |
| Blacklisting at the RA | Every logical RA keeps its own Internal Blacklist, unshared, fed by the MA's Internal Blacklist Manager (SCMS-507). Blacklisted → HTTP 500 at login (SCMS-1204) and pre-generation stops (SCMS-509) | [CAMP122] |
| Enrollment blocking (1609.2.1 era) | ECA/RA/SAS set the status to "blocked" when the MA decides an EE is not working correctly. Processing ≤ 5 days normally, ASAP on key compromise | [5GAA] (257)–(259) |
| Pilot reality | PROD initially had no MA. QA had manual revocation only. Pilots used a manual "minimum viable" MA: an operator reviews reports and adds devices to the CRL | [CAMP122] §4.2–4.3; [PRIMER] PDF pp.6, 8 |
| OBE reaction | expand the CRL into LVs for the current i, drop matching certificates, recompute before each i-period, prune exhausted entries. If the OBE finds itself, or its ECA/PCA/ICA/root, revoked, it stops transmitting for that PSID/SSP | [CAMP122] §5.2.10.2.3 |

---

## 9. Backend-internal flows (RA↔LA, RA↔ACA, MA↔others)

1609.2.1 defines only the RA↔ACA structures (§3.3). LA-RA, MA-RA, LA-MA, ACA-LA and ACA-MA are `NULL` in the 2022 ASN.1 ("not used by EEs") [ASN-2.1]. Everything below comes from CAMP or Brecht.

| Flow | Content | Protection | Source |
|---|---|---|---|
| RA → LA1, LA2 | request PLV chains for a device (identified by LCI). The RA keeps a PLV cache: provisioning precondition "PLV Cache has at least one PLV chain" | TLS between components. PLVs individually encrypted for the ACA (symmetric key shared LA↔ACA, rotated per `shared_key_update_interval`) | [CAMP122] §6.3.3.1, Table 68; [ASN-2.1] EncryptedIndividualPLV; [BRECHT] §V via R6 |
| RA → ACA | one `RaAcaCertRequestSpdu` per certificate after shuffling: cocoon key B, response key (certEncKey), linkageInfo{encPlv1, encPlv2}, dummy id | signed by the RA certificate (RaSsp) | [ASN-2.1]; [CAMP122] Table 68 |
| ACA → RA | `AcaRaCertResponseSpdu{requestHash, acaResponse.private}` | signed by the ACA. The inner response is encrypted to the EE, then ACA-signed | [ASN-2.1] |
| RA bundling | collect one i-period per device into `X_Y.zip` in that device's folder (SCMS-534) | — | [CAMP122] |
| Components ↔ PG | fetch GPF/GCCF at least daily | — | [CAMP122] Table 67 |
| Separation rules | PCA ≠ RA, PCA ≠ LA, LA1 ≠ LA2, LOP ≠ RA/MA, MA ≠ RA/LA/PCA (organisational) | — | [BRECHT] §X via R6 |
| Hostnames | the component certificate's id must match its FQDN (SCMS-1725). Recommended host identifiers: elector, rootca, ica, ma, pg, crlg, aca, eca | — | [CAMP122] Table 4; [SM-PROV] §3.3.1.1 |

---

## 10. CRL and CTL issuance and distribution

| Item | Value | Source |
|---|---|---|
| CRL series (US) | 256: root-signed (CRL signer, PG, MA), by certificate hash. 2: ICA, ACA, ECA, RA, LA, by hash. 1: OBU pseudonym, by linkage seeds. 3: RSU application + OBU identification, by hash. 4: enrollment, by hash ("may not need to be used"). Composite download preferred | [SMP] Table 1 |
| Provider publication SLA | within 1 business day of any change, and before nextCrl. Revoked certificates stay until expiry. No suspension | [SM-PROV] §3.4.9–3.4.10 |
| Draft NA CP | publish CRLs within 24 h of a status change via DC and/or RA. Issue at least monthly with 4 days overlap. Component revocation ≤ 5 days, ASAP on compromise | [5GAA] (49), (254)–(255), (422) |
| EE fetch cadence (CAMP) | "as often as possible but no later than once a week". On every RA connection (the RA relays the CRL) | [CAMP122] §3 item 10; [PRIMER] PDF pp.5, 8 |
| Overdue tolerance | 2 weeks past the next-CRL date, then distrust everything and warn | [CAMP122] Table 67 |
| Endpoints | CAMP CRL store `:8894/download-crl/` (unauthenticated). 1609.2.1: composite CRL, individual CRL, CMIS via RA or DC (If-Modified-Since) | [CAMP122] §6.2.3; [SMP] §4.4.8–4.4.14 |
| Broadcast | CRL Broadcast via RSUs or satellite radio ("pass-through") | [PRIMER] PDF p.5 |
| Pilot gap | in Phase 2 the SCMS "does not currently have a standard Misbehavior Detection and … CRL distribution mechanism" | [CVP-P2] item 21 |
| CTL | signed by m-of-N electors (FullIeeeTbsCtl.quorum). US CtlSeriesId `0348000000000000`. A new CTL is issued only on SCMS Manager request (add or remove root or elector). Root addition published ≤ 3 business days (provider rule). Draft CP: ≤ 15 days after an addition, ≤ 24 h after a removal | [SMP] §6.1; [SM-PROV] §3.4.11; [5GAA] (47) |
| Live CTL files | production CTL updated 2025-08-13. Test CTLs named `SCMS_Manager-Test_CTL-0348000000000000-<n>.zip`. **Derived:** n read as Time32 gives 2025-07-16 for the file published 2025-07-17, so n is probably the CTL's effective date | [SM-PUB] |

---

## 11. Privacy parameters

| Parameter | Value | Source |
|---|---|---|
| Shuffle of certificate requests, RA → PCA | **conflict:** Brecht "10,000 requests or a day's worth" (via R6) vs CAMP 1.2.2 `shuffle_threshold` = **1000** plus `max_cert_request_age` = 2 days. "Shuffle_threshold and certs_per_i_period must be consistent throughout the SCMS" | [BRECHT] §V-B via R6; [CAMP122] Tables 67–69, SCMS-631 |
| Shuffle of misbehaviour reports, RA → MA | 10,000 reports or 1 day | [CAMP122] SCMS-765 |
| No shuffle | RSU application and OBE identification requests | [CAMP122] §5.2.13.2, §5.2.16.4 |
| Certificates per week | 20 (sized for ≈2 h/day of driving); NYC 60. 5GAA draft range 10–100 | [PRIMER] PDF p.8; [CVP-P2] item 24; [5GAA] (419) |
| Certificate change | J2945/1 CERTCHG: every 5 min, except when < 2 km from the last change point. NYC: every 2 km travelled or 5 min, whichever first | [CVP-P2] item 25; [PRIMER] PDF pp.8–9 |
| Linkage value / PLV | 9 bytes. Linkage seed 16 B. LA id 2 B | [ASN-2] BaseTypes; [ASN-2.1] AcaRa |
| Location hiding | LOP on every EE→SCMS path (SCMS-517) | [CAMP122] |

---

## 12. Latency and throughput figures

| Quantity | Value | Type | Source |
|---|---|---|---|
| Initial provisioning rate | 17,000,000 / 31,557,600 s ≈ 0.5 requests/s (all years) | design sizing | [CAMP122] §6.3.3.4 |
| .info, LPF, LCCF, RA-certificate polling | 14 (y1), 70 (y3), 126 (y5), 267 (y10) requests/s: half of new cars plus all existing cars once a week | design sizing | [CAMP122] §6.3.4.4, §6.3.5.4, §6.3.7, §6.3.12 |
| Batch-file downloads | 85 files/s (y1: 17 M × 156 files) rising to ~337 files/s (y10: 170 M fleet) | design sizing | [CAMP122] §6.3.6.4 |
| Certificate issuance capacity | Autocrypt: up to 300 billion certificates/year (≈9,500/s **derived**). ISS (2020): scalable to 150 billion/year (≈4,800/s **derived**) | marketing, secondary | [VENDOR] |
| RA ACK / EE retry / retry cap | 1 s / 10 s / 10 per 60 min | requirement | [CAMP122] SCMS-521..523 |
| Clock tolerance | 5 s | requirement | [CAMP122] SCMS-1203 |
| TLS session lifetime | ≤ 5 min | requirement | [SMP] §3.2 |
| Wait parameters | ra-minWait 1 h, download-minWait 5 min, ra-maxGenDelay 1 week, eca-maxWait 1 month | requirement | [SMP] Table 3 |
| Certificate sizes | pseudonym 86 B, identification 89 B, RSE application 89 B, enrollment 87 B; RA 217, LA 205, PCA 216, ECA 150, ICA 195, MA 205, CRLG 190, PG 172, RCA 211, elector 166 B ("not fixed size") | example | [CAMP122] Table 3 |
| End-to-end measured latency (request → first batch; report → CRL) | **not found** for any live US deployment. CAMP gives no numeric first-batch bound | — | [CAMP122] §5.2.7.6.7.1; R6 §C |

Derived from the table above, not stated by a source: a 20/week × 156-week initial download is 3,120 × (86 B certificate + encryption and signature overhead) per device. The exact `SignedEncryptedCertificateResponse` size is **UNCONFIRMED**.

---

## 13. Realistic topology

| Aspect | What the sources support | Source |
|---|---|---|
| EE ↔ SCMS path | OBU → RSU (advertising SCMS or IP service) or cellular → internet → LOP → RA (HTTPS). Field networks had to carry IPv6 from OBU → RSU → TMC → internet | [PRIMER] PDF p.7; [CVP-LOG] PDF p.15 |
| Component links | TLS between all online components. Data passed through a component that must not read it is additionally encrypted at the application layer (PLVs LA→ACA via RA; ACA responses via RA; MBRs via RA) | [BRECHT] §II-B via R6; [ASN-2.1] |
| Offline elements | Root CA and electors offline, HSM FIPS 140-2 Level 3 or higher. Root key cryptoperiod 20 years, root certificate lifetime 70 years (SCMS Manager) | [SM-PROV] §3.5.2, §3.6 |
| Cloud | Sub-CAs (ICA, ECA, RA, ACA, LA, MA, CRL signer, DC) "may be operated in a secure cloud environment based on risk analysis and with certified HSM" (draft CP) | [5GAA] (281) |
| Environments | CAMP ran TEST (internal), QA (public internet) and PROD (offline commercial root CA in an HSM), each with its own root. PROD had no MA initially. Software was promoted TEST → QA (weeks) → PROD with USDOT approval | [CAMP122] §4 |
| Multiplicity | multiple RAs, ECAs, PCAs. Exactly two LAs. One intrinsically central MA, PG and SCMS Manager. Each RA its own blacklist | [BRECHT] §II via R6; [CAMP122] SCMS-507; [PRIMER] PDF p.7 |
| Geographic placement | No source gives data-centre locations for US SCMS providers. **UNCONFIRMED** | — |

---

## 14. Conflicts and open items

| # | Conflict / gap | Resolution suggestion |
|---|---|---|
| 1 | Request shuffle threshold 10,000/day (Brecht) vs 1000 + 2-day age (CAMP 1.2.2) | Treat both as configurable; default to CAMP 1.2.2 (newer, normative for the pilots) |
| 2 | Enrollment lifetime 7 y (R1.1) vs 6 y (R1.2.2) vs 3 y (5GAA draft) | Default 6 y (pilots); expose as a parameter |
| 3 | CAMP REST paths and ports vs 1609.2.1 v3 paths (unknown) | Model CAMP paths; mark the 1609.2.1 paths **UNCONFIRMED** |
| 4 | Pseudonym response name: CAMP `SignedEncryptedCertificateResponse` vs 1609.2.1 `AcaEeCertResponsePrivateSpdu` | Same construction (encrypt, then ACA-sign); use the 1609.2.1 name |
| 5 | Semantics of `*-maxAge` and `ra-maxGenDelay` | **UNCONFIRMED** (paywalled Tables 5–7) |
| 6 | Misbehaviour report format for 1609.2.1 deployments: SCMS Manager CVPD v1.0 vs SAE J3287 vs ETSI TS 103 759 | Use SCMS Manager v1.0 (public, deployed); J3287 content **UNCONFIRMED** |
| 7 | Real CRL sizes and cadence from a live US provider | **UNCONFIRMED**; only SLAs and design estimates exist |
