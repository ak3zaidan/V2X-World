# Verify: the "harvest now, link later" threat in the US SCMS (Direction 1, H4)

Written 2026-10-18. Sources:
- SCMS facts: `docs/design/scms-real/02-cryptography-and-formats.md` (here **C02**) and `03-protocols-and-operations.md` (**P03**).
- Corpus papers, text extracted with pypdf.
- Fresh sources: ETSI TR 103 949 V1.1.1, ETSI TR 103 971-1 V2.1.1, the 5GAA PQC report PDF, ePrint and arXiv abstracts, and OpenAlex.

Anything not verified is marked as such.

## 1. Verdict

1. **The threat is real and specific, but it is narrower than the thesis states.**
   - Pseudonym unlinkability against a future quantum adversary rests on one thing only: the **confidentiality of the ACA→EE certificate response**, and of the TLS that carries it.
   - It does **not** rest on BSM signatures, infrastructure certificates, linkage values or CRLs.
   - A recorder who captures the download of a device's batch can later recover every certificate in it, and so attribute every archived BSM signed under those certificates.
   - A recorder who captures only BSMs gets no linkage, even with Shor.
2. **Nothing found states this threat for V2X/SCMS.** Both standards-side PQC studies say the opposite: harvest-now attacks are "not the main concern for V2X" (5GAA), and confidentiality protection duration is "N/A" (ETSI). The claim is novel and contrarian. The quantification is the contribution.
3. **The thesis wording needs three corrections** (§5):
   - infrastructure certificates have no harvest-now *privacy* rationale;
   - "the provisioning KEM" is really two components with very different costs;
   - migration stops *new* exposure but never closes the archive.

## 2. What a passive recorder captures, and what Shor/Grover yields

The recorder sits on the access network (cellular/Wi-Fi/backhaul) between the EE and the LOP/RA, plus over-the-air BSM capture. Row I7 instead assumes an archive held by the RA. "Link" means pseudonym↔pseudonym (P–P) or pseudonym↔enrolment/device (P–E).

| # | Recorded item | Protection today (citation) | Future Shor/Grover recovers | Linkage yield |
|---|---|---|---|---|
| I1 | TLS session EE↔RA (all provisioning, download, MBR POSTs) | TLS 1.2/1.3. ECDHE-ECDSA only. Server-only auth. Session ≤5 min, no resumption (P03 l.56–60, [SMP] §3.2) | Shor on the ephemeral ECDHE share gives the session keys, and so the inner 1609.2 SPDUs. One ECDLP **per session** | Gateway to I2–I5. On its own, none beyond metadata |
| I2 | `EeRaCertRequestSpdu`: signed by the enrolment cert, ECIES to the RA's encryption key. Contains A, ck, H, ek (original butterfly, the US profile) | ECIES P-256 + AES-128-CCM (C02 §8, §9.1; P03 l.36–37, l.94) | Shor on the **RA's long-term** encryption key decrypts *every* request ever sent to that RA cert (amortised over all devices). That gives A, ck, H, ek and the enrolment cert in clear | Enrolment ↔ (A, ck, H, ek). This is still **no** certificate linkage: the certified key U = B_ij + c·G with uniform secret c, so it is information-theoretically independent of B_ij (C02 §9.3). Shor cannot test a guess |
| I3 | `EeRaDownloadRequest` / `Download-Req` header | Signed by the enrolment key, encrypted to the RA (P03 l.40, l.99, l.185) | Via I1 (and the RA key): the requester's enrolment cert plus the filename `<reqHash>_<i>.zip` | Ties the download session to the enrolment and to the request |
| I4 | **Batch zip of `AcaEeCertResponsePrivateSpdu`**: {certificate, c} encrypted to the cocoon encryption key J_ij = H + f_ek(x_enc)·G, then signed by the ACA | ECIES P-256 per certificate (C02 §9.2, §9.4; P03 l.105) | Two routes. (a) Shor on each J_ij or V. (b) After I2, a single Shor on **H** gives h, and h_ij = h + f_ek(·) for *all* (i, j) in the request's lifetime. Either way the **pseudonym certificates and c** come out in clear | **Full P–P and P–E linkage** of every certificate in every recorded batch. This is the break point. Brecht: certificates "are encrypted by the PCA to the OBE to avoid the RA being able to relate certificate content with a certain OBE" (brecht2018security p.9). Boldyreva proves BKM unlinkability only under **Oracle-DH + IND-CPA** of the ECIES-based RK-MRES (boldyreva2024provable p.3, Thm 7.2, p.13). That is computational and Shor-breakable |
| I5 | Misbehaviour report POST | Signed with a pseudonym cert, encrypted to the MA, sent in a request whose header is signed by the enrolment key (P03 l.43, l.192) | Via I1 + Shor on the MA encryption key: the reporter's pseudonym cert next to its enrolment signature | P–E for the reporter's certificates. A second, smaller channel |
| I6 | BSMs over the air | ECDSA signatures by pseudonym keys. Cert or HashedId8 digest in clear. Linkage value in the cert (C02 §5, §10) | Shor gives pseudonym private keys (a **forgery** threat only for still-valid certs). Grover on the LV: lv = plv1 ⊕ plv2 from two independent 128-bit AES seed chains (C02 §10.1), so ≥2^64 sequential Grover per seed. Impractical | **None.** Each certificate is an independent uniform key (I2 note), and the LVs are symmetric. Not a harvest-now risk, so BSM signatures can stay classical **for privacy** (forgery remains a forward-only threat, as in twardokus2024cryptography p.8) |
| I7 | CRLs with linkage seeds | Public by design (P03 l.41) | Nothing new | Revoked devices are already linkable. No quantum change |
| I8 | (Insider/archive) RA database: enrolment ↔ request ↔ stored encrypted batches (pre-generated up to 3 years ahead, P03 l.39). LCIs = E(pk_LA, ls(0)) | Same ECIES as I4. LCI format not standardised (C02 §10.3) | Shor on the J_ij/H in the RA's own store recovers every certificate. **The RA/ACA separation collapses retroactively, without the ACA colluding.** If the LCIs are public-key encrypted, Shor on the LA key gives the seeds | Full P–E for every device the RA served, whether or not anyone recorded the network |

**Ephemeral versus long-term.**
- TLS ECDHE and ECIES `v` are ephemeral, so they need one ECDLP per session or message.
- The RA encryption key and the caterpillar H are long-lived, so one ECDLP opens many items.
- ck and ek are **never in clear on the wire**; they are always inside I2.
- A CRQC run cost per target device is therefore: ~1 per TLS session recorded, plus 1 for H, plus amortised RA-key runs.
- **Targeted** de-anonymisation (persons of interest) is the realistic early threat. Mass linkage scales with recorded sessions. This is inference, not modelled.

## 3. Severity

- **Which pseudonyms.**
  - Every certificate in every batch whose **download** was recorded. US batches are i-period zips of up to 20 or 60 certificates per week (P03 l.39, l.185).
  - If the request I2 was also recorded, one Shor on H unlocks every *later* recorded batch from that request. ra-maxReqs = 1, so the request is "request once, top up forever" (P03 l.39).
  - Recording only the request or only BSMs yields no certificate linkage.
- **How far back.**
  - To the first recorded download: CV-pilot-era deployments onward. Archives made before migration stay exposed **forever**.
  - **After** migration, exposure continues for certificates already downloaded under ECIES (preload ≤ 3 years, P03 l.39), and for legacy OBUs that cannot be updated to a PQ KEM. 5GAA PQC-TR §4.2 calls this the "legacy dilemma" (refine-pqc.md §4).
- **What it buys the adversary.**
  - Every archived BSM carrying those certificate digests becomes attributable to one device.
  - It is also tied to its enrolment certificate and, at an access-network vantage point, to the IP or subscriber identity. The LOP hides the IP only *from the RA* (brecht2018security p.6; P03 l.67).
  - US enrolment certificates carry no name (`id none`, C02 §5 field 1), so a named person needs the access identity or OEM/RA records.
- **Caveats** (they lower severity but do not remove it):
  - Kinematic tracking already links pseudonym changes for a global BSM eavesdropper. The quantum attack adds *certainty*, multi-year reach and the enrolment link (inference).
  - It requires a CRQC; NIST disallows P-256 only after 2035 (refine-pqc.md §1).
  - It requires a vantage point on the provisioning path. Where TLS terminates (at the LOP or end-to-end at the RA) is **UNCONFIRMED**: CAMP describes the LOP only as stripping IP/MAC (P03 l.67).

## 4. Which single component closes it

| Component migrated | Closes the outsider recorder (I1–I5)? | Closes the RA archive (I8)? | Cost and standard | Note |
|---|---|---|---|---|
| **EE↔RA TLS key exchange → hybrid ML-KEM** (e.g. X25519MLKEM768) | **Yes**, if TLS is end-to-end to the RA | No | One extra ML-KEM-768 share per session. No 1609.2/2.1 change; it is an **SCMS Manager profile edit**, since TLS 1.3 is currently limited to ECDHE (P03 l.57) | Cheapest. Deployable now with stock TLS stacks. It makes H4 close to trivially true for outsiders |
| **ACA→EE response encryption → PQ** (per-certificate unlinkable keys) | Yes | **Yes** | Needs a 1609.2.1 change. A shared symmetric key would let the ACA link one device's certificates, so it needs per-certificate PQ keys (P1 upload), a PQ butterfly (ePrint 2024/946, qSCMS 2018/1247, code-based CBKE: Chen, IEICE ComEX 2025) or the KEM-to-PCA design of chen2026pseudonym (pp. 9–10) | The only complete fix. This is the component to name as "migrate first" |
| EE→RA request ECIES → PQ | Partly (it hides A, ck, H, ek, which removes the "one Shor on H" amortisation) | No | 1609.2 KEM recipient type | Necessary but not sufficient: I4 alone still breaks |
| Infrastructure / enrolment certs, BSM signatures | **No** | No | — | Signatures carry no harvest-now privacy risk. Infrastructure-first is justified by **forgery and lead time**, a different argument |

## 5. Consequences for Direction 1 (corrections)

1. **Split the rationale.**
   - "Infrastructure certificates first" must be argued from trust-anchor lifetime and forgery lead time.
   - "Provisioning first" must be argued from harvest-now.
   - The current thesis bundles them (research-plan.md l.9).
2. **"Provisioning KEM" is ambiguous. Define it as two arms:**
   - T: TLS hybrid KEM, closing outsider recording;
   - R: PQ response encryption, closing outsider recording and the RA archive.

   Family P3 ("P0 + ML-KEM on the download path", refine-pqc.md §3) closes only the outsider case if it means T.
3. **chen2026pseudonym is not P0.**
   - It already applies PQ KEM encryption to the RA *and* to the PCA, plus AES for the response, with PQ enrolment certificates and ECC end-entity keys (pp. 9–10).
   - That is a provisioning-KEM-first design, although it gives no harvest-now motivation: its security text says only that ECDLP-based unlinkability "may become vulnerable" to quantum computing (p.6).
   - Fikry et al. 2026 (ASSIC) also keep ECDSA pseudonyms with Dilithium enrolment/CA certificates and "Kyber for session keys", motivated by harvest-now on 15-year **enrolment certificates** (summaries/fikry2026post.md, abstract only).
   - So the *ordering* is published. The *re-linking analysis* is not.
4. **"N years earlier closes the exposure" is wrong as worded.**
   - Migration stops *accumulation*. Everything archived before it stays linkable.
   - Report instead the exposure accrued by migration year: device-weeks of linkable pseudonyms = recorded downloads × batch size, plus the preload and legacy-OBU tail. That needs a fleet/calendar model, not radio.
5. **The radio channel is irrelevant to this threat.** Provisioning runs over IP, not the PC5 safety channel, so the density/fleet-share break points (E2) and H4 are separate results. Do not present H4 as an output of the city-scale co-simulation.
6. **The engine has no harvest-now metric and no TLS model.** grep finds no "harvest" or "retroactiv" in `crates/`. `crates/v2xw-proto/src/hybrid.rs:35-38` models a KEM only as a replacement for ECIES when encrypting to the PCA or a device.

## 6. Prior statements searched

| Source | What it says | Overlap |
|---|---|---|
| 5GAA PQC-TR (Oct 2026) p.10 | V2X safety/mobility data is short-lived and non-confidential, "so this 'harvest-and-decrypt' attack … is not the main concern for V2X". HN-DL is cited only for commerce, insurance and personal data uploads (pp. 10, 13) | **Contradicts** our claim. It never considers provisioning (no SCMS or butterfly mention, refine-pqc.md §1) |
| ETSI TR 103 949 V1.1.1 (2023-05) Tables 2–3, p.16 | PII is protected "by use of pseudonymous keying … rather than by encryption". Confidentiality "Protection duration: N/A". "Exposure: N/A". Yet "Privacy of the user is to be maintained across the lifetime of the ITS-S" | **Contradicts / misses** it: the EU AT request has the same encrypted-request structure (TS 102 941), but the TR treats privacy as encryption-free |
| ETSI TR 103 971-1 V2.1.1 (2026-05) Annex A, p.8 | Generic: everything "transmitted or stored" under vulnerable algorithms "will become unprotected". The modelled threats are only T.Masquerade and T.UnauthorizedOperation | Generic harvest-now sentence. No pseudonym-linkage threat |
| qSCMS, ePrint 2018/1247 (Barreto et al.) | Abstract: butterfly ECC is "susceptible to quantum attacks"; privacy is a requirement | Generic motivation. No retroactive argument (abstract; full PDF blocked by Cloudflare) |
| Eaton–Lamontagne–Matsakis, ePrint 2024/946 | PQ BKE with formal unforgeability and unlinkability games | No harvest-now framing in the abstract. **Full text not read (403)**; check its introduction before claiming |
| chen2026pseudonym (arXiv 2606.14008) p.6 | Butterfly unlinkability relies on ECDLP and "may become vulnerable in the presence of quantum computing" | Closest statement, but forward-looking, not record-now. Its scheme already closes I2–I4 |
| boldyreva2024provable p.3, p.13 | BKM unlinkability rests on ODH + IND-CPA (computational) | Gives the formal basis for row I4. No quantum discussion |
| awasthi2026quantum p.2, p.6 | Generic transport harvest-now of "V2V safety messages" and telemetry | Inaccurate (BSMs are not encrypted). No SCMS/pseudonym |
| arXiv API (7 queries: "harvest now"+vehicular/V2X; pseudonym+quantum; SCMS+quantum; butterfly+quantum) and OpenAlex (5 queries) | Hits limited to Chen's PQ-SCMS series, PQCWC, PQCMC and generic surveys | None states the threat |
| Not reachable | ePrint 2024/946 and 2018/1247 full texts, IEEE 1609.2.1 prose, C2C-CC PQC white paper (not found) | Residual novelty risk: **low to moderate** |

## 7. What to claim

- **Defensible claim.** In the US SCMS, the unlinkability of archived pseudonym traffic against a quantum adversary is exactly as long-lived as the confidentiality of the certificate-download channel. It is independent of the signature algorithms. Two industry/standards migration studies explicitly judge harvest-now irrelevant to V2X, and this item-by-item trace shows they are wrong for provisioning.
- **Quantification to add.** Linkable device-weeks by migration year, under T-only versus R, with preload horizon and legacy-OBU share as factors. Also the CRQC runs per de-anonymised device.
- **Before submission.**
  - Read the introduction of ePrint 2024/946.
  - Confirm where TLS terminates (CAMP122 §5.1.8).
  - Settle the unified/CUBK response key (C02 §9.4 UNCONFIRMED). CUBK encrypts directly to B_t, so the same I4 analysis applies.
