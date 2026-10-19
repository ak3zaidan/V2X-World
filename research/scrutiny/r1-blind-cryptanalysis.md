# R1: blind cryptanalysis of US SCMS pseudonym unlinkability against a future quantum adversary

Date: 2026-10-18. I wrote this independently. My only project inputs were
`docs/design/scms-real/02-cryptography-and-formats.md` (cited below as **D02 §x**) and
`03-protocols-and-operations.md` (**D03 §x**). I read no research synthesis or scrutiny file and no git history.

The adversary is a **passive recorder (harvest now, compute later)**. Later it owns a cryptographically relevant
quantum computer (CRQC): Shor's algorithm solves ECDLP and ECDH on P-256, so it breaks ECDSA, ECQV, ECIES and
ECDHE, and Grover gives roughly a square-root speed-up against AES-128 and SHA-256 preimages. I treat active
quantum attacks only briefly (§7).

## Primary sources used

| Tag | Source | Where read |
|---|---|---|
| [BRECHT] | Brecht et al., "A Security Credential Management System for V2X Communications", IEEE T-ITS 2018, arXiv:1802.05323 | PDF pp. 4, 9–10, 13–14, 21 (pypdf extraction) |
| [BKS] | Boldyreva, Kumar, Sun, "Provable Security Analysis of Butterfly Key Mechanism Protocol in IEEE 1609.2.1 Standard", PoPETs 2024(4):565–582, https://petsymposium.org/popets/2024/popets-2024-0130.pdf | pp. 566–567, 569–571, 574–575 |
| [ASN-P] | IEEE 1609.2.1-2022 ASN.1 `Ieee1609Dot2Dot1Protocol.asn`, ETSI Forge tag `2022-published`, https://forge.etsi.org/rep/ITS/asn1/ieee1609.2.1/-/raw/2022-published/Ieee1609Dot2Dot1Protocol.asn | notes on `AcaEeCertResponsePrivateSpdu` and `AcaEeCertResponseCubkSpdu` (lines ~600–700) |
| [SM-MBR] | SCMS Manager, "Misbehavior Report and Application Specification for CV Pilot Deployment" v1.0 (2019), https://scmsmanager.org/wp-content/uploads/2020/01/Misbehavior-Report-and-Application-Specification-v1.0.pdf | §3.0 and §4.0 (printed pp. 9–10) |
| [UBK] | Simplicio et al., "The Unified Butterfly Effect", IACR ePrint 2018/089 | abstract only (the PDF is behind a Cloudflare challenge) |
| [qSCMS] | Barreto, Ricardini, Simplicio, Patil, "qSCMS: Post-quantum certificate provisioning process for V2X", IACR ePrint 2018/1247 | abstract only |
| [EAT] | Eaton, Lamontagne, Matsakis, "Provably Secure Butterfly Key Expansion from the CRYSTALS Post-Quantum Schemes", IACR ePrint 2024/946 | abstract only |

I did not read the paywalled body text of IEEE 1609.2 or 1609.2.1, nor the CAMP SCMS PoC specification (D03 tags
it [CAMP122]). Everything here that depends on those comes through D02 or D03 and is marked as such.

---

## 1. The mechanism in one paragraph (what has to stay secret)

**The request.** The vehicle (EE) sends a single provisioning request (`ra-maxReqs = 1`; "request once, top up
forever", D03 §1 item 5).
* It contains the caterpillar signing key A, the AES-128 expansion key ck, the caterpillar encryption key H and
  the expansion key ek (D02 §9.1).
* It is signed with the enrolment certificate and ECIES-encrypted to the RA's static encryption key (D03 §1
  item 2, §3.2), inside HTTPS with ECDHE TLS 1.2 or 1.3 (D03 §2).

**What the RA does.** It expands the request into cocoon keys B_ij = A + f_ck(i,j)·G and J_ij = H + f_ek(i,j)·G
(D02 §9.2; [BRECHT] p. 9). It shuffles them and sends each one to the ACA together with two PLVs encrypted for the
ACA (D02 §10.3).

**What the ACA does.**
* It certifies U = B + c·G (explicit) or R = B + k·G (implicit ECQV), using a fresh random c or k (D02 §9.3;
  [BRECHT] p. 10).
* It sets the linkage value to plv1 ⊕ plv2.
* It ECIES-encrypts {certificate, c or r} to J_ij, then signs the ciphertext (D02 §9.4; [ASN-P]).

**How the vehicle collects them.** The RA zips the responses per device and per week. The EE downloads them over
TLS with a `Download-Req` header signed by its enrolment key (D03 §4.2).

**Key observation.** Because c (or k) is uniform and fresh, U (or R) is *statistically independent* of B. Even an
unbounded adversary that knows A, ck and every B_ij cannot tell which certificate came from which cocoon key unless
it learns c (or k), or the certificate together with its request mapping. Those values exist only in three
places:
* the ECIES response ciphertexts;
* the ACA's memory or storage;
* the EE.

Unlinkability across certificates of one vehicle also depends on the linkage values, which are AES/SHA-based PRF
outputs of LA-held seeds (D02 §10.1; [BRECHT] pp. 12–13). Shor does not touch them.

So **Shor attacks unlinkability only by breaking confidentiality of something carrying (A, ck, H, ek), the
responses (c, cert), the LCIs (seeds), the PLVs, or the misbehaviour reports.** It does not attack it through
the BSM signatures or certificates themselves.

---

## 2. Source-by-source: what the recorder can learn

"Shor count" means the number of independent discrete-log computations needed.

### 2.1 Recorded broadcast BSMs with attached pseudonym certificates

**What is visible:** U or R, the linkage value, iCert, validity, region, application permissions, the ACA's
HashedId8, and ECDSA signatures (D02 §5).

**What Shor adds: nothing for linkability.**
* Shor on a pseudonym key gives its private key, which allows forgery, not linkage.
* Shor on R gives b_ij + k. With k uniform, that value is consistent with every candidate b_ij, so it carries no
  information.
* Shor on the ACA key gives d_ACA. This allows forgery, and lets anyone strip the ECQV r back to b + k, which
  again carries no information.

**Linkage values.** These are 72-bit truncated Davies–Meyer AES outputs keyed by 128-bit seeds (D02 §10.1).
Linking two of them requires a seed. Grover search over a 128-bit seed costs about 2^64 sequential AES
evaluations per targeted chain. That is the NIST category-1 margin, and I consider it practically out of reach.

**Residual.** The same classical, non-cryptographic linkage exists as today: certificate-change timing,
trajectories, and MAC/L2 identifiers. A CRQC changes none of it.

**Conclusion:** BSMs alone give no quantum gain in linkability.

### 2.2 Recorded provisioning requests (EE → LOP → RA)

**Layers:** TLS with ECDHE, then inner ECIES to the RA certificate's static encryption key, then a signature by
the enrolment certificate (D03 §1 item 2, §2, §3.2).

**Shor count:** one per TLS session (ephemeral ECDHE; the US profile forbids session resumption and caps sessions
at 5 minutes, D03 §2), plus one more for the inner ECIES layer. That second one is either per message (on the
ephemeral V) or **once per RA certificate** (on the RA's static key, which then opens every request ever sent to
that RA certificate).

**What it reveals:**
* the enrolment certificate, a stable device identifier;
* A, ck, H and ek, which give every B_ij and J_ij for every period i the request will ever cover (up to
  `ra-maxPreloadTime` = 3 years, and the caterpillar outlives an enrolment rollover: D03 §5, "existing pseudonym
  batches stay downloadable with the new certificate").

**On its own this is not enough to link pseudonym certificates**, because c or k is missing (§1). It becomes
complete linkage once combined with §2.3 or §2.5 (see §3).

**Network identity:** on the access side the recorder also sees the IP address or cellular identity. The LOP
removes it only from the RA's view (D03 §2). So a TLS break binds network identity to enrolment identity.

### 2.3 Recorded certificate downloads (EE ← RA)

**Layers:** TLS with ECDHE, then a `Download-Req` header. The header is signed by the enrolment key and carries a
filename `<reqHash>_<i>.zip`; D03 §4.2 does not say whether it is also encrypted. Inside the zip, each file is
`AcaEeCertResponsePrivateSpdu`: ECIES to J_ij, then an ACA signature (D03 §4.2, §3.2; [ASN-P]).

**After a TLS break (one Shor per session):** the recorder sees the enrolment identity, the batch timing and
count, and the ciphertexts, which are **bound to the identity**.

**To decrypt the ciphertexts** the recorder needs the recipient key J_ij, or h_ij. ECDH needs the recipient's
public point: Shor on the ephemeral V gives v, but the shared secret is v·J_ij. J_ij is not derivable from the
download. The recipient identifier is at most a hash of the key (D02 §8.1); exactly which RecipientInfo form the
response uses is unconfirmed, see §8.

**Therefore downloads alone give identity plus volume, but not the certificates.** Downloads plus the request
(§2.2) give everything:
* Shor once on H gives h, and then h_ij = h + f_ek(x_enc) for all i, j classically. One Shor opens every
  response for that request's lifetime.
* Each response yields {certificate, c}, so every pseudonym certificate is linked to the enrolment identity, and
  so to every other pseudonym certificate of that vehicle.

### 2.4 The TLS sessions as such

TLS (ECDHE key exchange, ECDSA or X.509 server authentication, AES-GCM/CBC/CCM; D03 §2) is the **only**
confidentiality layer around three things:
* the `Download-Req` identity binding;
* the misbehaviour-report upload header;
* the network-address-to-enrolment mapping.

Around the request content it is one of two layers (§2.2). The CAMP rule that one session carries the LPF/LCCF
fetch, then requests, then batches, then `.info` (D03 §4.2) means **a single Shor on one session can expose
request and download together**.

AES-128/256 in the record layer is safe against Grover in practice. The server's ECDSA authentication matters
only to an active attacker (§7).

**Uncertain:** whether TLS terminates at the LOP or end-to-end at the RA (D03 §2 says only "strips IP/MAC and
forwards"). If it terminates at the LOP, then:
* the LOP and its links see the `Download-Req` and the misbehaviour headers in clear;
* the inner ECIES layer becomes the only protection for the request.

### 2.5 Backend component traffic (RA↔LA, RA↔ACA, RA↔MA)

D03 §9 says "TLS between components". Breaking it (Shor per session; component sessions may be long-lived, so
the count is small) gives the following.

**RA→ACA `RaAcaCertRequest`:** B_ij, J_ij (as `certEncKey`) and encPlv1/encPlv2, all signed by the RA. The
**ACA→RA `AcaRaCertResponse`** carries the `requestHash` and the EE-encrypted response (D03 §9; D02 §9.1). So J_ij
is visible here, and **each response can be opened with one Shor per certificate** (on J_ij or on V). That gives
the certificate and c, plus the B_ij it was issued for.

This step alone does not identify a vehicle, because the requests are shuffled. Two things re-group them:
* **Ciphertext bytes are carried unchanged** from the ACA→RA response into the EE's zip (the RA "zips it
  untouched", D03 §7, §9). Matching them against a broken download (§2.3) binds each opened certificate to the
  enrolment identity. This defeats the shuffle **without ever recovering the request**.
* If the request was recovered (§2.2), the recorder computes B_ij itself, finds the matching `RaAcaCertRequest`,
  follows the `requestHash` to the response and decrypts with h_ij. No download is needed.

**LA→RA PLV responses** (per device chain, D03 §9) carry the **same `EncryptedIndividualPLV` ciphertexts** that
later appear in `RaAcaCertRequest.linkageInfo` (D02 §10.3). Byte-matching therefore groups the shuffled
RA→ACA requests by device, again without breaking any encryption beyond TLS.

**LCIs.** [BRECHT] p. 13 defines the linkage chain identifier as "the initial linkage seed ls1(0) … that LA 1
… encrypts to itself, e.g., E(pk1, ls1(0)), where pk1 is the public key of LA 1" (also Fig. 4, p. 14). The RA
keeps LCIs per device and sends them to LAs, and they move RA→MA→LA during revocation (D03 §8.2; D02 §10.3). **If
the deployed LCI really is ECIES to the LA's static key**, then:
* one Shor per LA key turns every recorded or stored LCI into ls(0);
* that gives every PLV, and with the partner LA's seed (paired through encPlv1/encPlv2 in the same
  `RaAcaCertRequest`), every linkage value of that device for all periods;
* the result is complete cert-to-cert linkage of the vehicle, past and future, independent of butterfly
  entirely.

The LA–RA, LA–MA and MA–RA formats are not standardised (`NULL` in 1609.2.1, D02 §10.3), so this is a design
reading, not a confirmed deployed format.

**LA↔ACA PLV key.** The PLVs are AES-128-CCM under a key shared between the LA and the ACA. The key-sharing
method is "out of scope of the standard" (D02 §10.3), with rotation per `shared_key_update_interval` (D03 §9). If
that key is established over ECDH or ECIES and the exchange is recorded, Shor recovers it. Then every encPLV
decrypts, giving linkage values per device group.

### 2.6 Misbehaviour reports

**Format:** a report is signed with "the end-entity certificate" ([SM-MBR] §3.0; D03 §8.1 says a pseudonym
certificate in CAMP's design). It carries generationTime, generationLocation, the reporter's own BSM and suspect
BSMs with their certificates (D03 §8.1).

**Protection:** it is ECIES-encrypted to the **MA certificate's static encryption key** with an ephemeral sender
key ([SM-MBR] §3.0). It is POSTed to the RA with a `Download-Req` header "signed by the device's enrollment
certificate", over TLS ([SM-MBR] §4.0).

**Outsider:** one TLS Shor gives enrolment identity ↔ report ciphertext. One Shor on the MA key (shared by all
reports ever sent to that MA certificate) opens the reports. Together they give **reporter pseudonym certificate ↔
enrolment identity**, plus the reporter's location and time.

**RA insider or RA logs:** the RA already holds the identity-to-ciphertext pairing (the header). One Shor on the
MA key gives the same result for every report it ever saw. The RA's shuffle before forwarding to the MA (D03 §8.2)
does not help, because the binding is formed before the shuffle.

**Reach:** each report links one pseudonym certificate, and through its linkage value nothing more without seeds.
But reports recur, so over time they sample many of a vehicle's pseudonyms.

### 2.7 CRLs

**Content:** CRLs are plaintext and unauthenticated on download (D03 §4.2, §10). They publish (ls1(iRev),
ls2(iRev)) for revoked devices (D02 §11). That gives linkage of a revoked device's certificates from iRev forward.
This is by design and classical.

**Backward linkage:** linking a revoked device's earlier certificates needs SHA-256 preimages on a 128-bit
unknown input (D02 §10.1). Grover costs about 2^64 per step back, per device. That is impractical, but it is the
only place where Grover is the binding cost.

**Downloads:** CRL downloads are not authenticated (D03 §4.2), so they reveal no EE identity.

**Shor adds no linkage**, only CRL forgery (an active attack).

### 2.8 An RA's stored data (stolen or retained, then decrypted later)

Per D02 §9.5 and D03 §§3.3, 5, 8–9, the RA stores:
* enrolment certificates, registered by the operator (and therefore mapped to a vehicle);
* A, ck, H, ek, and the (i, j) for every cocoon key;
* the zips of EE-encrypted responses (kept for re-download while the certificates are unexpired, SCMS-549, D03
  §6.4, i.e. **up to 3 years of pre-generated batches**);
* the LCIs;
* possibly the misbehaviour-report headers and ciphertexts in transit.

**Response archive:** one Shor on H per device (or on each J_ij) decrypts every stored response. That gives
**every pseudonym certificate of every vehicle linked to its enrolment identity.**
* No TLS change helps, because this data is at rest inside the trust boundary the design relies on.
* This is exactly the "privacy against a corrupted RA" property that [BKS] Thm 5.1 (p. 574) proves under the
  Hash Diffie-Hellman assumption ([BKS] Thm 6.1, p. 575), which Shor breaks.

**LCIs:** if they are public-key encrypted, Shor on the two LA keys gives all seeds for all devices (§2.5).

**Misbehaviour reports:** see §2.6.

**Conclusion:** the RA archive is the highest-value harvest target. A single breach today is enough for
system-wide linkage later.

### 2.9 An ACA's stored data

The ACA stores shuffled (B_ij, J_ij, encPLVs, the issued certificate, c or k, the linkage value) records with no
EE identity and no j (D02 §9.5).

**On its own**, cocoon keys of one EE are unlinkable because they differ by AES-PRF outputs f_ck. [BKS] Thm 5.2
(p. 575) proves ACA-privacy assuming only that the expansion function is an ideal cipher. Shor does not help:
logs of B_ij differ by unknown f_ck values. Grover on ck is about 2^64 per *candidate pair* of cocoon keys, and
infeasible at scale.

**The ACA archive plus any one source of (A, ck) for a device** (a broken request, §2.2, or RA data) gives B_ij,
which finds the ACA records, which give the certificates. That is full linkage without touching any response
ciphertext.

**The ACA plus RA archives** amount to classical collusion: no privacy even today ([BKS] p. 566).

---

## 3. Harvest-to-linkage paths, cheapest first

| # | Recorded or held | Shor work (CRQC runs) | Result |
|---|---|---|---|
| P0 | RA archive (stolen or retained) | 1 per vehicle (on H), or 1 per certificate | All vehicles: all pseudonyms ↔ enrolment identity |
| P0' | RA-held LCIs, *if* public-key | 2 in total (LA1 and LA2 keys) | All vehicles: all linkage values, all periods |
| P1 | EE request session + EE download session(s) | 1 (request TLS) + 1 (RA static key, amortised) + 1 per download session + 1 (H) | One vehicle: years of pseudonyms ↔ enrolment identity and network address |
| P2 | EE request session + RA↔ACA traffic | as P1, but the backend sessions replace the downloads | as P1 |
| P3 | EE download sessions + RA↔ACA traffic | 1 per TLS session + 1 per certificate | as P1 (more expensive, about 20 per week per vehicle) |
| P4 | RA↔LA + RA↔ACA traffic, LCIs or the LA–ACA key public-key based | few (component sessions) + 2 LA keys | All recorded devices: cert-to-cert linkage, no identity (re-identification by trajectory) |
| P5 | Misbehaviour-report upload, or RA's MBR log | 1 per TLS session + 1 MA key (amortised) | Sampled pseudonyms ↔ enrolment identity, with location |
| — | BSMs only / CRLs only / ACA archive only | n/a | No quantum gain |

Information-theoretic hiding of B by c means that **no future computation links a pseudonym certificate to its
request unless a ciphertext that carried c, the certificate, seeds or PLVs was recorded or stored.** This is also
why signature algorithms do not matter for passive linkability.

---

## 4. What butterfly unlinkability rests on

| # | Assumption | Source | Holds against a CRQC? |
|---|---|---|---|
| A1 | RA and ACA do not collude; separation rules ACA≠RA, ACA≠LA, LA1≠LA2 | [BKS] p. 566; [BRECHT] p. 13; D03 §9 | Organisational. Unchanged, but P0 makes "RA alone" equivalent to collusion |
| A2 | The RA routes a whole EE batch to one ACA, so it cannot split an EE across ACAs | [BKS] pp. 566–567, 571 | Organisational. Unchanged |
| A3 | c or k fresh, uniform and secret (statistical hiding of B in U or R) | [BRECHT] p. 10 and fn. 7; D02 §9.3 | **Yes** (information-theoretic), provided the ACA's RNG and storage are sound |
| A4 | Responses confidential to the EE, including from the RA: ECIES under additive related keys (IND-CPA-wRKA, Hash DH) plus target-robustness; also UF-CMA of the ACA signature | [BKS] Thm 5.1 p. 574, Thm 6.1–6.2 p. 575 | **No.** HDH falls to Shor |
| A5 | The expansion function f_ck / f_ek is a PRF (ideal cipher): ACA-side cocoon-key unlinkability | [BKS] Thm 5.2 p. 575; [BRECHT] p. 9 | Yes, modulo Grover (about 2^64) and an unproven QROM/quantum-ideal-cipher lift of [BKS]'s classical proof |
| A6 | EE↔RA channel secure: protects A, ck, H, ek and the identity bindings | [BKS] p. 571 ("talk over a secure channel"), p. 569; D03 §2 | **No** with ECDHE TLS and the inner ECIES layer. [BKS] *assumes* it and does not model the outsider |
| A7 | Linkage values: seed chain one-way, PLVs pseudorandom; PLVs hidden from the RA (LA–ACA key); LCIs hidden from the RA | [BRECHT] pp. 12–13; D02 §10.1, §10.3 | PRF and hash: yes (Grover margin). LCI and LA–ACA key: **depends on how they are encrypted or established**, which is not standardised |
| A8 | Shuffle, LOP and transport metadata do not re-link (the threshold, D03 §6.3, §8.2) | D03 §2, §6.3 | Not cryptographic. But ciphertext byte-identity across hops (§2.5) means the shuffle is only as strong as the backend transport's confidentiality |
| A9 | Misbehaviour reports confidential from the RA and outsiders (ECIES to the MA's static key) | [SM-MBR] §3.0 | **No** |

[BRECHT] itself flags the quantum dependency (p. 4): "we assume that today's cryptographic mechanisms are
acceptably secure … certain solutions need to be redesigned, such as the Butterfly key expansion … since it
assumes … the discrete logarithm problem." It frames this around ECDSA and key expansion, not around the
privacy (encryption) side.

---

## 5. Minimum post-quantum set

Integrity primitives do **not** need to be post-quantum for passive unlinkability: ECDSA on BSMs, ECQV/ECDSA
pseudonym and CA certificates, CRL and CTL signatures, enrolment signatures, and TLS server authentication.
Symmetric primitives can stay as they are: AES-128 for f_ck and f_ek, PLVs, CCM and the TLS record layer;
SHA-256 for seeds. Grover leaves about 64-bit quantum work. If more margin is wanted, AES-256 or 256-bit seeds
are an easy upgrade.

What must change depends on whom you want to keep out.

**(a) Outsider recording only the EE access network.** It is sufficient to make the EE↔RA TLS key exchange PQ or
hybrid. One alternative sufficient set, keeping classical TLS, is all three of:
* a PQ inner encryption of the request (to the RA);
* PQ misbehaviour-report encryption (to the MA);
* acceptance that IP ↔ enrolment identity leaks.

This works because the response ciphertexts are useless without J_ij (§2.3). Neither option alone covers the
other's channel. TLS-PQ is the simpler fix and covers request, download and misbehaviour reports together.

**(b) Outsider recording EE and backend traffic.** You need (a), **plus** PQ confidentiality on RA↔ACA, RA↔LA and
RA↔MA links, **plus** LCIs encrypted symmetrically or with a PQ KEM, **plus** an LA–ACA PLV key established
without classical ECDH. Without these, P3 and P4 work even with PQ EE-side TLS.

**(c) The design's own threat model** (a curious RA, or anyone who later obtains RA storage). You need **PQ
response encryption.** This is the encryption half of butterfly: a KEM whose per-certificate public keys the RA
knows but cannot decrypt to. You also need PQ misbehaviour-report encryption and PQ or symmetric LCIs. No
transport fix can substitute, because the data is at rest at a party the design explicitly distrusts (A4, A9).

Two ways to get PQ response encryption:
* **Keep the expansion:** a related-key-secure PQ KEM with key expansion ([qSCMS] ePrint 2018/1247; [EAT] ePrint
  2024/946, ML-KEM/ML-DSA variants with proofs of unlinkability).
* **Drop the expansion for encryption:** independent per-certificate PQ KEM keys. Expansion is an efficiency
  device, not a privacy requirement, at a large upload cost.

The signing half (A, B, U) can stay classical as far as passive linkability goes, because A3 is
information-theoretic. Moving it to a lattice scheme with key expansion ([EAT]) *replaces* that statistical
hiding with a computational (MLWE-type) assumption. That is a new linkability assumption to analyse, not a
removal of one.

**Overall minimum against "a passive CRQC recorder with no SCMS insider data":** confidentiality of every hop that
carries request contents, response ciphertexts plus their J, misbehaviour reports, LCIs or PLV keys. In practice
that is PQ key exchange on EE↔RA plus the component links, plus symmetric or PQ LCI and LA–ACA keys.

**Against insider storage:** add PQ response encryption and PQ misbehaviour-report encryption.

Signatures are not in either minimum set.

---

## 6. Ordering consequence (my reading)

The harvest window is open now and is cumulative. A request recorded today exposes up to 3 years of that
vehicle's future pseudonyms (D03 §6.4). An RA zip archive holds up to 3 years ahead for every vehicle.

Migrating signatures first (BSM, certificates, CRL) does nothing for privacy against a recorder. Migrating
confidentiality first does:
* (a) TLS KEX on EE↔RA and backend links: cheap, standard hybrid TLS 1.3;
* (b) LCI and LA–ACA key handling: backend-only, not EE-visible;
* (c) response and misbehaviour-report encryption: needs an EE and 1609.2.1 change, so it is the longest lead
  time.

Data already harvested under classical key exchange cannot be protected retroactively, except by retiring the
exposed caterpillars, i.e. issuing a fresh request.

---

## 7. Active quantum attacks (brief, out of the passive scope)

A CRQC used in real time could do four things:
* forge the RA's X.509 TLS certificate and the ECIES-to-RA setup to man-in-the-middle provisioning;
* forge an EE's enrolment signature to fetch its zips (still encrypted to J_ij);
* let an RA mint certificates with its own c by forging the ACA's signature ([BKS] Thm 5.1 needs UF-CMA of DS2);
* forge CRLs.

These require PQ *signatures* before a CRQC exists. They are not harvest-now threats.

---

## 8. Uncertainties

1. **LCI encryption.** [BRECHT] p. 13 says "e.g., E(pk1, ls1(0))", but the deployed CAMP format is not public,
   and 1609.2.1 leaves LA–RA as `NULL` (D02 §10.3). If the LCI is symmetric (an AES key held by the LA), P0' and
   the LCI part of P4 disappear.
2. **LA–ACA PLV key establishment** is unspecified (D02 §10.3). If it is pre-shared out of band or symmetric, the
   PLV-decryption part of P4 disappears.
3. **TLS termination point** (LOP or RA) and whether CAMP's `SecuredAuthenticatedDownloadRequest` is also
   encrypted to the RA (D03 §4.2 does not say). Either case changes which single Shor exposes the header, not the
   conclusion.
4. **Response RecipientInfo form** (rekRecipInfo with HashedId8(J), or another) is unconfirmed (D02 §8.1). If any
   form exposed J_ij, downloads alone would suffice (P3 without backend traffic).
5. **Where and when the single provisioning request is sent:** in the field over cellular or Wi-Fi, or in a
   secured factory environment. The bootstrap is in a secure environment (D03 §5), but the RA provisioning
   request is an internet POST via the LOP (D03 §2, §4.2). Pilot practice was manual (D03 §5).
6. **RA and ACA retention.** How long the RA keeps zips, requests, A/ck/H/ek, misbehaviour headers and logs, and
   whether the ACA keeps c or k, are policy questions I could not confirm. Re-download while unexpired (D03
   §6.4) implies at least that long for zips.
7. **Whether misbehaviour reports are signed with a pseudonym or another EE certificate.** [SM-MBR] §3.0 says only
   "the end-entity certificate".
8. **Enrolment certificate identity content.** D02 §5 says it is typically `none`; D03 §5 says `id.name` is
   present and unique in the US. Either way its HashedId8 is a stable device identifier, and the RA registers it
   per vehicle.
9. **Proof gaps.** [BKS]'s proofs are classical ROM/ideal-cipher proofs. The ACA-side result (Thm 5.2) has not, to
   my knowledge, been re-proved in the QROM. [qSCMS] and [EAT] were read only as abstracts (the ePrint PDFs are
   behind a Cloudflare challenge), so I make no claim about their exact assumptions or whether they treat the
   outsider or RA-archive harvest case.
10. **1609.2.1 §9.3.4.2** (unified and CUBK encryption-key derivation) is unconfirmed (D02 §9.4). It is irrelevant
    to the US original-butterfly profile (D03 §1 item 3), but in CUBK the response is encrypted to B_t itself, so
    the same RA-archive exposure applies.
