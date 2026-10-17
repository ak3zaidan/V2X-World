# SCMS in the US today: deployment and governance

Research note, 2026-10-16. This is research only: no code changes. Its purpose is to pin down what the real US Security Credential Management System looks like *as operated*, as opposed to as designed, so the simulator can clone it.

Conventions:
- **[P]** marks a primary source: a standard, an SCMS Manager publication, a USDOT or FCC document, or a published certificate policy.
- **[V]** marks a vendor claim, such as a press release or product page.
- **[S]** marks any other secondary source, such as a law-firm summary or a search-engine digest.
- **[A]** marks my own inspection of a published artefact, such as decoding the CTL files SCMS Manager publishes.
- **UNCONFIRMED** means I could not verify the claim from a source.
- "1609.2.1" means IEEE Std 1609.2.1-2022, and "the Profile" means the SCMS Manager *IEEE 1609.2.1 Profile v1.0.3*.

---

## 0. Ten things the simulator must get right (summary)

1. **There is one trust domain with many providers.** SCMS Manager LLC is the US policy authority. It controls one CTL series, `03 48 00 00 00 00 00 00`. Its five Electors sign that CTL, and any 3 of the 5 make it valid. Each commercial SCMS Provider runs its own Root CA and full sub-CA tree, and that root sits on the CTL. *(Profile §6.1; Provider Req. §1.2.2; Elector Tech Spec §2.1, §2.4)* [P]
2. **The production CTL is small.** The 13 Aug 2025 production CTL embeds exactly **two root CAs** and **five electors** [A]:
   - `v2xrootca.ghsiss.com`, run by ISS, now OmniTrust.
   - `rootca.prod.s2x.io`, which is SAESOL Tech's "S2X". The attribution comes from the vendor [V] and the hostname.
   - `v2xelector1` to `v2xelector5.scmsmanager.org`.

   The 2025 test CTL carries 5–6 roots: Autocrypt, Chunghwa Telecom, ISS, Microsec, SAESOL, and an Eviden/c-its-pki root. [A]
3. **The root and the electors are offline.** Each sits in a FIPS 140-2 Level 3 HSM and is never networked. The root key has a 20-year cryptoperiod and its certificate a 70-year lifetime. Elector certificates last 15 years by default, and the first five were staggered at 12/15/18/21/24 years. *(Provider Req. §3.5.1, §3.6.1–3.6.4; Elector Policy §2.2.8, §2.2.10, §2.3.1)* [P]
4. **Electors sign with ECDSA NIST P-384.** Elector public keys are the ultimate trust anchor. The OEM or vendor installs them in a secure environment at enrolment. *(Elector Tech Spec §2.3, §2.7; EE Req. §5.6.1)* [P]
5. **The profiled options are fixed:**
   - Original butterfly keys, with the cocoon expansion at the RA and the butterfly expansion at the ACA.
   - Implicit certificates.
   - No ACPC, no OAuth/SAS, and no X.509 enrolment certificates.
   - TLS 1.2/1.3 sessions of at most 5 min, with the scmsV3 REST API.
   - A 1-week i-period with a 1-hour overlap. The i-period epoch is **4 am Eastern, Tue 6 Jan 2015**.

   *(Profile §2.1.9, §2.1.17–2.1.20, §3.2, §5.1, §6.2)* [P]
6. **There are five CRL series.** Revocation uses composite CRLs:
   - Series 256: system certificates the root signs.
   - Series 2: ICA, ACA, ECA, RA, LA.
   - Series 1: OBU pseudonyms, revoked by **linkage seed**.
   - Series 3: RSU application certificates and OBU identification certificates, revoked by hash.
   - Series 4: enrolment certificates.

   A provider must republish a CRL within 1 business day of any change. *(Profile Table 1, §6.3; Provider Req. §3.4.10)* [P]
7. **Providers must run a Misbehaviour Authority and share reports with each other.** The SCMS Manager MBR spec is the CV-pilot MVP: PSID 0x26 carries reports about PSID 0x20 BSMs. SAE J3287 was published in Aug 2024. An end-to-end over-the-air MBD-to-CRL demo was only shown at a June 2026 plugfest. Production MA operation at scale is **UNCONFIRMED**. *(Provider Req. §2.2; MBR Spec §1.0, §2.2)* [P][V]
8. **RSU and OBU lifecycles differ:**
   - OBU pseudonyms: 1-week validity, about 20 per week (NYC negotiated 60), with up to 3 years pre-generated.
   - RSU application certificates: 1 week, generated at most a week ahead, so an RSU needs backhaul to the RA every week.
   - Wyoming could not keep identification and application certificates fresh in rural areas, so it fell back to pseudonyms.

   *(USDOT SCMS Technical Primer FHWA-JPO-19-775, pp. 5–8)* [P]
9. **Enrolment (bootstrap) happens at the factory or vendor in an audited secure environment.** Key generation must meet the FIPS 140-2 §4.7 "level 2" requirements. Elector and root certificates are installed over a high-integrity channel. Enrolment certificates carry wildcard permissions, CRL series 4, and an identifiedRegion of USA, Canada and Mexico only. They are re-enrolled 12–24 months before expiry, with no overlap. *(EE Req. §5.6; Profile §4.4.1, §6.3.1; Re-enrolment Spec §2)* [P]
10. **The regulatory backdrop is C-V2X only, with no FCC security mandate.** FCC 24-123 (adopted 20 Nov 2024) moved ITS in 5.895–5.925 GHz to C-V2X. DSRC sunsets on **14 Dec 2026**. The FCC imposes no SCMS or 1609.2 rule (¶15). USDOT's Aug 2024 plan sets non-binding targets, including that "2 SCMS providers demonstrate interoperable security credentials management" by 2028. *(FCC 24-123 ¶15; DA 25-125 p.2; USDOT plan pp.10–13)* [P]

---

## 1. Governance

### 1.1 SCMS Manager LLC
- SCMS Manager describes itself as the "V2X security standard setting organization for the US V2X ecosystem". Its footer gives the legal name SCMS Manager LLC. *(https://www.scmsmanager.org/ and /about/)* [P]
- Its members fall into three groups: automotive OEMs, V2X technology and service companies, and non-commercial agencies (federal, state, county, municipal). No members are named publicly. *(/about/)* [P]
- **History.** The 2019 Elector Policy names the SCMS Manager **Board of Directors (BOD)** as "the policy authority for SCMS operations". It says the elector concept was "originally envisaged by CAMP and USDOT" (CAMP VSC5). *(Elector Policy v1.0, 22 Oct 2019, §1.0)* [P] The CAMP Proof-of-Concept (POC) SCMS was built under a USDOT cooperative agreement. *(Brecht et al., "A Security Credential Management System for V2X Communications", IEEE T-ITS 2018, arXiv:1802.05323, Abstract and §I)* [P]. For the CV Pilots, the POC "was replaced with a commercial solution". *(CVPD Lessons Learned Logbook Synthesis, FHWA-JPO-22-927, Apr 2022, p.11)* [P]
- **Current bodies** *(Provider Req. v1.1, 9 Aug 2024, §1.2; EAC Req. v1.0, 9 Aug 2024, §1–2)* [P]:
  - **Ecosystem Audit Committee (EAC).** It decides by majority vote of Core members, one vote per member, and a split vote fails. It adds and removes Root CAs and Electors and approves provider and elector CP/CPS documents. It verifies WebTrust compliance every year and approves all SCMS Manager policy documents. It has a Chair, who is the single point of contact, and a Vice-Chair. *(EAC Req. §2.1–2.5)*
  - **Policy Workgroup.** It reviews a provider's CP against the Provider Requirements, using the auditor's report, and reports to the EAC within one month. *(EAC Req. §2.4.1)*
  - **Standards Board.** It runs working groups and liaises with IEEE, SAE and OmniAir. *(/about/)*
  - **External PKI auditor.** An accredited WebTrust for CAs auditor, independent of the audited entity. *(Provider Req. §1.2.6)*
  - **Subscribers.** OEMs, Tier 1s, device makers, governments and road operators that sign a Subscriber Agreement with a provider. *(Provider Req. §1.2.5)*
- **Voting thresholds changed between versions.** The 2019 Elector Policy required 75% of the BOD to add or remove an RCA or Elector (§1.1). The 2024 EAC Requirements use a simple majority of the EAC (§2.2, §2.5). The 2024 documents supersede the 2019 one in practice. Which governs today is **UNCONFIRMED**, but the 2024 documents are the newer ones.

### 1.2 Published policy set
All of these are on https://www.scmsmanager.org/publications/ [P]:

| Document | Version / date | What it fixes |
|---|---|---|
| Provider Requirements | v1.1, 9 Aug 2024 | The template every provider's CP/CPS must follow (Annex 1 = the de-facto US CP) |
| EAC Requirements | v1.0, 9 Aug 2024 | EAC membership, voting, CTL change authority |
| Elector Policy | v1.0, 22 Oct 2019 | 5 electors, ballots, response times, HSM, offline |
| Elector Technical Specification | v1.1 (2020) | P-384 ECDSA, 3-of-5 quorum, tbsCTL format |
| IEEE 1609.2.1 Profile | v1.0.3, 15 Mar 2023 | Every option choice in 1609.2.1 (see §5) |
| End-Entity Security Requirements, Design Guidance and Validation Approach | v2.04, 12 Oct 2023 | Device HSM/architecture/enrolment environment |
| EE Certificate Re-enrollment Technical Spec | v1.0 (2020) | Enrolment-cert rollover (CAMP protocol) |
| Misbehavior Report and Application Spec; ASN.1 for MBR | v1.0 / v1.1 (Jan 2020) | MBR format for CV pilots |
| Drafts (Jul 2025, not yet policy): Intersection Validation Policy, Validation Misbehavior, Validation Report Format | draft | Validated-intersection SSP flags (§7.3) |

The following documents are referenced but were still "in development" as of the 2024 texts: Elector Requirements and the CTL Update Process. *(Provider Req. §1.1)* Whether they are published now is **UNCONFIRMED**. They are not on the publications page as of Oct 2026.

### 1.3 Is there a certificate policy?
- SCMS Manager does not publish one ecosystem-wide CP. Instead, Provider Requirements Annex 1 is "a template which shall be used by the Providers as a basis for their own CP and CPS". A CPS need not be public. *(Provider Req. §1, §2.1)* [P]
- 5GAA states that North America has "no available common policy" for SCMS PKI operators, and that the provider policies that exist are incomplete. 5GAA has published a proposed CP, which the Government of Canada partly sponsored. *(5GAA TR "Credential Management Supporting V2X Commercial Deployments", 14 Jul 2025, https://5gaa.org/credential-management-supporting-v2x-commercial-deployments/)* [S]. This read against Provider Requirements v1.1 suggests the template is not regarded as a full CP.
- One provider CP is public: **ISS Root CA Certificate Policy v1.4** (2023), https://ghsiss.com/wp-content/uploads/2023/03/ISS_Root_CA_Certificate_Policy_v1_4.pdf [P]. Details are in §3.3.

### 1.4 Misbehaviour policy
- Every provider "shall implement a Misbehavior Authority (MA) component and adopt the MA procedures", and must agree to share misbehaviour reports with the other providers. *(Provider Req. §2.2)* [P]
- The only published MBR specification is the CV-pilot one *(MBR & Application Spec v1.0, §1.0–2.3)* [P]:
  - Reports travel under PSID 0x26.
  - The only application covered is PSID 0x20 (BSM).
  - Reports are typed as (type, subtype, evidence).
  - Each stored report has a weight, and the weight is multiplied daily at 05:00 Eastern.
  - Reports upload over TCP/IP in descending weight order. If the store is full, the lowest-weight report is evicted.
- SAE J3287 (V2X Misbehavior Reporting) was issued in August 2024 *(SAE J3287_202408)* [P, catalogue entry only]. It is the newer industry format. SCMS Manager has not yet re-profiled to it (**UNCONFIRMED**).
- The Profile places the EE–MA interface outside its scope (§2.1.3). MBR submission is defined only by the MBR spec (§2.1.12). [P]

---

## 2. Who operates what (2024–2026)

### 2.1 Providers
| Provider | Evidence | Status |
|---|---|---|
| **INTEGRITY Security Services (ISS)**, Santa Barbara CA, a Green Hills subsidiary, "now OmniTrust" | Root `v2xrootca.ghsiss.com` is on the **production CTL** [A]. ISS also runs **SCMS Manager's Electors**: "ISS is a 'Provider' of Elector CA operations to SCMS Manager" *(ISS CP v1.4 §2.6.5)* [P]. ISS says it delivered its Root CA and CMS in 2017 [V] | Production |
| **SAESOL Tech** (Korea; LG-invested) "S2X CMS" | Root `rootca.prod.s2x.io` with `ica/eca/aca/ra.prod.s2x.io` is on the **production CTL** [A]. The vendor claims a listing in the "Production CTL of the North American SCMS Manager" *(cybersectools.com/tools/saesol-tech-s2x-cms)* [V] | Production (attribution [A]+[V]) |
| **AUTOCRYPT** (Korea) | Test root `rootca.plugfest.scms.autocrypt.io` *(Test CTL, 17 Jul 2025)* [A] | Test / plugfest |
| **Microsec** (Hungary) | Test root `microsec-scms-rca-2024-1.cloud-scms.v2x-pki.com` [A] | Test |
| **Chunghwa Telecom** (Taiwan) | Test root `rca.cht-scms.com.tw` [A] | Test |
| **Eviden** | SCMS Manager lists Eviden among the test-CTL participants *(publications page)* [P]. It probably maps to `https://rca.plugfest.c-its-pki.org` (**UNCONFIRMED** mapping) | Test |
| **BlackBerry/Certicom, ESCRYPT (ETAS)** | Listed as SCMS providers in a 2022 USDOT/ITE fact sheet, "for informational purposes" *(CV PFS SCMS Fact Sheet, 7 Mar 2022)* [S]. Neither appears on the 2025 CTLs [A] | **UNCONFIRMED** current status |

- **Interoperability.**
  - The Provider Requirements require providers to be interoperable with each other. Device interoperability must be certified by an approved test lab or shown at interop events. *(§2.7)* [P]
  - The test CTLs exist precisely for OmniAir plugfests. The Mar 2025 test CTL was issued "for OmniAir Spring Plugfest and virtual SCMS testing". *(publications page)* [P]
  - A secondary summary of a 2024/2025 USDOT procurement notice says "no multi-vendor CTL has been developed yet" [S]. That is out of date, because a production CTL with two roots now exists [A].
- **Joining the CTL** *(Provider Req. §2.8)* [P]:
  1. The provider submits its CP and CPS, the name of its HSM and its FIPS 140 certificate, its TISAX AL2 or ISO 27001 certificate, and its WebTrust seal.
  2. The Policy WG reviews the submission and the EAC decides.
  3. The provider mails its 1609.2.1 artefacts (CCF, RCA certificate) **on a flash drive by tracked courier**.
  4. SCMS Manager confirms the RCA key hash with the provider **by video call**.
  5. The electors sign a new CTL.
  6. The provider must also send all its sub-CA certificates so SCMS Manager can keep a master CCF. *(§2.1.1)*

### 2.2 The real component list
These are the hostname conventions from Provider Req. §3.3.1.1: `elector`, `rootca`, `ica`, `ma`, `pg`, `crlg`, `aca`, `eca`. ISS's CP uses `aa`/`ea` instead *(ISS CP §5.1.1)*. [P]

Components seen in published 2025 chain files [A]:
- **ISS pre-prod:**
  - `v2xrootca-test.ghsiss.com` (40-year root)
  - `ica.preprod.v2x.isscms.com` (30 years)
  - `dot1.eca.preprod…` (30 years)
  - `pca.preprod…` (30 years)
  - `ra.preprod…` (30 years)
  - `la1.preprod…` and `la2.preprod…` (30 years each)
  - `ma.preprod…` (1473 × 60 h ≈ 10 years)
  - `crlg.preprod…` (≈10 years)
  - `v2xpg-test.ghsiss.com` (≈10 years)
- **SAESOL:** `rootca` (30 years), `ica` (25), `eca` (15), `aca` ×2 (4), `ra` (6), `ma` (12), `pg` (12).
- **AUTOCRYPT:** `rootca` (70), `ica` (40), `eca`, `aca` and `ra` (20 each).
- **SAESOL production:** `rootca`, `ica`, `eca`, `aca` ×2 and `ra` under `*.prod.s2x.io`.

**LA1 and LA2 are real components.** ISS publishes distinct certificates for `la1` and `la2` [A]. That they are run by separate organisations, as the CAMP design requires, is **UNCONFIRMED**. Both sit under one provider's domain.

| Component | Online / offline | Source |
|---|---|---|
| Elector (×5) | **Offline**, powered off and stored securely when unused, FIPS 140-2 L3 | Elector Policy §2.2.8, §2.2.10 [P] |
| Root CA | **Offline**, "shall not have network connectivity at any time"; logs only when activated | Provider Req. §3.5.1–3.5.2; ISS CP §7.2 [P] |
| ICA | Not specified. CAMP: "shields the root" | Brecht §II-B [P]; **UNCONFIRMED** |
| PG (policy generator: GPF/GCCF) | Signed by root, CRL series 256. CAMP calls it intrinsically central | Profile Table 1; Brecht §II-B [P] |
| MA (incl. CRL generator) | Signed by root, CRL series 256, online (receives reports). Each provider must run one | Profile Table 1; Provider Req. §2.2 [P] |
| CRL signer / CRLG | Signed by root | Profile Table 1 [P] |
| ECA | Online, reached by the EE only during enrolment, over TLS | Profile §2.1.2, §2.1.16; Primer p.4 [P] |
| RA | Online, the only server EEs talk to routinely (HTTPS, scmsV3) | Profile §2.1.4, §2.1.8; Primer p.6 [P] |
| ACA / PCA | Online behind the RA; does the butterfly expansion | Profile §6.2.1 [P] |
| LA1 / LA2 | Online, internal; the EE never talks to them | Primer p.4 [P] |
| LOP (location obscurer proxy) | **Not in 1609.2.1 or the Profile.** CAMP and Primer only | Primer p.4 [P]; deployment **UNCONFIRMED** (likely absent) |
| DCM (device configuration manager) | Used at bootstrap in the secure environment | Re-enrolment Spec §1 [P] |
| DC (distribution centre) | Defined in 1609.2.1; DC–EE interface **not profiled** | Profile §2.1.1 [P] |
| SAS (OAuth) | **Not supported** | Profile §2.1.5, §2.1.19 [P] |
| CRL Store / CRL Broadcast | In the CAMP design. In practice, EEs download CRLs from the RA on every connection | Primer pp.3–4 [P] |

### 2.3 Instances, hosting and physical security
- **Cardinality.**
  - Electors: exactly 5 *(Elector Tech Spec §2.1)*.
  - Root CAs: one per provider; 2 in production [A].
  - Providers can run several ECAs and PCAs, for example per region, manufacturer or device type *(Primer pp.4–5)*.
  - "There may be multiple RAs active at any given time" *(Primer p.6)*.
  - CAMP calls the MA, PG and SCMS Manager the only "intrinsically central" components *(Brecht §II-B)*. In today's multi-provider model, however, **each provider runs its own MA and PG**: provider-specific `ma` and `pg` certificates exist [A].
- **HSMs.**
  - All Root and sub-CA keys are generated and stored in HSMs validated to FIPS 140-2 Level 3 or higher. A Root private key is never exported in plaintext. *(Provider Req. §3.6.1–3.6.2)* [P]
  - Physical access needs at least two people. Disaster recovery uses an m-of-n split of tokens and passphrases. *(Provider Req. §3.6.2)* [P]
  - ISS stores its m-of-n shares "at least 100 miles away" and restores to an alternate secure site *(ISS CP §7.5.2)* [P].
- **Facilities.** Cryptographic hardware is physically segregated from other systems, behind layered physical controls reviewed under WebTrust. Records are archived offsite for 5 years. *(Provider Req. §3.5.1, §3.5.3)* [P]
- **Cloud and regions.**
  - SAESOL markets S2X as cloud-native on AWS, Azure, Naver Cloud and G-Cloud [V].
  - Microsec's root hostname includes `cloud-scms` [A].
  - ISS describes a hosted "Certificate Management Service" [V].
  - Which online components run in which cloud regions is **UNCONFIRMED** for every provider; none publishes this.
- **Audit.** Providers need WebTrust for CAs plus TISAX AL2 or ISO 27001. The EAC re-verifies every year. *(Provider Req. §2.1; EAC Req. §2.4.2)* [P]

---

## 3. Trust model

### 3.1 Electors and the CTL
- Electors replace a single super-root. They sign **tbsCTL** messages, which 1609.2.1 uses instead of CAMP's "ballots". A CTL is valid with **any 3 of the 5** signatures. SCMS Manager waits for the first 3 but still requires all 5 to respond, to prove they still work. *(Elector Tech Spec §2.2, §2.4; Provider Req. §1.2.2)* [P]
- **Signature algorithm.** ECDSA over NIST **P-384**, for all electors *(Elector Tech Spec §2.3)* [P]. The 2025 test elector certificates are confirmed as `ecdsaNistP384`, explicit, and self-signed with SHA-384 [A].
- **Cryptographic agility.** Post-quantum migration would add a second elector signature during a transition *(Elector Tech Spec §2.5)* [P].
- **What EEs hold.** EEs are initialised with the five SCMS Manager elector certificates from the publications page *(Profile §2.1.15)*. Elector public keys must get "the highest protection against improper replacement", though they need not be secret *(Elector Tech Spec §2.7)*. SCMS Manager publishes their SHA-384 fingerprints. [P]
- **CtlSeriesId** = `03 48 00 00 00 00 00 00` *(Profile §6.1)* [P]. The IEEE RA registers CtlSeriesIds *(standards.ieee.org/products-programs/regauth/ctlseriesid/)* [P].
- **Published CTLs** [A]:
  - **Production:** dated 13 Aug 2025, sequenceNumber 1, effectiveDate Time32 679737605 (≈ 16 Jul 2025). It embeds two roots (ISS, SAESOL) and five production elector certificates, and comes with a separate cert-chain file for SAESOL's sub-CAs. A full field decode was not done; its quorum and approve/remove lists are **UNCONFIRMED**.
  - **Test (17 Jul 2025):** sequenceNumber 3, quorum = 3, five `rootCaApprove` HashedId32 entries, and 6 root certificates in the package. Electors are `v2xelector{1..5}.test.scmsmanager.org`, with validity 12/15/18/21/24 years from start Time32 503136005 (≈ 11 Dec 2019).
- **Response times.** Elector Policy §2.2.4–2.2.5 says a removal is executed within 48 h and an addition within 5 business days [P]. ISS's CP commits to a CTL removal within 48 h to 3 business days *(ISS CP §6.13)* [P]. Provider Req. §3.4.11 says a new CTL is issued only on EAC request, with an add published within 3 business days [P].
- **Exercising the electors.** The BOD must run a Test Ballot so the electors are not idle for more than one year *(Elector Policy §1.1)* [P].

### 3.2 Root lifetime, rollover and termination
- The root private key has a 20-year cryptoperiod and the root certificate a 70-year lifetime *(Provider Req. §3.6.4; ISS CP §8.5)* [P]. Test roots in the wild vary: 70 years (Autocrypt, Microsec, CHT), 40 (ISS test), 30 (SAESOL), 8 (c-its-pki) [A].
- At key changeover, the provider signs only with the new key. It destroys the old private key but keeps the old certificate available until everything it signed has expired *(Provider Req. §3.5.4)* [P].
- **Termination.** Only an elector vote or the EAC can terminate a Root CA. The provider must give 365 days' notice, and a simple majority of subscribers must agree to transfer to a successor CA. *(Provider Req. §3.5.6)* [P]
- When SCMS Manager revokes a root or elector, it is **not** placed on a CRL. It simply drops out of the next CTL *(ISS CP §6.5.1)* [P].
- Renewal and suspension are not supported. Rekey always uses a fresh key. *(Provider Req. §3.4.6–3.4.9)* [P]

### 3.3 ISS CP specifics (the only public provider CP)
- ISS runs an offline root plus electors for SCMS Manager. Its root may sign the ICA, PG, MA, CRLG, EA and AA. *(§2.7.1)* [P]
- **CRL timing:** root CRLs are issued at least every 5 years, and within 30 days of a revocation decision. Normal publication latency is 5 business days, or 1 business day from a DR site. *(§6.10–6.12)* [P]
- **Other domains:** the same CP also covers an Australian C-ITS root (Queensland TMR, ETSI TS 103 097) and V2G roots for ISO 15118 *(§1)* [P].

---

## 4. Certificate types (OBU, RSU, backend)

| Type | Holder | Validity / cadence | Revocation | Source |
|---|---|---|---|---|
| Enrolment | every EE | Long (years). Re-enrol 12–24 months before expiry, no overlap; ECA max request age 1 week | CRL series 4, but in practice the RA's blocklist | Profile §4.4.1, Table 3, §6.3.1; Re-enrolment Spec §2 [P] |
| Pseudonym (butterfly, LinkageData id) | OBU | 1 week, 1 h overlap; ~20 per week (NYC: 60); up to 3 years generated, one new week added weekly | Series 1, by **linkage seed** | Profile §2.1.17, §6.3.3; Primer pp.5–7 [P] |
| Identification (butterfly or not, non-linkage id) | Emergency, transit and other privileged OBUs | One valid at a time per application; "local" i-value | Series 3, by hash | Profile §1.1, §6.3.3; Primer p.5 [P] |
| Application | RSU (SPaT, MAP, TIM, RTCM) | 1 week, generated at most a week ahead, so the RSU contacts the RA weekly | Series 3, by hash | Primer pp.5–7; RSU LL p.17 [P] |
| TMC / back-end signing | TMC appliance signing MAP, TIM, RTCM, RSM | Same as application; needs a FIPS 140-2 L3 HSM with per-signature authentication | Series 3 (**UNCONFIRMED**) | EE Req. TMC section [P]; RSU LL p.17 (NYC signs MAP/TIM at the TMC) [P] |
| Component certs (ICA, ACA, ECA, RA, LA) | SCMS | Years (see §2.2) | Series 2, by hash | Profile Table 1 [P] |
| System certs (CRL signer, PG, MA) | SCMS | Years | Series 256, root-signed | Profile Table 1 [P] |

Other profiled details:
- All profiled certificates are **implicit** *(Profile §6.3.1, §6.3.3)* [P]. Root and elector certificates are explicit [A].
- The enrolment certificate has wildcard permissions (`subjectPermissions = all`), no `OperatingOrganizationId`, and no LinkageData *(Profile §4.4.1)* [P].
- Download tuning *(Profile Table 3)*: `ra-maxPreloadTime` ranges from 2 weeks to 3 years; `download-minWait` is 5 min; `ra-minWait` is 1 h. [P]

**EE device groups** *(EE Req. v2.04, table on p.12)* [P]:
| Group | Devices | Minimum FIPS 140-2 level |
|---|---|---|
| 1 | Add-on OBUs (BSM, no vehicle bus), PSM devices, micro-mobility | Level 1 |
| 2 | Vehicle OBUs (BSM with vehicle bus), transit, freight, SRM for preemption | Level 2 |
| 3 | RSUs, TMC appliances (SPaT, MAP, RTCM), PSAP vehicles | Level 3 |

All HSMs must still meet the FIPS 140-2 §4.5 Level 3 *physical* boundary *(§5.4.2)*. A FIPS validation certificate itself is not required *(§4.3)*. [P]

---

## 5. Provisioning: factory enrolment and the DCM
- Enrolment ("bootstrap") is "the point where an initial trust relationship is established". It must happen in a **secure environment** with four elements *(EE Req. §5.6–5.6.1)*: a documented procedure, a physically secure location (or actively monitored open area), authorised equipment, and an activity log. Independent auditors may inspect it. [P]
- **Key generation or injection** must be done inside that environment, following the FIPS 140-2 §4.7 "level 2" requirements. No copy of the private key may be kept outside the EE. *(§5.6.1.1 item 4)* [P]
- **Trust anchors** are installed over the high-integrity channel: one or more root CA certificates plus the elector certificates *(item 5)* [P].
- **Who enrols.** The device vendor generally does the bootstrap. In the NYC pilot, devices were enrolled as they came off the assembly line. Factory "burn-in" certificates expire before installation, so a live SCMS connection is needed before an RSU goes into service. *(Primer p.6; RSU LL p.18)* [P]
- **Direct authorisation path** *(Profile §2.1.7)*:
  1. The EE operator gives the ECA the canonical public key. The canonical identity is the HashedId32 of the canonical key.
  2. The ECA accepts the key only if the device meets the EE Security Requirements.
  3. The EE operator then registers the enrolment certificate with the RA. The RA serves only registered EEs and must support blocking.

  Indirect authorisation (via a DCM) is also allowed. [P]
- **Re-enrolment** can happen in the field, without a DCM, using the existing enrolment certificate with the RA *(Re-enrolment Spec §1)*. The 2019 Primer said re-enrolment needed a secure environment, which reflects pilot practice. [P]
- **Redeployment.** Moving an RSU outside its certificate region, or reassigning an emergency OBU, requires wiping its enrolment and application certificates and re-enrolling *(EE Req. §6.2.3.7)* [P].

---

## 6. CV Pilot experience (NYC, Tampa/THEA, Wyoming)
Sources: SCMS Technical Primer FHWA-JPO-19-775 (Nov 2019), RSU Lessons Learned FHWA-JPO-20-804 (May 2020), and the Logbook Synthesis FHWA-JPO-22-927 (Apr 2022). All are [P].
- **SCMS history.** All three sites started on the CAMP POC SCMS, which a commercial SCMS later replaced. Features added to the SCMS sometimes forced rollbacks. **THEA lost almost 75% of its original participants** because of the SCMS delay. *(Synthesis pp.11, 16)* The commercial vendor is not named; ISS is commonly assumed (**UNCONFIRMED**).
- **Usage.** The sites used the SCMS only for signing, never for encryption *(Synthesis p.11)*.
- **Certificate counts.** About 20 pseudonyms per week, sized for 2 h of driving a day. NYC taxis, driving about 12 h a day, got **60 per week** *(Primer p.7)*.
- **Change rule.** NYC changed certificates every 2 km or 5 min, whichever came first. This fixed a privacy gap in the J2945/1 CERTCHG "absolute distance" exception *(Primer pp.7–8)*.
- **Download cadence.**
  - THEA preloaded **3 years** of certificates *(Synthesis p.16)*.
  - NYC topped off weekly through RSUs that advertise the SCMS service in a WSA. The RSU acts as a gateway to a TMC proxy over IPv4 backhaul *(RSU LL p.4)*.
  - OBUs sometimes left RSU coverage before a top-off finished [S].
- **Rural connectivity.** Wyoming could not keep its 1-week application and identification certificates fresh. It used pseudonym certificates, 3-year supply, for its distress-notification application *(Primer p.8)*.
- **Operations.**
  - NYC ran a weekly report on certificate top-off success and failure *(Synthesis p.16)*.
  - Wyoming's Lear RSUs logged so many certificate errors that the logs filled up, and a firmware fix was needed *(Synthesis p.16)*.
  - Devices whose stock of certificates ran out in transit had to be rebooted to fetch new ones *(Primer p.8)*.
- **Message signing.** NYC signed MAP and TIM centrally at the TMC with an HSM; SPaT was signed at the RSU *(RSU LL pp.17, 26)*.
- **Misbehaviour.** A Minimum Viable Product MBD was built for the pilots: an MA operator reviewed reports and added devices to the CRL *(Primer p.7)*.
- **Scale.** NYC planned about 450 RSUs; THEA planned 1,600+ vehicles and 40+ intersections [S]. As-built counts are **UNCONFIRMED**.

---

## 7. What is live versus specified only

| Item | Status | Basis |
|---|---|---|
| Multi-provider CTL with 5 electors | **Live** (production CTL Aug 2025, 2 roots) | [A] |
| Offline roots and electors in FIPS L3 HSMs | Required; audited by WebTrust | [P] |
| Butterfly pseudonyms, weekly i-periods | Live since the CV pilots | [P] |
| Linkage authorities LA1/LA2 | Certificates exist (ISS pre-prod) | [A]; organisational split **UNCONFIRMED** |
| Linkage-seed revocation (CRL series 1) | Specified and profiled; production revocations **UNCONFIRMED** | [P] |
| MA per provider | Required; the CV-pilot MVP existed; end-to-end MBD-to-CRL demo only in Jun 2026 [V] | partly live, **UNCONFIRMED** at scale |
| LOP | Not in 1609.2.1 or the Profile; **likely not deployed** | [P] by omission |
| CRL broadcast (RSU/satellite) | Not used; CRLs come from the RA | [P] Primer |
| DC–EE, EE–MA interfaces, SAS/OAuth, ACPC, ISO 21177 | Not profiled or not supported | Profile §2.1.1, §2.1.3, §2.1.5, §6.2.3, §3.3 [P] |
| Validated-intersection SSP flags | Draft (Jul 2025) | §7.3 [P] |
| Production OEM vehicles using SCMS certificates | **UNCONFIRMED** in this note; see the deployment research note R2d | — |

---

## 8. US status, 2024–2026
- **FCC.**
  - The Second Report and Order (FCC 24-123) was adopted on 20 Nov 2024, released on 21 Nov 2024, and published at 89 FR 100838 on 13 Dec 2024. It codifies C-V2X in 5.895–5.925 GHz [P].
  - The rules took effect on 11 Feb 2025. From then on, new ITS licences cover only C-V2X RSUs. DSRC RSUs may run until **DSRC sunsets on 14 Dec 2026** *(DA 25-125, 11 Feb 2025, pp.2–3)* [P].
  - The order sets **no** security or SCMS requirement. It expects compliance with "existing standards and best practices" and defers to USDOT and NHTSA *(FCC 24-123 ¶15)* [P].
- **USDOT.** *Saving Lives with Connectivity: A Plan to Accelerate V2X Deployment* was published in Aug 2024 and is non-binding [P]. Its SCMS-related targets:
  - "2 providers utilize interoperable SCMS credentials" (Table 2, short-term 2024–2028, p.11).
  - "2 SCMS providers demonstrate interoperable security credentials management following secure by design principles" (p.10 graphic and Table 3, p.12).
  - "10 deployments feature interoperable SCMS capability" (long-term 2032–2036, p.13).

  The term assignment for the second and third items comes from PDF text extraction; verify it against the PDF. Copy used: https://www.pedbikeinfo.org/downloads/Accelerate_V2X_Deployment_FINAL.pdf. The its.dot.gov original currently returns 404.
- **Industry.**
  - OmniAir runs the interop plugfests that use SCMS Manager test CTLs [P].
  - In Sep 2025 ISS joined the EU ECTL at Levels 1 and 2 [V]. SAESOL claims ECTL Level 0 and China TRCL listings [V].
  - Canada may adopt the US SCMS Manager or cross-certify with it *(Elector Tech Spec §3.0)* [P]. 5GAA's 2025 CP proposal is partly Canada-sponsored [S].

---

## 9. Implications for the simulator (for the build agents)
- Model **SCMS Manager (EAC + electors)** as an offline governance actor. Its outputs are CTL issues, each with sequenceNumber, effectiveDate and quorum = 3 of 5, ECDSA-P384 signatures, and CtlSeriesId `0348000000000000`.
- Model **N providers**: 2 in production, up to 6 in test. Each needs:
  - An offline RCA (70-year certificate, 20-year key).
  - An ICA.
  - Online ECA, RA, ACA, LA1/LA2, MA and PG.
  - A CRL signer.
  - Hostnames matching the real naming conventions.
- **Cross-provider trust** is purely CTL plus CCF. An OBU on provider A verifies an RSU on provider B through B's root on the CTL.
- **Time base:** i-period epoch 2015-01-06 04:00 America/New_York, 168 h periods with a 1 h overlap. Implement the five CRL series exactly as in Profile Table 1.
- **Lifecycle gaps:** RSU application certificates need weekly backhaul, and OBU top-offs depend on connectivity. Give the UI distinct states for "certificate starvation" (Wyoming) and "top-off failure" (NYC).
- **Do not invent** a LOP, CRL broadcast, or a single national MA for the "as-deployed" mode. Offer them as optional "CAMP reference design" toggles.

## Sources (primary unless marked)
- SCMS Manager publications: https://www.scmsmanager.org/publications/. All the PDFs and CTL zips cited above, including `2025/08/20250813-production_ctl.zip` and `2025/07/SCMS_Manager-Test_CTL-0348000000000000-679737605.zip`. Drafts: https://www.scmsmanager.org/drafts/
- ISS Root CA Certificate Policy v1.4: https://ghsiss.com/wp-content/uploads/2023/03/ISS_Root_CA_Certificate_Policy_v1_4.pdf
- Brecht et al., IEEE T-ITS 2018: https://arxiv.org/abs/1802.05323
- USDOT FHWA-JPO-19-775 SCMS Technical Primer: https://rosap.ntl.bts.gov/view/dot/43635
- FHWA-JPO-20-804 RSU Lessons Learned: https://rosap.ntl.bts.gov/view/dot/50749
- FHWA-JPO-22-927 CVPD Lessons Learned Logbook Synthesis: https://rosap.ntl.bts.gov/view/dot/66286
- FHWA-JPO-18-687 National SCMS Deployment Support Literature Search: https://rosap.ntl.bts.gov/view/dot/36395
- FCC 24-123: https://docs.fcc.gov/public/attachments/FCC-24-123A1_Rcd.pdf; DA 25-125: https://docs.fcc.gov/public/attachments/DA-25-125A1_Rcd.pdf
- USDOT V2X plan (Aug 2024): https://www.pedbikeinfo.org/downloads/Accelerate_V2X_Deployment_FINAL.pdf
- SAE J3287_202408: https://saemobilus.sae.org/standards/j3287_202408-v2x-misbehavior-reporting
- [S] 5GAA TR, 14 Jul 2025: https://5gaa.org/credential-management-supporting-v2x-commercial-deployments/
- [S] CV PFS SCMS Fact Sheet, 7 Mar 2022: https://engineering.virginia.edu/sites/default/files/Connected-Vehicle-PFS/Resources/CV%20PFS%20SCMS%20Fact%20Sheet%2003072022.pdf (fetch blocked; contents via search digest)
- [V] SAESOL S2X: https://cybersectools.com/tools/saesol-tech-s2x-cms; ISS root CA page: https://ghsiss.com/?p=11; ISS ECTL release, 30 Sep 2025 (PR Newswire)
