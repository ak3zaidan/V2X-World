# Verify: is "migration order" novel for V2X, or generic PQC advice? (key `pqc`)

Written 2026-10-18. It checks the thesis in `refine-pqc.md` §3 and `research-plan.md` §1. Primary texts read in this pass (pypdf):
- NIST IR 8547 ipd (https://nvlpubs.nist.gov/nistpubs/ir/2024/NIST.IR.8547.ipd.pdf);
- ETSI TR 103 949 V1.1.1 (https://www.etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf);
- the 5GAA/GSMA report (the same PDF as `refine-pqc.md`);
- `research/papers/` copies of twardokus2024cryptography, boldyreva2024provable, abrar2026deployment, mamun2026post, karinsalo2026migrating and chen2026pseudonym.

Searches: arXiv API, OpenAlex, IACR ePrint and web search. Pages are PDF page numbers. A source marked "secondary" was not read in the original.

## Verdict

**The ordering itself is generic, and much of the V2X version is already published.**
- NIST's draft already says to migrate key establishment first because of harvest-now-decrypt-later, and that authentication can stay classical until a quantum computer exists (§1).
- For V2X, three papers already publish an order:
  - twardokus2024cryptography gives a phased "era" plan in which ECDSA BSM signatures stay classical under a partially hybrid certificate cycle (§2);
  - fikry2026post gives a selective SCMS: PQ for CAs, enrolment certificates and Kyber session keys, with pseudonyms and CAMs left on ECDSA;
  - chen2026pseudonym gives a PQ-CA / ECC-pseudonym hybrid.

  "Infrastructure first, BSM signatures last" therefore cannot be a contribution.

**What is novel, as far as the searches reached, is narrower and sharper.**
- **The asset.** In V2X the harvest-now asset is *pseudonym unlinkability*, specifically the RA/ACA split of knowledge in butterfly provisioning. The two V2X migration documents state the opposite: ETSI TR 103 949 rates disclosure impact "None per se", and 5GAA/GSMA says HNDL "is not the main concern for V2X".
- **The consequence.** This makes the *provisioning path* the first thing to migrate, at zero over-the-air cost.
- **Prior art.** No source was found that names this mechanism.
  - abrar2026deployment and fikry2026post mention HNDL for V2X, but only loosely (§2).
  - The phrase "harvest now, link later" had no hits on arXiv, OpenAlex or the web.

## 1. Generic guidance: what each source says about order and dates

| Source | Status | Order it gives | Dates |
|---|---|---|---|
| NIST IR 8547 | **Still an initial public draft** (12 Nov 2024; comments closed 10 Jan 2025; no final version linked on https://csrc.nist.gov/pubs/ir/8547/ipd as of today) | "Unlike with encryption, where there is a threat of 'harvest now, decrypt later,' an authentication system remains secure as long as [it is] secure when the authentication is performed" (pp. 14–15). "NIST expects to prioritize the migration to quantum-resistant key-establishment schemes … to protect against 'harvest now, decrypt later'" (p. 24) | ECDSA: 112-bit deprecated after 2030; **≥128-bit (P-256) disallowed after 2035**, with no deprecation step (Table 2, pp. 20–21) |
| NCSC UK, *PQC migration timelines* (20 Mar 2025) | Guidance | Discovery and plan, then priority systems. It notes that PKI authentication will follow more slowly than confidentiality. It does not frame KEM-versus-signature ordering explicitly | 2028 / 2031 / 2035 (https://www.ncsc.gov.uk/guidance/pqc-migration-timelines) |
| IETF draft-ietf-pquip-pqc-engineers | The datatracker reports it as RFC 9958 (Informational, Jun 2026); not read beyond the datatracker summary | HNDL applies to confidentiality. Start with hybrid key exchange, which is self-contained; signatures need ecosystem change | none (https://datatracker.ietf.org/doc/draft-ietf-pquip-pqc-engineers/) |
| NSA CNSA 2.0 | Secondary sources only | By *product class*, not KEM-first. Software/firmware signing comes **first** (prefer 2025, exclusive 2030), then networking equipment (2026/2030), then browsers/cloud and OS (2033) | Secondary: https://postquantum.com/quantum-policy/nsa-cnsa-2-0-pqc/ |
| EU NIS CG coordinated roadmap (23 Jun 2025) | Recommendation | Ordered by risk: start by end-2026, high-risk use cases by end-2030, the rest as far as feasible by 2035 | https://digital-strategy.ec.europa.eu/en/library/coordinated-implementation-roadmap-transition-post-quantum-cryptography |
| BSI TR-02102-1 v2026-01 | Secondary only | Hybrid is required. **Key agreement comes before signatures**: classical key agreement alone is not recommended after 31 Dec 2031, classical signatures after 31 Dec 2035 | https://postquantum.com/security-pqc/bsi-tr-02102-cryptographic-guideline-update/ |
| CISA/NSA/NIST factsheet (Aug 2023) | Only the landing page was read | Roadmap, inventory, vendors. No order confirmed | https://cisa.gov/resources-tools/resources/quantum-readiness-migration-post-quantum-cryptography |
| NIST SP 1800-38 | Not checked in this pass | n/a | n/a |

**Takeaways.**
- "Key establishment first, signatures later, because of HNDL" is the *default* in NIST, IETF and BSI. A thesis that says only this, applied to V2X, adds nothing.
- CNSA 2.0 puts long-lived *code-signing* roots first. That is the generic basis for "infrastructure first". It is also generic.

## 2. V2X-specific landscape

| Source | Proposes an order or phases? | Privacy or unlinkability as the asset at risk? |
|---|---|---|
| **ETSI TR 103 949 V1.1.1 (2023-05)**, ETSI's ITS/C-ITS QSC migration study | It applies the TR 103 619 process: inventory, plan, execute. It says "every asset used in the trust model needs to be migrated". It advises **against hybrid** "subject to more detailed analysis" (p. 18). Rollout should "begin at the bottom of the hierarchy and [be] confirmed prior to switchover" of the Root CA (p. 23). That is end-entity capability first, then CA switch, which is the reverse of naive "infrastructure first" | **Explicitly no.** Table 3: data is protected "by use of pseudonymous keying … rather than by encryption"; disclosure impact "None per se"; protection duration "N/A" (p. 16). It does say privacy must last "the lifetime of the ITS-S" (Table 2, p. 16). It never connects this to provisioning |
| **5GAA/GSMA, *PQC for Automotive – Impact Assessment*** (Oct 2026, 35 pp.; title confirmed on p. 1) | It gives deployment dates: high-priority systems about 2030, all about 2035 (§6; see `refine-pqc.md` §1). Industry **declines to publish order**: public material should avoid "what will be fixed first" (p. 17), and OEM prioritisation is internal (p. 18). It suggests a symmetric plus periodic-signature scheme for frequent messages (§5.2.4) | **No.** HNDL "is not the main concern for V2X" because safety data is short-lived and not confidential (p. 10). Privacy appears only as a generic consequence category (p. 16). It never mentions SCMS, butterfly keys or provisioning (`refine-pqc.md` §1) |
| **twardokus2024cryptography** (NDSS 2024) | **Yes, a V2V phase plan.** "Roadmap for a PQ Transition in V2V". Era A_v: ECDSA stays secure while certificates live fewer than v hours, about 14 years from 2024, with a partially hybrid design. Era B: fully hybrid on new hardware. Era C: ECDSA disallowed (pp. 2, 5) | No. Message layer only |
| **fikry2026post** (IEEE ASSIC, Aug 2026; abstract via OpenAlex, DOI 10.1109/assic70037.2026.11717405) | **Yes, a selective SCMS.** Dilithium for enrolment and CA certificates and for DENMs; ECDSA kept for pseudonyms and CAMs; Kyber for session keys. Motivated by HNDL on 15-year enrolment certificates | No. Its HNDL argument concerns the forgery lifetime of signatures, not unlinkability |
| **chen2026pseudonym** (IEEE CJECE 2026) | PQ CA hierarchy with hybrid pseudonym certificates. Cites the EU roadmap (p. 2) | Claims unlinkability of the *new* scheme; no retroactive analysis |
| **abrar2026deployment** (p. 3) | No order | It mentions HNDL plus vehicular "privacy" in one sentence, and conflates it: "record signed … transmissions today and decrypt or forge them". BSMs are plaintext, so nothing in them is decrypted. No mechanism is given |
| **mamun2026post** (Vehicular Communications 2026) | A phased *research-gap* roadmap: short, medium and long term, with SCMS redesign mid-term (pp. 47–48) | HNDL only generic, on "archived vehicular data" (p. 12) |
| **karinsalo2026migrating** (ICCWS 2026) | "Phased, with initial deployment in non-critical components"; hybrid is transitional (pp. 6–7) | No |
| **chouhan2026quantum** (Zenodo; landing page only) | A "phased roadmap from 2024 to 2035+" and use-case priorities. Which item comes first is not visible in the abstract | No privacy, pseudonym or HNDL terms on the record page (https://zenodo.org/records/19488046) |
| **lohmiller2025survey** (IEEE Access) | A survey of migration state; abstract only. No order is claimed in the summary | Not stated |
| **arXiv 2601.11104** | Generic X.509 PQ-PKI strategies (`refine-pqc.md` §1) | No |
| **boldyreva2024provable** (PoPETs 2024) | n/a | **This is the formal hook.** BKM privacy against the RA relies on ECIES security under Hash/Oracle Diffie–Hellman (Thm 6.1 p. 11; Thm 7.2 p. 13). The authors leave "post-quantum security of the protocol" as future work (p. 13) |
| ePrint 2024/946 (PQ BKE) | n/a | It proves unlinkability for *new* PQ BKE. The abstract says nothing about the classical scheme's retroactive exposure (https://eprint.iacr.org/2024/946) |
| IEEE 1609, C2C-CC, CAMP, SAE | No public PQC document was found for 1609.2 (only the 802.11bt PAR), for C2C-CC, or for CAMP (one search each). 5GAA says 1609 "is starting" PQC extensions (§6). The J3161/J2945-1 limits are in `refine-pqc.md` | — |

## 3. Why the SCMS makes the privacy asset concrete (all from `docs/design/scms-real/`)

- **What travels on the provisioning path.**
  - `EeRaCertRequest` is signed with the enrolment key and encrypted to the RA's ECIES P-256 key. It carries caterpillar keys A and H and the AES expansion keys ck and ek (`02-cryptography-and-formats.md` §9.1; `03-protocols-and-operations.md` item 2).
  - ACA responses are ECIES-encrypted to the cocoon keys (same sources).
  - Downloads run over HTTPS with TLS 1.2 or 1.3, with a `Download-Req` header signed by the enrolment key (`03-…` items 3 and 6).
- **What the RA holds by design.**
  - It knows the device identity and every (i, j) (`02-…` §9.5).
  - It pre-generates and stores up to **3 years** of encrypted batches (`03-…` item 5 and the l.248/l.263 rows).
  - It cannot read the certificates only because of ECIES/(EC)DH (boldyreva2024provable).
- **Two adversaries.**
  - **Outsider recording the network.** A future CRQC breaks TLS ECDHE and then ECIES. That maps the enrolment identity to the full certificate batch, i.e. every pseudonym the car will broadcast over the period.
  - **The RA, or anyone holding its archive.** With a CRQC it decrypts its own stored batches and breaks the RA/ACA split retroactively.
- **What a broadcast-only adversary gains: nothing obvious (this is an inference to prove).**
  - Butterfly expansion is AES-based (`02-…` §9.2), and linkage values are AES/SHA-based (§10.1).
  - PLVs travel AES-CCM-encrypted under an LA–ACA shared key (§10.3). That is Grover-only, *unless* that key was agreed with ECDH, which the standard leaves out of scope.
  - So the retroactive exposure is concentrated in provisioning. The thesis must state this as a claim and prove it.
- **The cost asymmetry that makes the order non-trivial.**
  - A PQ or hybrid KEM on the provisioning path costs nothing over the air.
  - A PQ issuer signature on pseudonym certificates (the chen2026pseudonym or fikry-style "infrastructure first") is carried in every over-the-air certificate. ECQV implicit certificates have no PQ equivalent (`02-…` §6).
  - Separately, ETSI's bottom-up rule means CA signatures cannot switch until receivers can verify them.

## 4. Novel versus generic

**Generic, so cite it rather than claim it:**
- KEM before signatures because of HNDL (IR 8547; IETF; BSI);
- long-lived roots and code signing first (CNSA 2.0);
- risk-tiered dates of 2030 and 2035 (EU, NCSC, 5GAA);
- classical BSM signatures under short-lived certificates with partial hybrid (twardokus2024cryptography);
- selective PQ for CA and enrolment certificates with ECDSA pseudonyms (fikry2026post; chen2026pseudonym);
- phased V2X roadmaps (mamun2026post; chouhan2026quantum).

**Novel (no prior statement found; claim it with "to our knowledge"):**
1. **The asset.** Pseudonym unlinkability is a harvest-now asset in V2X, which contradicts ETSI TR 103 949 Table 3 and 5GAA/GSMA p. 10.
2. **Where it sits.** The asset is concentrated in the butterfly-provisioning path, and is quantum-retroactive against both a network recorder and the RA's own archive. Broadcast traces alone are not (to prove).
3. **The outsider/insider split.**
   - Hybrid TLS 1.3 at RA endpoints closes the outsider case without any 1609.2.1 change. This is cheap and generic, so say so.
   - Only a PQ or hybrid recipient type for `AcaEeCertResponsePrivateSpdu`, or a PQ BKE, closes the RA-archive case. That is a concrete 1609.2.1 ask.
4. **System-scale quantification** of exposure (linkable pseudonym-weeks per vehicle per year of migration) jointly with channel break-points. No V2X paper does both.

**Weak or untestable as now worded:**
- "Order matters more than algorithm" is rhetoric. Turn it into a test: across families P0–P3 and signature schemes, privacy exposure depends only on the provisioning-KEM year, and the signature choice moves only channel break-points.
- "Closes the exposure N years earlier" is wrong in form. Archives recorded before migration stay linkable for good, and batches already downloaded stay valid up to 3 years. Report **the year accumulation stops** and **the cumulative exposed pseudonym-years avoided**.

## 5. Defensible phrasing

> Post-quantum guidance prioritises key establishment over signatures because of harvest-now-decrypt-later [NIST IR 8547 ipd; RFC 9958]. V2X migration studies conclude that this threat does not apply to V2X, because safety messages are unencrypted and short-lived [ETSI TR 103 949; 5GAA/GSMA 2026]. We show that it does apply, to a different asset: pseudonym unlinkability. In the US SCMS, the RA/ACA separation that keeps pseudonyms unlinkable to enrolment rests on (EC)DH-based ECIES and TLS [Boldyreva et al. 2024]. Provisioning traffic recorded today, or batches archived by an RA, therefore let a future quantum adversary map up to three years of a vehicle's pseudonyms to its enrolment certificate, while broadcast traffic alone does not. We quantify this exposure and its cost under four provisioning families at city scale. Migrating the provisioning path first costs no channel capacity and stops new exposure from accumulating years before signature migration. BSM signatures can then follow the partially hybrid cycle of [Twardokus et al. 2024].

**Rules for the text.**
- Never write "first migration order for V2X". Cite twardokus2024cryptography, fikry2026post, chen2026pseudonym and mamun2026post as prior orders.
- Do not call the HNDL-privacy link wholly new. abrar2026deployment p. 3 gestures at it, so say "first to identify the mechanism and quantify it".
- Say "draft" whenever IR 8547 is cited. P-256 is disallowed after 2035 with no deprecation step.

## 6. Corrections to `refine-pqc.md` / `research-plan.md`

- **Add fikry2026post as prior art** (it is missing from both). It weakens "infrastructure-first" further than chen2026pseudonym alone. Its "N=1000, 50 vehicles, DSRC" capacity numbers are a baseline to beat.
- **twardokus2024cryptography is already a V2X phase plan.** H4's "reverses the message-first emphasis" should read "extends the era-A_v plan to the provisioning layer".
- **Explain why "infrastructure-first" differs from ETSI's bottom-up rule** (p. 23), and that a PQ PCA signature has a channel cost, unlike the provisioning KEM. Profile P0 must carry that cost.
- **Add an arm "P3-TLS only"** (hybrid TLS 1.3 at the RA, inner ECIES unchanged) to separate the generic fix from the 1609.2.1 change.

## 7. Still open

- Read the full texts of chouhan2026quantum, lohmiller2025survey, fikry2026post and NIST SP 1800-38 to check for any privacy or provisioning ordering.
- Confirm whether US pseudonym certificates are implicit in the SCMS Manager profile.
- Confirm how the LA–ACA PLV key is agreed in deployed SCMSs.
- Write the proof sketch that a broadcast-only CRQC adversary gains no linkage.
