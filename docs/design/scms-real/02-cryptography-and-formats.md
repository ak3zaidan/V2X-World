# 02 — SCMS cryptography and data formats, exactly

Status: research only (2026-10-16). No code was changed. Scope: everything the engine must compute
bit for bit to be an IEEE 1609.2 / 1609.2.1 SCMS clone: certificate and SPDU formats, encoding,
every hash, signature, key-derivation and encryption step, butterfly keys, linkage values, CRLs,
chain validation, and the published test vectors.

Notation: `||` is concatenation; `H` is SHA-256; `n` is the P-256 group order; `G` the base point;
`AES_k(x)` is one AES-128 block encryption; integers are big-endian unless stated.
**VERIFIED** means I recomputed the published vector myself with an independent script (Python,
`cryptography` package) in this session. **UNCONFIRMED** means I could not see the normative text.

---

## 0. Source key

| Id | Source | Where | Kind |
|---|---|---|---|
| S1 | IEEE 1609.2-2022 ASN.1 modules (`Ieee1609Dot2` v2.6, `Ieee1609Dot2BaseTypes` v2.4, `Ieee1609Dot2Crl` v3.2, `Ieee1609Dot2CrlBaseTypes` v3.2, `Ieee1609Dot2CrlSsp` v2.2). The ASN.1 comments carry the normative per-field text and clause numbers. | tag `2022-published`: https://forge.etsi.org/rep/ITS/asn1/ieee1609.2/-/tree/2022-published ; also the official IEEE download https://standards.ieee.org/wp-content/uploads/2023/02/1609.2-2022_downloads.zip | primary |
| S2 | IEEE 1609.2.1-2022 ASN.1 modules (EE-RA, ACA-RA, ACA-EE, Protocol, ACPC...) | tag `2022-published`: https://forge.etsi.org/rep/ITS/asn1/ieee1609.2.1 ; IEEE zip https://standards.ieee.org/wp-content/uploads/2022/05/1609.2.1-2022_downloads.zip | primary |
| S3 | IEEE 1609.2.1-2026 download bundle (includes the 1609.2-2025 modules: `Ieee1609Dot2` v2.7, BaseTypes v2.5, CrlBaseTypes v3.3) | https://standards.ieee.org/wp-content/uploads/2026/06/1609.2.1-2026_downloads.zip ; forge branch `v2025` | primary |
| S4 | IEEE Std 1609.2a-2017 (Amendment 1), full text: §5.3.1, §5.3.2, §5.3.5, §5.1.2.4, §6.4.x, **Annex D.6 cryptographic test vectors** | https://docbox.etsi.org/STF/Archive/STF538_TC_ITS/STFworkarea/libaries/IEEE_Std_1609_2a-2017.pdf | primary (hosted by ETSI) |
| S5 | SECG SEC 4 v1.0 (2013), ECQV: §2.3 (Hn), §3.3–3.8 | https://www.secg.org/sec4-1.0.pdf | primary |
| S6 | CAMP SCMS crypto test vectors and generator scripts (`lv`, `glv`, `bfkeyexp`, `implicit`, `ecies`, `kdf`, `mac1`, `aesccm`, `ecdh`). README points to CAMP wiki page "Modifications to Crypto Primitives" (https://wiki.campllc.org/display/SP/Modifications+to+Crypto+Primitives; the wiki refused connections today). | https://github.com/conz27/crypto-test-vectors (last push 2017-11-14, no licence file) | CAMP-origin, unofficial mirror |
| S7 | Brecht, Therriault, Weimerskirch, Whyte, Kumar, Hehn, Goudy, "A Security Credential Management System for V2X Communications", IEEE T-ITS 2018: §IV (butterfly), §V-C (linkage values) | https://arxiv.org/abs/1802.05323 | secondary (design paper by the SCMS designers) |
| S8 | Boldyreva, Kumar, Sun, "Provable Security Analysis of Butterfly Key Mechanism Protocol in IEEE 1609.2.1 Standard", PoPETs 2024(4):565–582 | https://petsymposium.org/popets/2024/popets-2024-0130.pdf | secondary (peer-reviewed analysis) |
| S9 | ETSI TS 102 941 V2.2.1 (2022-11), §6.2.3.5 (butterfly AT provisioning) and Annex F (ECIES walk-through) | https://www.etsi.org/deliver/etsi_ts/102900_102999/102941/02.02.01_60/ts_102941v020201p.pdf | primary (EU, shared mechanism) |
| S10 | IEEE SA pages: 1609.2-2022 superseded by **1609.2-2025** (approved 2025-12-10, published 2026-10-05); 1609.2.1-2022 superseded by **1609.2.1-2026** (approved 2026-02-12, published 2026-07-03); 1609.2.2-2025 (guide, multi-jurisdiction trust, published 2026-08-26) | https://standards.ieee.org/ieee/1609.2/12070/ , https://standards.ieee.org/ieee/1609.2.1/11855/ , https://standards.ieee.org/ieee/1609.2.2/11829/ | primary |
| S11 | OpenSCMS (LG Electronics, Apache-2.0, Rust), "open source implementation of an IEEE 1609.2.1-2022 SCMS" | https://github.com/OpenSCMS/OpenSCMS (files `ra/src/core_logic/expansion_function.rs`, `la/src/core_logic/gen_*.rs`) | implementation (not a conformance reference; see §13) |
| S12 | Vanetza `security/v3` (ETSI TS 103 097 v2 = 1609.2 certificate format), `hash.cpp` | https://github.com/riebl/vanetza/tree/master/vanetza/security/v3 | implementation |

The full IEEE 1609.2-2022/-2025 and 1609.2.1-2022/-2026 body text is paywalled and I did not read it.
Anything below that depends on body text I could not see is marked UNCONFIRMED.

---

## 1. Which edition to clone (currency)

* **1609.2-2025 is the active edition** (S10). Its ASN.1 (S3) differs from 2022 in three places only:
  `Ieee1609Dot2` minor 6 → 7, BaseTypes minor 4 → 5; the 2022 `ToBeSignedCertificate` extension
  fields `appExtensions`, `certIssueExtensions`, `certRequestExtension` become `OPTIONAL` (in 2022
  they were mandatory once the extension marker was used, an apparent erratum); `Countersignature`
  additionally requires `omitted ABSENT`. I diffed the code lines of both module sets.
* **1609.2.1-2026 is the active edition** (S10). Its modules are mostly version bumps (major 3 → 4);
  the CRL base types gain `iPeriodInfo` (see §11.4).
* The data structures and algorithms below are therefore the same for 2022 and 2025/2026 except where
  noted. All on-the-wire versions: `Ieee1609Dot2Data.protocolVersion = 3`, `CertificateBase.version = 3`,
  `CrlContents.version = 1`, 1609.2.1 PDUs `version = 2` (S1, S2).

---

## 2. Primitive set (what "the cryptography" is)

| Function | Exact algorithm | Source |
|---|---|---|
| Signature | ECDSA over NIST P-256 (FIPS 186-4); optional brainpoolP256r1, brainpoolP384r1, NIST P-384; SM2 in 2022 extension | S1 `Signature`; S4 §5.3.1 |
| Hash | SHA-256 (root), SHA-384 (extension), SM3 (extension) | S1 `HashAlgorithm`; S4 §5.3.3 |
| Implicit certificates | ECQV per SEC 4 with 1609.2 changes (§6) over P-256 and brainpoolP256r1 only | S4 §5.3.2 |
| Public-key encryption | ECIES per IEEE 1363a, P-256 or brainpoolP256r1, used only to wrap a 16-byte AES key (§8) | S4 §5.3.5 |
| KDF | KDF2 (IEEE 1363a §13.2) with SHA-256 = ANSI X9.63 KDF | S4 §5.3.5; S6 `kdf.py` |
| MAC | MAC1 = HMAC-SHA-256, 256-bit key, tag truncated to 128 bits | S4 §5.3.5, D.6.3 |
| Symmetric | AES-128-CCM, 12-byte nonce, 16-byte tag, no AAD; SM4-CCM in 2022 extension | S1 `One28BitCcmCiphertext`; S4 D.6.1 |
| Butterfly expansion | AES-128 in Davies–Meyer mode, 3 blocks, reduced mod n (§9) | S2 §9.3.5 refs; S7 §IV; S6 |
| Linkage | SHA-256 seed chain + AES-128 Davies–Meyer PLV truncated to 9 bytes (§10) | S1 §5.1.3.4 refs; S6; S7 §V-C |
| Encoding | Canonical OER (COER, ITU-T X.696) of the ASN.1 above, with 1609.2 canonicalisation (§3) | S1, S4 §6.1 |

---

## 3. Encoding: COER and canonicalisation

* All modules are `DEFINITIONS AUTOMATIC TAGS`; every structure that is hashed or signed is encoded
  with **COER** (Canonical Octet Encoding Rules, ITU-T X.696). S4 §5.3.2(a) and S1 throughout.
* **Canonicalisation before hashing** (S1 notes on `Certificate`, `ToBeSignedCertificate`,
  `VerificationKeyIndicator`, `HeaderInfo`, `EcdsaP256Signature`; S4 §6.1):
  * every `EccP256CurvePoint` in `verifyKeyIndicator`, in a certificate `encryptionKey`, and in
    `HeaderInfo.encryptionKey` is encoded **compressed** (`compressed-y-0` / `compressed-y-1`);
  * in `EcdsaP256Signature.rSig`, the canonical form is **`x-only`** (the signer may transmit the full R
    point for fast verification; it is converted to x-only for hashing; x mod p vs mod n difference is
    negligible, 2^-128, S1 note);
  * canonicalised structures: `Certificate`/`CertificateBase`/`ToBeSignedCertificate`, `SignedData`,
    `ToBeSignedData`, `HeaderInfo`, `SignedDataPayload`, `Ieee1609Dot2Data` (S4 §6.1).
* Fixed-size integers (`Uint8/16/32/64`) are fixed-width big-endian in OER; `Psid ::= INTEGER (0..MAX)`
  is length-prefixed. (General OER rule, X.696; not re-read here.)
* Engine note: bit-exactness of every hash below depends on a correct COER encoder. Cross-check
  against an independent OER codec (asn1c fork with OER, Bouncy Castle `org.bouncycastle.oer.its`,
  `asn1tools` with the `oer` codec, fillabs `ieee1609dot2js`) using the S1/S3 modules unchanged.

---

## 4. Base types that matter for bit-exactness (S1 `Ieee1609Dot2BaseTypes`)

| Type | Definition |
|---|---|
| `HashedId8` | **low-order 8 bytes** (= last 8 bytes) of the hash of the COER encoding (canonicalised if applicable). Example in the standard: `SHA-256("") = e3b0c442…7852b855` → HashedId8 `a495991b7852b855`. |
| `HashedId3` | low-order 3 bytes (→ `52b855` for the empty string, derived). |
| `HashedId10` | low-order 10 bytes (`934ca495991b7852b855` for the empty string). Used for hash-based CRL entries. |
| `HashedId32/48` | full SHA-256 / SHA-384 (2022). |
| Whole-certificate hash | the hash used to compute HashedIdN of a certificate; SHA-256 for P-256 chains, SHA-384 when the issuer uses `sha384AndDigest` (1609.2 §5.3.9.2 / §6.4.3; rule text UNCONFIRMED beyond the S1 notes). |
| `Time32` | Uint32 **TAI seconds since 2004-01-01 00:00:00 UTC**. Derived: Time32 = POSIX-UTC seconds − 1072915200 + leap seconds inserted since 2004 (5 so far: end-2005, end-2008, mid-2012, mid-2015, end-2016). |
| `Time64` | Uint64 TAI **microseconds** since the same epoch (`HeaderInfo.generationTime`). |
| `ValidityPeriod` | `{start Time32, duration Duration}`; expiry = start + duration (used as `HashBasedRevocationInfo.expiry`). |
| `Duration` | CHOICE of Uint16 `microseconds | milliseconds | seconds | minutes | hours | sixtyHours | years`; **1 year = 31 556 952 s**. |
| `GeographicRegion` | `circularRegion {center TwoDLocation, radius Uint16 metres}` · `rectangularRegion SEQUENCE OF {northWest, southEast}` · `polygonalRegion (≥3 points)` · `identifiedRegion SEQUENCE OF {countryOnly UnCountryId | countryAndRegions | countryAndSubregions}` (UN M.49 numeric; USA = 840, derived). Latitude/longitude are integers in 1/10 micro-degree (range ±900 000 000 / ±1 800 000 000; `unknown` = +1); elevation Uint16 decimetres above −409.5 m. |
| `PsidSsp` | `{psid Psid, ssp ServiceSpecificPermissions OPTIONAL}`; SSP is `opaque OCTET STRING` or (ext.) `bitmapSsp OCTET STRING (0..31)`. |
| `PsidSspRange` | `{psid, sspRange OPTIONAL}`, `SspRange ::= opaque SEQUENCE OF OCTET STRING | all NULL | bitmapSspRange {sspValue, sspBitmask}`. |
| `SubjectAssurance` | 1 byte: bits 7–5 assurance level, 4–2 reserved, 1–0 confidence. |
| `CrlSeries` | Uint16. `IValue` Uint16. `LaId` 2 bytes. `LinkageSeed` 16 bytes. `LinkageValue` 9 bytes. `GroupLinkageValue {jValue 4 bytes, value 9 bytes}`. |
| `EcdsaP256Signature` | `{rSig EccP256CurvePoint, sSig OCTET STRING(32)}`. |
| `EciesP256EncryptedKey` | `{v EccP256CurvePoint, c OCTET STRING(16), t OCTET STRING(16)}`. |
| `PublicEncryptionKey` | `{supportedSymmAlg SymmAlgorithm (aes128Ccm), publicKey BasePublicEncryptionKey (eciesNistP256 point…)}`. |
| `SymmetricEncryptionKey` | `aes128Ccm OCTET STRING(16)`. |

PSIDs fixed by the standards: CRL = **256 (0x100)** (`CrlPsid`, S1 `Ieee1609Dot2Crl`);
SCMS management messages = **35 (0x23)** (`SecurityMgmtPsid`, S2 Protocol). Application PSIDs
(e.g. BSM 0x20) come from the PSID registry in IEEE 1609.12, not checked here.

---

## 5. Certificate structure (S1 `Ieee1609Dot2`, 2022 §6.4)

```
Certificate ::= CertificateBase (ImplicitCertificate | ExplicitCertificate)
CertificateBase ::= SEQUENCE { version Uint8(3), type CertificateType {explicit, implicit, ...},
                               issuer IssuerIdentifier, toBeSigned ToBeSignedCertificate,
                               signature Signature OPTIONAL }
ImplicitCertificate: type=implicit, verifyKeyIndicator=reconstructionValue, signature ABSENT
ExplicitCertificate: type=explicit, verifyKeyIndicator=verificationKey, signature PRESENT
IssuerIdentifier ::= CHOICE { sha256AndDigest HashedId8, self HashAlgorithm, ...,
                              sha384AndDigest HashedId8, sm3AndDigest HashedId8 }
```

`ToBeSignedCertificate` (field order is the encoding order):

| # | Field | Type | Meaning / rule |
|---|---|---|---|
| 1 | `id` | `CertificateId ::= CHOICE {linkageData LinkageData, name Hostname, binaryId OCTET STRING(1..64), none NULL}` | pseudonym certs use `linkageData {iCert IValue, linkage-value LinkageValue(9), group-linkage-value GroupLinkageValue OPTIONAL}`; CA certs use `name`; OBU enrolment certs typically `none` (profile, see doc 03). |
| 2 | `cracaId` | `HashedId3` | low 3 bytes of the whole-certificate hash of the **CRACA's certificate** (the CA whose CRL signer may revoke this cert). |
| 3 | `crlSeries` | Uint16 | CRL series on which the cert may appear. |
| 4 | `validityPeriod` | `{start Time32, duration}` | |
| 5 | `region` | OPTIONAL | absent ⇒ same as issuer; absent in a self-signed root ⇒ worldwide. |
| 6 | `assuranceLevel` | OPTIONAL `SubjectAssurance` | |
| 7 | `appPermissions` | OPTIONAL `SEQUENCE OF PsidSsp` | what the holder may sign; each PSID at most once; verifiers must support ≥ 8 entries. |
| 8 | `certIssuePermissions` | OPTIONAL `SEQUENCE OF PsidGroupPermissions` | what a CA may issue; at most one `all` entry, which covers PSIDs not listed elsewhere. |
| 9 | `certRequestPermissions` | OPTIONAL, same type | what an enrolment cert holder may request (enrolment certs carry this, not appPermissions). |
| 10 | `canRequestRollover` | OPTIONAL NULL | reserved, no semantics. |
| 11 | `encryptionKey` | OPTIONAL `PublicEncryptionKey` | point compressed when hashed. |
| 12 | `verifyKeyIndicator` | `CHOICE {verificationKey PublicVerificationKey, reconstructionValue EccP256CurvePoint}` | explicit key vs ECQV reconstruction value P_U; compressed when hashed. |
| ext | `flags` | `BIT STRING {usesCubk(0)} (SIZE(8))` OPTIONAL | CA-only: holder supports the compact unified butterfly response (§9.4). |
| ext | `appExtensions`, `certIssueExtensions`, `certRequestExtension` | sequences of extensions (e.g. `OperatingOrganizationId`) | mandatory-if-extension-present in 2022, OPTIONAL in 2025. |

Constraint: at least one of `appPermissions`, `certIssuePermissions`, `certRequestPermissions` present.

`PsidGroupPermissions ::= {subjectPermissions CHOICE {explicit SEQUENCE OF PsidSspRange, all NULL},
minChainLength INTEGER DEFAULT 1, chainLengthRange INTEGER DEFAULT 0, eeType BIT STRING {app(0),
enrol(1)} DEFAULT {app}}`. `minChainLength = 0` in `certIssuePermissions` makes the cert invalid;
`chainLengthRange = −1` means unbounded. The permitted chain length below the CA is
`[minChainLength, minChainLength + chainLengthRange]` counting down to and including the EE cert.

**Explicit certificate signature** (S1 note on `CertificateBase.signature`; S4 §5.3.1): ECDSA over
`Hash(COER(toBeSigned)) || Hash(signer-id input)` where the signer-id input is the canonical COER of
the issuer certificate, or the empty string if `issuer = self`. See §7 for the exact digest.

---

## 6. Implicit certificates: ECQV exactly as profiled by 1609.2

SEC 4 (S5) equations, with 1609.2 deviations (S4 §5.3.2 a–d; repeated in S1 `ToBeSignedCertificate` note):

| Step | SEC 4 | 1609.2 profile |
|---|---|---|
| Request (§3.3) | requester picks k_U, sends R_U = k_U·G | in SCMS butterfly, R_U is the **cocoon signing key** B (§9) |
| Generate (§3.4) | CA picks k; P_U = R_U + k·G; Cert_U = encode(…, P_U); e = Hn(Cert_U); if e·P_U + Q_CA = O retry; r = e·k + d_CA mod n | Cert_U = COER `ImplicitCertificate` with `reconstructionValue = P_U` (compressed). **Hash input is not Cert_U**: it is `H(COER canonical ToBeSignedCertificate) || H(COER canonical issuer Certificate)`. |
| Hn (§2.3) | SEC 4 truncates H(S) to the leftmost ⌊log2 n⌋ = 255 bits | **No truncation**: e = OS2I(SHA-256(input)), the full 256-bit value (S4 §5.3.2 d). Reduce mod n in the arithmetic. |
| Extract (§3.5) | Q_U = e·P_U + Q_CA | same e |
| Reception (§3.6) | d_U = r + e·k_U mod n; check d_U·G = Q_U | private key validity judged per SEC 4 §3.6 (S4) |
| Self-signed (§3.7–3.8) | CA key = (0, O) | 1609.2 roots are explicit certificates in practice (profile, UNCONFIRMED here) |

So, for the engine:

```
e   = int.from_bytes(SHA256( SHA256(COER(tbsCert_canonical)) || SHA256(COER(issuerCert_canonical)) ), 'big')
P_U = B + k·G                      (ACA, k random)
r   = (e·k + d_ACA) mod n          (ACA → EE as privateKeyInfo, 32 bytes)
d_U = (e·b + r) mod n              (EE; b = cocoon private key)
Q_U = e·P_U + Q_ACA                (any verifier)
```

Two implementations that ignore deviation (b) or (d) interoperate with each other and with nobody
else. **Warning:** the CAMP `implicit.py` generator (S6) uses SEC 4 truncation (`e = SHA256 >> 1`) and
`H(tbsCert || P_U)` "for illustration purposes" (its own docstring says so, and notes the 1609.2 v3
form was confirmed by W. Whyte on 2015-10-29). Its vectors test the SEC 4 algebra, **not** the
1609.2 profile.

---

## 7. Signed SPDU: what exactly is hashed and signed

Structures (S1):

```
Ieee1609Dot2Data ::= { protocolVersion Uint8(3), content CHOICE { unsecuredData Opaque,
     signedData SignedData, encryptedData EncryptedData, signedCertificateRequest Opaque, ...,
     signedX509CertificateRequest Opaque } }
SignedData ::= { hashId HashAlgorithm, tbsData ToBeSignedData, signer SignerIdentifier, signature Signature }
ToBeSignedData ::= { payload SignedDataPayload, headerInfo HeaderInfo }
SignedDataPayload ::= { data Ieee1609Dot2Data OPTIONAL, extDataHash HashedData OPTIONAL, ..., omitted NULL OPTIONAL }
HeaderInfo ::= { psid, generationTime Time64 OPT, expiryTime Time64 OPT, generationLocation ThreeDLocation OPT,
     p2pcdLearningRequest HashedId3 OPT, missingCrlIdentifier OPT, encryptionKey EncryptionKey OPT, ...,
     inlineP2pcdRequest SEQUENCE OF HashedId3 OPT, requestedCertificate Certificate OPT,
     pduFunctionalType OPT, contributedExtensions OPT }
SignerIdentifier ::= CHOICE { digest HashedId8, certificate SEQUENCE OF Certificate, self NULL, ... }
```

**Digest construction** (1609.2 §5.3.1, via S1 `SignedData` note and S4 §5.3.1 c)2)):

```
data_input   = COER(canonical tbsData)                       -- plus || H(omitted payload) if payload.omitted is present
signer_input = COER(canonical signing certificate)  if signer is digest or certificate
             = ""                                    if signer is self
M      = H(data_input) || H(signer_input)               -- 64 bytes for SHA-256
digest = H(M)                                           -- the value ECDSA actually signs
```

* The signing certificate is the **end-entity (authorisation) certificate** even when `signer` carries a
  chain (`certificate` = chain, EE first, each next is the issuer of the previous); `digest` carries
  its HashedId8.
* For self-signed SPDUs `H("") = e3b0c442…b855` is used as the second half.
* Vanetza's `calculate_message_hash` implements exactly `H(H(payload) || H(cert))` (S12), an
  independent cross-check.
* ECDSA output: `rSig` as `x-only` (FIPS 186-4 r) or as compressed/uncompressed R (SEC 1 "fast
  verification"); canonical form for any hashing is x-only (S1).
* Header rules worth implementing: if both generation and expiry time present, generation < expiry;
  `inlineP2pcdRequest` and `p2pcdLearningRequest` mutually exclusive (S1 `HeaderInfo`).
* `pduFunctionalType`: 1 tlsHandshake, 2 iso21177ExtendedAuth, 3 iso21177SessionExtension (S1).

---

## 8. Encryption: ECIES + AES-128-CCM

### 8.1 Structures (S1)

```
EncryptedData ::= { recipients SEQUENCE OF RecipientInfo, ciphertext SymmetricCiphertext }
RecipientInfo ::= CHOICE { pskRecipInfo HashedId8, symmRecipInfo SymmRecipientInfo,
                           certRecipInfo PKRecipientInfo, signedDataRecipInfo PKRecipientInfo,
                           rekRecipInfo PKRecipientInfo }
SymmRecipientInfo ::= { recipientId HashedId8, encKey SymmetricCiphertext }
PKRecipientInfo   ::= { recipientId HashedId8, encKey EncryptedDataEncryptionKey }
EncryptedDataEncryptionKey ::= CHOICE { eciesNistP256 EciesP256EncryptedKey, eciesBrainpoolP256r1 …, ..., ecencSm2256 … }
SymmetricCiphertext ::= CHOICE { aes128ccm One28BitCcmCiphertext, ..., sm4Ccm … }
One28BitCcmCiphertext ::= { nonce OCTET STRING(12), ccmCiphertext Opaque }   -- ciphertext = plaintext + 16
```

| RecipientInfo | recipientId | ECIES P1 |
|---|---|---|
| `certRecipInfo` | HashedId8 of the recipient certificate (whole-cert hash, canonical COER) | SHA-256(COER canonical certificate) |
| `signedDataRecipInfo` | HashedId8 (SHA-256) of the `Ieee1609Dot2Data` that carried `headerInfo.encryptionKey`, canonicalised per §6.3.4 | SHA-256(that COER `Ieee1609Dot2Data`) |
| `rekRecipInfo` | HashedId8 of COER `PublicEncryptionKey` | SHA-256("") — only for responses to anonymous requests (S4 note) |
| `symmRecipInfo` | low 8 bytes of SHA-256(COER `SymmetricEncryptionKey`) | n/a (AES-CCM key wrap) |
| `pskRecipInfo` | same HashedId8 of the pre-shared `SymmetricEncryptionKey` | n/a (data encrypted directly with the PSK) |

Recipients: verifiers must support ≥ 8 RecipientInfo; all must convey the same data key (S1).

### 8.2 ECIES exactly (S4 §5.3.5) — VERIFIED against all four S4 D.6.2 vectors

```
v  random in [1, n-1];  V = v·G                       (fresh for every encryption; sent as EciesP256EncryptedKey.v)
Z  = x-coordinate of v·R_recipient (32 bytes)         (ECSVDP-DHC; cofactor 1 on P-256; non-DHAES mode: V is NOT hashed)
K  = KDF2(Z, 48, P1) = SHA256(Z || 00000001 || P1) || SHA256(Z || 00000002 || P1)   truncated to 48 bytes
K1 = K[0:16]   (XOR "stream cipher" key)   K2 = K[16:48]  (MAC key, 256 bits)
c  = k_AES XOR K1                                     (16 bytes)
t  = leftmost 16 bytes of HMAC-SHA256(K2, c)           (MAC1, tBits=128, P2 = empty)
```

Decryption recomputes Z = x(r·V), checks t in constant time, then k = c XOR K1. ETSI TS 102 941
Annex F (S9) describes the same scheme (it omits P1 in its prose).

**Published-vector erratum (important for validation):** in S4 Annex D.6.2 and in the CAMP file
`ecies.txt` (S6), the printed `P1` of every vector equals the AES key `k` (16 bytes). That is a print
bug in the generator (`ecies.py` prints `k` on the `P1` line). The real P1 values are in `ecies.py`:
vectors 1–2 `P1 = A6B7B52554B4203F7E3ACFDB3A3ED8674EE086CE5906A7CAC2F8A398306D3BE9`, vectors 3–4
`P1 = 05BED5F867B89F30FE5552DF414B65B9DD4073FC385D14921C641A145AA12051`. With these, C and T of all
four vectors reproduce exactly; with the printed P1 none do. Whether 1609.2-2022/-2025 Annex D fixed
the print is UNCONFIRMED.

### 8.3 AES-128-CCM (S4 §5.3.8, D.6.1) — vector 1 VERIFIED

NIST SP 800-38C / RFC 3610 with no associated data, tag 16 bytes, nonce 12 bytes (so q = 3, max
message 2^24−1 bytes). Plaintext = COER `Ieee1609Dot2Data` (or a 16-byte key inside `symmRecipInfo`).
Nonce generation rule (random vs. structured, uniqueness per key) is in 1609.2 §5.3.8 body text:
UNCONFIRMED; S9 only says never reuse a nonce under one key.

### 8.4 Symmetric key reuse (S1)

A sender can put `EncryptionKey.symmetric` (an AES key) into `HeaderInfo.encryptionKey` of a signed SPDU
that is itself encrypted; the peer then replies using `pskRecipInfo` or `symmRecipInfo` with the
HashedId8 of that `SymmetricEncryptionKey` instead of a new ECIES operation. `symmetric` must only be
used when the carrying SignedData is itself securely encrypted. Guidance on choosing the approach:
1609.2 Annex C.7 (not read).

---

## 9. Butterfly keys (IEEE 1609.2.1 §9.3)

### 9.1 Wire formats (S2 `EeRaInterface`, `AcaRaInterface`, `AcaEeInterface`, `Protocol`)

```
EeRaCertRequest ::= { version Uint8(2), generationTime Time32, type CertificateType,
    tbsCert ToBeSignedCertificate (cracaId '000000'H, crlSeries 0, appPermissions PRESENT,
            certIssue/RequestPermissions ABSENT, verifyKeyIndicator = verificationKey),   -- = caterpillar signing key A
    additionalParams AdditionalParams OPTIONAL, ... }
AdditionalParams ::= CHOICE { original ButterflyParamsOriginal, unified ButterflyExpansion,
                              compactUnified ButterflyExpansion, encryptionKey PublicEncryptionKey, ... }
ButterflyParamsOriginal ::= { signingExpansion ButterflyExpansion, encryptionKey PublicEncryptionKey,   -- caterpillar enc key H
                              encryptionExpansion ButterflyExpansion }
ButterflyExpansion ::= CHOICE { aes128 OCTET STRING(16), ... }                     -- the expansion key ck / ek
RaAcaCertRequest ::= { version Uint8(2), generationTime Time32, type, flags BIT STRING {butterflyExplicit(0), cubk(1)},
    linkageInfo { encPlv1, encPlv2 } OPTIONAL, certEncKey PublicEncryptionKey OPTIONAL,
    tbsCert (cracaId '000000'H, crlSeries 0 — ACA replaces them) }
AcaRaCertResponse ::= { version 2, generationTime, requestHash HashedId8, acaResponse CHOICE {
    plain AcaEeCertResponsePlainSpdu, private AcaEeCertResponsePrivateSpdu, cubk AcaEeCertResponseCubkSpdu } }
AcaEeCertResponse ::= { version 2, generationTime Time32, certificate Certificate, privateKeyInfo OCTET STRING(32) OPTIONAL }
RaEeCertAck ::= { version 2, generationTime, requestHash HashedId8, firstI IValue OPTIONAL, nextDlTime Time32 }
```

### 9.2 Expansion function — VERIFIED against CAMP `bfkeyexp.txt` and OpenSCMS

S7 §IV eq. (1)–(2), S6 `bfkeyexp.py`, S11 `expansion_function.rs` all agree:

```
x_sign = 0x00000000 || i(32) || j(32) || 0x00000000          -- 128-bit block, i and j big-endian 32-bit
x_enc  = 0xFFFFFFFF || i(32) || j(32) || 0x00000000
f_k^int(x) = (AES_k(x+1) XOR (x+1)) || (AES_k(x+2) XOR (x+2)) || (AES_k(x+3) XOR (x+3))   -- 384 bits; +1..+3 mod 2^128
f_k(x) = f_k^int(x) mod n                                     -- big-endian integer
cocoon signing key:     B_ij = A + f_ck(x_sign)·G,   b_ij = a + f_ck(x_sign) mod n
cocoon encryption key:  J_ij = H + f_ek(x_enc)·G,    h_ij = h + f_ek(x_enc) mod n     (original variant)
```

Checks I ran: CAMP vector (`ck = 121D…3B1B`, i = 0x217D79E1, j = 0x11) gives
`f = 79081DDD…58E12D5E`; enc vector gives `A1559B33…DD3BA317`; OpenSCMS's encryption vector
(`i = 7590, j = 0`) gives `e30a331d…bd8de68` — all reproduced. The CAMP vector uses j in [1, 20]
"padded to 32 bits"; i is the 32-bit time-period index (the ASN.1 `IValue` is Uint16, the expansion
input field is 32 bits). A secondary paper (arXiv 2507.06244, in Chinese) describes the encryption
prefix as four `0x11` bytes; that conflicts with all three sources above and is wrong.

### 9.3 What the ACA does with a cocoon key

* **Explicit certificate** (`flags.butterflyExplicit`): `tbsCert.verifyKeyIndicator.verificationKey`
  holds B. ACA picks random c, certifies `U = B + c·G`, returns `privateKeyInfo = c` ("private key
  randomization value"). EE: `u = b + c mod n` (S2 `AcaEeCertResponse`; S7 §IV).
* **Implicit certificate**: B plays R_U in ECQV (§6): `P_U = B + k·G`, `privateKeyInfo = r`
  ("private key reconstruction value"); EE: `d_U = e·b + r`.
* S8 (Fig. 3) notes the standard uses the random offset directly (h_i = r_i) and proves security only
  "with small modifications", proposing h_i = H(r_i || pk_cc[i]); it also proposes an efficiency
  improvement. Not adopted by 1609.2.1-2026 as far as the ASN.1 shows (UNCONFIRMED).

### 9.4 The three variants and the response encryption (S2 `Protocol` notes)

| Variant | EE uploads | Response to EE | Encryption key used by ACA | ACA signs response? |
|---|---|---|---|---|
| original | A, ck, H, ek | `AcaEeCertResponsePrivateSpdu` (Ieee1609Dot2Data-EncryptedSigned) | cocoon encryption key J derived from (H, ek) per §9.3.4.2; EE decrypts with h_ij per §9.3.4.1 | yes, with the **same ACA cert that issued the inner cert** (new validity rule) |
| unified | A, ck | `AcaEeCertResponsePrivateSpdu` | cocoon encryption key "derived from tbsCert.verifyKeyIndicator and additionalParams.unified as specified in 9.3.4.2"; "the caterpillar public key and expansion function for encryption are the same as those for signing" | yes |
| compact unified (CUBK) | A, ck | `AcaEeCertResponseCubkSpdu` (Ieee1609Dot2Data-Encrypted, **not signed**) | the ACA derives the encryption key from the cocoon signing key B_t it received (§9.3.4.2); requires `RaAcaCertRequest.flags.cubk`, `certEncKey` absent, and the **ACA certificate's `flags.usesCubk`** set | no |
| none | `encryptionKey` | Private SPDU encrypted to that key | given key | yes |
| plain | — | `AcaEeCertResponsePlainSpdu` (unsecured) | — | no (used when privacy from RA is not needed) |

The exact §9.3.4.2 formula for the unified/CUBK encryption key is UNCONFIRMED. Most likely (from the
ASN.1 wording and S7/Simplicio et al. "Unified Butterfly Effect", IACR ePrint 2018/089, which I could
not download): unified uses `A + f_ck(x_enc)·G` (same ck, encryption-domain prefix) and CUBK encrypts
directly to `B_t`. Settle this from the 1609.2.1 text before building.

### 9.5 Who learns what

* **RA**: the EE's identity (enrolment-signed request), A, ck (and H, ek), every cocoon key and its
  (i, j); it stores j because "the value of j is not communicated to the ACA" and encodes it in the
  download filename (S2 notes). It sees responses only encrypted to the EE, and cannot map B to the
  final certified key because of the ACA's secret c or k.
* **ACA**: shuffled cocoon keys, tbsCert contents, iCert, decrypted PLVs and hence the linkage value;
  not the EE identity, not j, not ck.
* S8 §4.1: no privacy if RA and ACA collude; a malicious RA can deanonymise by splitting one EE's
  requests across different ACAs; privacy holds per batch if one ACA serves the batch.
* Derived from S7 §V-C: the ACA together with one LA could link a device's certificates (the LA knows
  which PLVs form one chain; the ACA sees the decrypted PLV in each certificate). The design assumes
  the ACA does not collude with an LA. Treat this as a derived reading; it is not stated verbatim.

---

## 10. Linkage values

### 10.1 Seed chain, PLV, LV — VERIFIED against CAMP `lv.txt`

S6 `lv.py` (the CAMP reference), consistent with S7 §V-C and S1 field sizes:

```
la_id  : 16 bits (LaId, 2 bytes)            ls_x(0): 16 random bytes per device per LA
ls_x(i) = first 16 bytes of SHA256( la_id_x || ls_x(i-1) || 0^112 )      -- 32-byte input, zero padded
plv_x(i,j) = first 9 bytes of ( AES_{ls_x(i)}( la_id_x || j(32) || 0^80 ) XOR ( la_id_x || j || 0^80 ) )
lv(i,j)  = plv_1(i,j) XOR plv_2(i,j)                                      -- 9 bytes, computed only by the ACA
```

CAMP vector: la_id1 = 0x8E09, ls1(0) = 4840ED22…FBA9F210, j = 8 → plv1(0,8) = 61F391B5D930E103ED;
la_id2 = 0x59CE → lv(0,8) = 688ACF70E25EC01F32. OpenSCMS's comment cites "IEEE Std 1609.2-2022 –
D.7 Test vectors for Linkage Values lv(i,j)", the same title as the CAMP file, so 1609.2-2022 Annex D.7
very probably carries these vectors (UNCONFIRMED). S7 says "32-bit la_id"; the ASN.1 `LaId` and the
CAMP vectors use 16 bits — follow the 16-bit form. 1609.2-2022 adds placeholders
`SeedEvolutionFunctionIdentifier` and `LvGenerationFunctionIdentifier` (both NULL) and
`ExpansionAlgorithmIdentifier {sha256ForI-aesForJ, sm3ForI-sm4ForJ}` (S1 CRL base types), so the
functions above are the default "SHA-256 for i, AES for j" pair.

**OpenSCMS diverges** (S11): little-endian la_id, 8-byte la_id field, no 0^112 padding, 16-byte PLV
without truncation. It is not interoperable on linkage and must not be used as a reference.

### 10.2 Group linkage (S6 `glv.py`; S1)

Group seeds gs_x(i) evolve and expand with the **same** `ls`/`plv` functions; j is a 32-bit group
value; `glv(i,j) = gplv_1 XOR gplv_2`. CAMP also defines an encrypted index
`ei(j,k) = AES_ik(input) XOR input` with input = 32-bit || 32-bit || 0^64 under a 128-bit key ik.
The certificate carries `group-linkage-value {jValue (4 bytes), value (9 bytes)}`. How jValue relates
to ei (truncation, which half) is UNCONFIRMED. CRL entries: `GroupCrlEntry {iMax, la1Id, linkageSeed1,
la2Id, linkageSeed2}` and (ext.) `GroupSingleSeedCrlEntry {iMax, laId, linkageSeed}`.

### 10.3 PLV transport to the ACA

* 1609.2.1-2022/-2026 (S2 `AcaRaInterface`): `LinkageInfo {encPlv1, encPlv2}`, each
  `EncryptedIndividualPLV {version 2, laId, encPlv Ieee1609Dot2Data-SymmEncryptedSingleRecipient{PreLinkageValue(9)}}`
  — **AES-128-CCM under a key shared between that LA and the ACA**, one `symmRecipInfo` recipient whose
  id is the HashedId8 of the shared key. How the key is shared is out of scope of the standard.
* CAMP PoC description (S7): PLVs "encrypted (individually) for the PCA but sent to the RA". The
  standard therefore settled on symmetric encryption; there is no "PCA encrypts the PLV to itself".
* **Linkage chain identifier (LCI)** (S7 §V-C): the LA encrypts its initial seed to itself,
  e.g. `LCI = E(pk_LA, ls(0))`; the RA keeps LCIs per device so that the MA can later ask each LA to
  open its chain. In 1609.2.1 the LA–RA, LA–MA, MA–RA interfaces are `NULL` placeholders ("not used by
  EEs") (S3), so the LCI format, PLV request format and LA key handling are **not standardised**; the
  engine has to model them from S7 and CAMP material.

### 10.4 Activation codes (ACPC)

1609.2.1 §9.5 (ACPC, `Ieee1609Dot2Dot1Acpc`, `RaEeCertInfo.acpcTreeId`) lets certificates be
pre-issued and encrypted, then unlocked per period by activation codes. Details are out of this file's
scope; formats are in S2/S3.

---

## 11. CRLs (S1 `Ieee1609Dot2Crl`, `Ieee1609Dot2CrlBaseTypes`, `Ieee1609Dot2CrlSsp`)

### 11.1 Envelope and signer

`SecuredCrl` = `Ieee1609Dot2Data` → `signedData`, payload `unsecuredData` CONTAINING `CrlContents`,
`headerInfo.psid = 256`, with `generationTime`, `expiryTime`, `generationLocation`,
`p2pcdLearningRequest`, `missingCrlIdentifier`, `encryptionKey` all **ABSENT**. The signature follows §7.
The signer's certificate carries PSID 256 with `CrlSsp {version 1, associatedCraca CracaType {isCraca,
issuerIsCraca}, crls SEQUENCE OF CrlSeries}`: `isCraca` ⇒ the signer is itself the CRACA;
`issuerIsCraca` ⇒ its issuer is. `crlCraca` must match accordingly and `crlSeries` must be listed in
`crls` (§7.4 rules via S1 notes). The 1609.2.1 `CrlSignerSsp` is separate and only for PSID 35.

### 11.2 Contents

```
CrlContents ::= { version Uint8(1), crlSeries CrlSeries, crlCraca HashedId8,       -- full whole-cert hash of the CRACA cert, low 8 bytes
                  issueDate Time32, nextCrl Time32 (must be > issueDate), priorityInfo {priority Uint8 OPT},
                  typeSpecific CHOICE { fullHashCrl, deltaHashCrl  ToBeSignedHashIdCrl,
                                        fullLinkedCrl, deltaLinkedCrl ToBeSignedLinkageValueCrl, ...,
                                        fullLinkedCrlWithAlg, deltaLinkedCrlWithAlg ToBeSignedLinkageValueCrlWithAlgIdentifier } }
ToBeSignedHashIdCrl ::= { crlSerial Uint32, entries SEQUENCE OF { id HashedId10, expiry Time32 } }
ToBeSignedLinkageValueCrl ::= { iRev IValue, indexWithinI Uint8,
        individual SEQUENCE OF JMaxGroup OPT, groups SEQUENCE OF GroupCrlEntry OPT, ..., groupsSingleSeed OPT,
        iPeriodInfo SEQUENCE OF IPeriodInfo OPT  -- 2025/2026 only }
JMaxGroup ::= { jmax Uint8, contents SEQUENCE OF LAGroup }
LAGroup   ::= { la1Id LaId, la2Id LaId, contents SEQUENCE OF IMaxGroup }
IMaxGroup ::= { iMax Uint16, contents SEQUENCE OF IndividualRevocation {linkageSeed1, linkageSeed2}, ..., singleSeed OPT }
IPeriodInfo ::= { startOfNextIPeriod Time32, iPeriodLength Duration, guaranteedToIValue IValue }   -- 2025/2026
```

* `indexWithinI`: 0 for the first CRL with a given (crlCraca, crlSeries, iRev), +1 per full or delta CRL.
* `HashBasedRevocationInfo.id` = HashedId10 of the revoked cert's whole-cert hash; `expiry` = start + duration.
* Device check for linkage entries (derived from S1 field semantics + §10.1): for each
  (la1Id, la2Id, ls1, ls2) at iRev, advance both seeds `iCert − iRev` steps, compute `lv(iCert, j)` for
  j = 0 … jmax−1 and compare with the certificate's `linkage-value`; stop tracking once `iCert > iMax`.
  The certificate is matched only if `cracaId`/`crlSeries` match the CRL's `crlCraca`/`crlSeries`
  (1609.2 §5.1.3.2, UNCONFIRMED wording).
* Revocation of a CA certificate revokes everything that chains to it and is first received after the
  CRL issue date (S4 §5.1.3.1).
* 1609.2.1 adds `CompositeCrlSpdu` (unsecured bundle of CRLs and CTLs) for distribution (S2 Protocol).

---

## 12. Chain validation rules (S4 §5.1.2.x, §5.2, §6.4.28–6.4.29a; S1 notes)

A signed SPDU is valid only if (S4 §5.2 list):

1. a chain from the signing cert to a known trust anchor can be built (§5.1.2.1), every certificate is
   well-formed, and every link verifies cryptographically (explicit: ECDSA per §5; implicit: the key
   reconstructed per §6 must verify the next object) (§5.1.2.3);
2. the chain is internally consistent (§5.1.2.4) and trustworthy (§5.1.2.4a: starts at a trust anchor,
   nothing revoked, optionally nothing expired, "strongly recommended");
3. the SPDU is consistent with the signing certificate (PSID in `appPermissions`, SSP rules, time and
   region checks per the SDEE profile) and verifies.

Consistency of a subordinate with its issuer (§5.1.2.4):

* every `appPermissions` / `certIssuePermissions` entry of the subordinate is covered by an issuer
  `certIssuePermissions` entry (explicit PSID, or the `all` entry for PSIDs not otherwise listed) whose
  `eeType` allows `app` (or `enrol` for `certRequestPermissions` chains);
* SSP rules: subordinate opaque SSP needs issuer `sspRange all` or an identical opaque entry; omitted
  SSP needs issuer `all` or a zero-length opaque entry (§6.4.28); bitmap SSP: for every bit set in the
  issuer `sspBitmask`, the subordinate has the same bitmask bit set and the same `sspValue` bit (S1
  `BitmapSspRange` note);
* chain length: with issuer (mcr_i, clr_i) and subordinate (mcr_s, clr_s), require
  `mcr_i ≤ mcr_s + 1` and `mcr_i + clr_i ≥ mcr_s + clr_s + 1`, with mcr_s = clr_s = 0 for an EE cert;
  `clr = −1` means unbounded;
* validity period inside the issuer's and region inside the issuer's (absent region inherits) — the
  inheritance rule is in S1; the "contained in" wording of §5.1.2.4 bullets was elided in S4
  ("[other bullet points omitted]"): UNCONFIRMED wording, standard practice;
* response consistency: an `AcaEeCertResponsePrivateSpdu` is invalid unless signed by the same ACA
  cert that issued the enclosed certificate (S2).

Implementation limits that are normative: support ≥ 8 entries in appPermissions,
certIssuePermissions, certRequestPermissions, SSP ranges and RecipientInfos; "Maximum Full Certificate
Chain Length 8" appears in the S4 PICS table.

---

## 13. Official and quasi-official test vector sources

| Source | Covers | Status in this session |
|---|---|---|
| **IEEE 1609.2a-2017 Annex D.6** (S4) | D.6.1 AES-CCM-128 ×6, D.6.2 ECIES ×4, D.6.3 MAC1 ×7, D.6.4 KDF2 ×4 | AES-CCM #1 VERIFIED; ECIES #1–4 VERIFIED with the real P1 (print erratum, §8.2); MAC1 = RFC 4231 truncated (#1 matches); KDF2 = ANSI X9.63 vectors (output lines did not extract from the PDF; my KDF2 gives `443024c3dae66b95e6f5670601558f71` for #1, equal to `known_key1` in CAMP `kdf.py`: VERIFIED) |
| **IEEE 1609.2-2022 / -2025 Annex D** | consolidates the above; D.7 linkage values (per S11 comment); likely more | not seen — UNCONFIRMED, buy the standard |
| **IEEE 1609.2.1-2022 / -2026 annexes** | butterfly / ACPC vectors, if any | not seen — UNCONFIRMED |
| **CAMP vectors** (S6) | `lv.txt` (i = 0, 1), `glv.txt`, `bfkeyexp.txt` (sign + enc), `implicit.txt` (SEC 4 form only), `ecies.txt` (P1 misprint), `kdf.txt`, `mac1.txt`, `aesccm.txt`, `ecdh.txt` (NIST SP 800-56A) | lv, bfkeyexp VERIFIED; ecies VERIFIED with P1 from `ecies.py`; implicit NOT the 1609.2 profile |
| SEC 4 (S5) | none — the document has no test vectors (TOC: Annexes A–C only) | confirmed |
| NIST CAVP | ECDSA P-256 SigGen/SigVer, CCM (DVPT), SHA-256, X9.63 KDF (SP 800-135) | https://csrc.nist.gov/projects/cryptographic-algorithm-validation-program — primitive level |
| RFC 4231 | HMAC-SHA-256 | https://www.rfc-editor.org/rfc/rfc4231 |
| ETSI TS 103 096 / ETSI ITS test system | conformance certificates in 1609.2 v3 (ETSI profile, explicit only) | https://forge.etsi.org/rep/ITS/TS.ITS — usable for COER/HashedId8/ECDSA cross-checks |
| Vanetza v3 (S12) | explicit-cert parsing, canonicalisation, message hash | cross-check implementation; no butterfly/implicit files seen in its tree |
| OpenSCMS (S11) | self-generated expansion vectors (reproduce §9.2) | linkage code non-conformant; not a reference |
| Engine today | `crates/v2xw-sec/tests/legacy_vectors.rs` pins outputs of `legacy/scms_sim_ref` (self-generated) | does not yet use the CAMP or IEEE vectors; no ECIES in `v2xw-sec` |

Other ASN.1 tooling for encoding cross-checks: Bouncy Castle `org.bouncycastle.oer.its.ieee1609dot2`
and `…ieee1609dot2dot1`
(https://downloads.bouncycastle.org/java/docs/bcutil-jdk18on-javadoc/org/bouncycastle/oer/its/ieee1609dot2dot1/package-summary.html),
fillabs `ieee1609dot2js` / `ieee1609dot2dot1js` (GitHub).

---

## 14. What the engine must implement to be bit-exact (build checklist)

1. **COER encoder/decoder** generated from the S3 (2025/2026) modules, with 1609.2 canonicalisation
   (compressed points, x-only rSig) applied before every hash.
2. **HashedId3/8/10** = low-order bytes of the whole-cert hash; the empty-string examples as unit tests.
3. **Time32/Time64** in TAI from 2004-01-01 with the leap-second table; `Duration` with 31 556 952 s years.
4. **ECDSA P-256** over `H(H(COER tbsData) || H(COER signer cert | ""))`; x-only and compressed-R forms.
5. **ECQV** with the 1609.2 hash input and the untruncated 256-bit e (§6); SEC 4 retry on O.
6. **ECIES** exactly as §8.2 (KDF2 = X9.63, K1 16 + K2 32, MAC1 128-bit tag, non-DHAES, P1 per
   RecipientInfo type) and **AES-128-CCM** (12-byte nonce, 16-byte tag, no AAD).
7. **Butterfly expansion** §9.2 for both domains, explicit (`+c·G`, privateKeyInfo = c) and implicit
   (ECQV with B as R_U) issuance, original/unified/CUBK response encodings, the `usesCubk` and
   same-ACA-signer checks.
8. **Linkage**: seed chain with the 0^112 pad, PLV with 0^80 pad and 9-byte truncation, LV XOR,
   group linkage, PLV transport as `EncryptedIndividualPLV` (AES-CCM, HashedId8 of the shared key).
9. **CRLs**: SecuredCrl envelope with the ABSENT header fields, CrlSsp checks, hash-based and
   linkage-based matching, iMax pruning, `iPeriodInfo` (2025/2026).
10. **Chain validation** §12 including the chain-length inequalities and SSP consistency.

Validation rules for the build (from the "checks that cannot fail" lesson): every vector test must be
shown to go red when one input is perturbed — e.g. use the printed P1 from D.6.2 and confirm ECIES
fails; swap the 0x00000000/0xFFFFFFFF prefixes and confirm the bfkeyexp test fails; drop the 0^112
pad and confirm lv fails; use SEC 4 truncation in ECQV and confirm a 1609.2-profile vector fails
(once one is obtained). There is currently **no public vector for the 1609.2-profiled ECQV hash or
for unified/CUBK responses**: the engine needs either the paid IEEE annexes or an interop capture
from a commercial SCMS before those two can be called validated.

---

## 15. Open items (UNCONFIRMED)

1. 1609.2.1 §9.3.4.1/9.3.4.2: exact derivation of the unified and CUBK response encryption key.
2. 1609.2 §5.1.3.4 body text (linkage) — confirmed only through CAMP reference code and field sizes.
3. Group linkage: relation of `jValue` (4 bytes) to CAMP's encrypted index ei(j,k).
4. Contents of 1609.2-2022/-2025 Annex D beyond D.6, and any 1609.2.1 test-vector annex.
5. AES-CCM nonce construction rule in §5.3.8.
6. Whether 1609.2-2025 corrected the D.6.2 P1 misprint.
7. The S8-proposed `h_i = H(r_i || pk_cc[i])` change: adopted in 1609.2.1-2026 or not.
8. LCI, PLV-request and LA key formats: not standardised in 1609.2.1 (NULL interfaces); need CAMP
   PoC interface documents (CAMP wiki unreachable today).
