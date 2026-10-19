# Final verdict: "harvest now, link later" in the US SCMS

Date 2026-10-18. Final arbiter over round 1 (`r1-arbiter.md`, which merged a blind cryptanalysis, a prior-art hunt, a fact check and two reviews) and round 2 (`r2-review-privacy.md`, `r2-review-industry.md`, both written from the r1 revised claim only). All files read in full. Own checks: one primary-source fetch and one URL probe (below).

## 1. Overall verdict: HOLDS WITH CHANGES

- **Scores.** All four reviewers (r1 crypto, r1 systems, r2 privacy, r2 industry) gave **Major revision, confidence 4/5**. None recommends rejection. r1 systems "would expect to support acceptance" once fixed (r1-review-systems.md §11). r2 industry "would like to see it published" (r2-review-industry.md §10).
- **Core mechanism: no input refutes it.** A CRQC turns stored provisioning confidentiality (request + download, backend links, or RA state) into the RA–ACA collusion that butterfly privacy assumes away.
  - Corrupted-RA privacy rests on Hash-DH (Boldyreva–Kumar–Sun, PoPETs 2024(4), doi:10.56553/popets-2024-0130, Thm 5.1/6.1). Both r2 reviewers re-verified this.
  - Broadcast BSMs and certificates give no quantum gain, because the ACA offset hides them (arXiv:1802.05323 p.10). Both r2 reviewers agree; the hiding is statistical in the ROM (`h = H(r‖pk)`, BKS Fig. 10).
- **What does not hold as written:**
  - the novelty wording ("first to identify the mechanism");
  - the Chen et al. framing;
  - fleet-wide O(1) numbers presented as fact;
  - an evaluation that counts certificates instead of measuring privacy loss.

## 2. Arbiter rulings on round-2 disputes

| # | Point | Ruling | Basis |
|---|---|---|---|
| R1 | r2-privacy W1: "downloads alone give a quantum gain, one discrete log per response ephemeral key" | **Rejected for the 1609.2 encoding.** Shor on the ephemeral V gives v, but the ECIES secret is v·J_ij. The download carries only `recipientId HashedId8` of J_ij, not J_ij itself. | Own check: IEEE 1609.2 ASN.1, `Ieee1609Dot2.asn` (ETSI Forge, `master`, fetched today) l.629–632 `PKRecipientInfo ::= SEQUENCE { recipientId HashedId8, encKey … }`; r1-blind §2.3. BKS Fig. 10 abstracts this detail away. |
| R1′ | Same point, other vantage points | **Accepted in part.** J_ij is visible on RA→ACA (`certEncKey`) and in RA state, so an alternative per-certificate count, min(1 per vehicle via H, 1 per certificate via J_ij or V), applies to paths (b) and (c). Still check the CUBK and unified variants. | r1-blind §2.4 l.161–163; r1-arbiter §5 item 7 |
| R2 | Chen et al. 2026 already state the dependency and use a PQ KEM | **Accepted. Novelty narrowed.** | arXiv:2606.14008 p.6, pp.10–11 (§III.B.2), p.17; doi:10.1109/icjece.2026.3692881 |
| R3 | Boldyreva attribution | **Accepted.** Corrupted-RA (Thm 5.1) rests on Hash-DH under related-key attack plus robustness. The secure-channel premise belongs to the outsider and corrupted-ACA cases. Map each path to the theorem it voids: (c) → Thm 5.1; (a)/(b) → channel premise. BKS has no LAs, so the LCI and PLV paths lie outside every published proof. | r2-industry W2 (pp.3, 7); r2-privacy S1 (p.566, 574) |
| R4 | Misbehaviour reports: pseudonym-signed (Brecht p.16) vs enrolment-signed | **Not a contradiction, but unverified.** Brecht describes the *inner* report (pseudonym-signed, encrypted to the MA). The SCMS Manager CV-Pilot MBR spec v1.0 §4.0 describes an *outer* `Download-Req` header signed by the enrolment certificate. Keep (d) conditional on that 2019 pilot spec and confirm it against current specs. | r1-blind §2.6, citing scmsmanager.org MBR spec v1.0 §3.0/§4.0; arXiv:1802.05323 p.16 |
| R5 | LCI = E(pk_LA, seed) is only an "e.g." | **Accepted by every round.** O(1)-per-fleet is a *scenario*. If the standard leaves it open, the result becomes a recommendation. | arXiv:1802.05323 p.13; `LaRaInterfacePdu ::= NULL` (1609.2.1 ASN.1, r1-arbiter l.15) |
| R6 | Top-ups vs lifetime auto-issuance from one request | **New and accepted.** Model both behaviours. Lifetime issuance strengthens the case for forced re-keying. | arXiv:1802.05323 p.7, p.15 |
| R7 | PQ.08 "cannot be found" | **Keep, with care.** Two r1 agents quoted the full PDF independently (r1-fact-check (h); r1-prior-art l.138). Today the landing page returns HTTP 200 and the PDF endpoint `sdc_download/9673` returns HTTP 500 to a script. Archive a copy now and cite the landing page plus §3.3. | https://5gaa.org/post-quantum-cryptography-for-automotive-impact-assessment/ |
| R8 | Adversary model, metric, baseline, circular validation | **Accepted (both r2 reviewers).** | r2-privacy W3–W5; r2-industry W5, W7 |
| R9 | Which mitigation comes first | **No fixed order is claimed.** Hybrid TLS on the access path is near-free (r2-privacy W7). For a carrier adversary its increment may be small (r2-industry §6). The data decides. | — |

## 3. Final claim (to write from)

> In the US SCMS (IEEE 1609.2.1 butterfly key expansion), pseudonym unlinkability is a confidentiality asset with a shelf life of vehicle lifetime plus data retention. Broadcast BSMs, certificates, CRLs and CA records give a future quantum adversary no linkage (statistical hiding by the ACA offset), and neither do requests alone or downloads alone in the 1609.2 encoding. Linkage requires a stored provisioning flow or state carrying caterpillar keys, offsets, linkage seeds or pre-linkage values.
>
> We give a classical-now/quantum-later privacy game extending Boldyreva–Kumar–Sun and use it to:
> - enumerate the sufficient paths for three named adversaries (access-network recorder, backend recorder, future holder of an RA archive) with discrete-log costs per path;
> - give a fleet-constant cost under explicitly parameterised deployment choices (LCI wrap, LA–ACA key establishment, TLS termination, retention, top-up behaviour);
> - show that migration must force fresh caterpillar keys and re-seed linkage chains.
>
> We quantify, on realistic mobility against a strong classical tracker, the privacy loss already accumulated before migration and the share of it removable by server-side fixes alone. We compare this with ETSI TR 103 949 and 5GAA/GSMA, which treat V2X confidentiality as quantum-uncritical, and with Chen et al. 2026, who fix the channel but do not analyse retroactive exposure.

**Novelty statement:** "first to analyse and quantify retroactive (store-now) linkability of SCMS pseudonyms, including the backend and linkage-plumbing paths". Not "first to identify the mechanism", and not a migration order.

**Earlier phrasing to retire:** "unlinkable only while the certificate download stays secret". The download alone does not suffice (R1).

## 4. What each round changed

| Round | Change |
|---|---|
| R1 | Dropped "linkage values remain safe" (the LCI/PLV containers may be public-key). Added misbehaviour-report, backend byte-matching and single-key-response paths. Added discrete-log counts as the headline axis. Hybrid TLS narrowed to "access-network only, end-to-end". PQ response encryption is "necessary, not sufficient". Forced re-key and re-seed added. Asset order replaced by a per-key inventory. Airtime dropped as a cost. Classical-tracker increment added (r1-arbiter §2–§4). |
| R2 | Chen et al. credited; novelty narrowed again. Boldyreva re-attributed per theorem. Formal quantum-later game required (Bindel et al., doi:10.1007/978-3-319-59879-6_22; doi:10.1007/978-3-030-25510-7_12). Archive path (c) separated as compromise or compelled disclosure. Named adversaries with the same-adversary baseline (telematics, ALPR, carrier location). Established privacy metric (Wagner & Eckhoff, doi:10.1145/3168389). Headline moved to accumulated exposure plus the share fixable server-side for the legacy fleet. Lifetime auto-issuance added. Ghosh–Kate (ACNS 2015) and Grubbs–Maram–Paterson (ePrint 2021/708) to cite. |
| Final | "Downloads alone" error rejected via the 1609.2 ASN.1 (R1). The per-certificate count kept for backend and archive (R1′). MBR conflict reframed as inner vs outer layer (R4). |

## 5. Remaining objections and answers

| Objection | Answer |
|---|---|
| "Obvious: Shor breaks ECDH, use hybrid TLS" | TLS cannot fix the RA-archive and LCI/PLV paths or the exposure already harvested. Re-seeding is non-obvious and costly. Two standards documents classify the asset the other way (ETSI TR 103 949 Table 3, p.16; 5GAA/GSMA §3.3). |
| "Chen et al. already did it" | They fix the channel. They give no store-now shelf life, no backend/LCI/MBR paths, no counts, no re-keying requirement and no measurement (r2-industry W1). |
| "Your adversary can already track these cars" | Report the increment per named adversary: cross-gap and cross-week linking, binding to enrolment and VIN, and adversaries without carrier access. Concede where telematics dominates (r2-privacy W4). |
| "The O(1) results rest on an e.g." | Present them as scenarios. Verify against 1609.2.1-2022/2026 and the CAMP and SCMS Manager specs. If open, recommend a symmetric or PQ wrap as a standards change. |
| "Validation is circular" | Closed form for counts. The simulator only for mobility, the tracker baseline and fleet mix. Encodings sourced from the 1609.2.1 ASN.1. Sensitivity analysis instead of agreement checks. |
| "RA and ACA are one provider anyway" | Report the outsider-who-gets-the-RA-store case and the split-operator case separately (r2-industry W6). |
| "PQ butterfly fixes it" | It makes path (c) computational again (MLWE-type). State that assumption and its shelf life (r1-blind §5; r2-industry W8). |

## 6. Residual risk

- **Novelty: MEDIUM.**
  - Chen et al. are one paragraph away from the store-now framing.
  - The PQ-BKE authors (ePrint 2024/946) and qSCMS (2018/1247) were read in full only through Wayback by r1-prior-art; both r2 reviewers saw abstracts only.
  - Fikry et al. 2026 was never found (r1-prior-art l.158).
  - No reviewer's search found a paper quantifying store-now SCMS linkage (r2-privacy §4; r2-industry W1).
- **Correctness of the core: LOW.** Four reviewers and a blind derivation converge, and the one r2 error claim was checked against the ASN.1 and rejected.
- **Correctness of the headline numbers: MEDIUM-HIGH.** They depend on unverified deployment facts:
  - LCI wrap;
  - LA–ACA key establishment;
  - TLS termination (LOP, load balancer, OEM back end);
  - RA retention;
  - MBR outer signature;
  - top-up behaviour;
  - the CUBK offset.
- **Significance of the harm: MEDIUM-HIGH.** The classical tracker and the same adversary's telematics may absorb most of the increment (r2-privacy W4; r2-industry W5). Unknown until measured.

## 7. Venue

- **Primary: PoPETs.** It is a privacy result, and BKS published there. Target a 2027 issue deadline; check the dates on petsymposium.org, since earlier dates were inferred.
- **VehicleSec '27:** only for a separately scoped short paper, such as the standards and operator recommendations. Check dual-submission rules first, because both venues have proceedings.
- **NDSS 2028:** not recommended unless the formal game, the measurement and the deployment verification are all complete.
- **arXiv:** an early note is reasonable *after* sections 2–3 are fixed. Confirm that the PoPETs policy allows it.

## 8. Concrete next steps before writing

1. **Verify the deployment formats** against IEEE 1609.2.1-2022 ASN.1 (ETSI Forge) and the 1609.2.1-2026 text, with the CAMP EE/RA/LA specs and SCMS Manager specs as fallback:
   - LCI encryption;
   - LA–ACA key establishment;
   - MBR outer header;
   - permitted EE–RA transports and TLS termination;
   - top-up versus lifetime issuance;
   - the CUBK and unified offset.

   Label every unverifiable path as a scenario.
2. **Read in full:** ePrint 2018/1247, 2024/946 and Chen et al. §III.B.2 and p.17. Search again for Fikry 2026. Archive the 5GAA/GSMA PDF.
3. **Write the classical-now/quantum-later privacy game** (extending ePrint 2024/1674 with LAs). Prove "no gain" for BSMs, requests and downloads, and "sufficient" for each path.
4. **Build the key and flow inventory and the closed-form exposure model** (r1-arbiter E1–E2), with the per-certificate alternative count (R1′) and both issuance behaviours (R6).
5. **Define the metric and three adversaries.** Pick a mobility dataset and a classical tracker baseline (Wiedersheim, doi:10.1109/WONS.2010.5437115; Petit, doi:10.1109/COMST.2014.2345420).
6. **Only then scope the engine build.** Simulator work is for mobility, the tracker and fleet mix, not for re-deriving counts. Add fault-injection toggles per deployment parameter.
