# Review (Reviewer C, privacy / PoPETs PC)

**Submission under review:** "Harvest-now, link-later: post-quantum unlinkability of SCMS pseudonym provisioning" (thesis direction, claims (1)–(4) as stated in the task).
**Reviewer profile:** location privacy, pseudonym schemes, tracking attacks.
**Sources I read myself:** listed inline. I could not open ePrint 2018/1247 or 2024/946 in full (HTTP 403 for scripted access), and I found no copy of the 5GAA/GSMA "PQ.08" document. Statements about those three rest on abstracts only, and I say so where it matters.

---

## 1. Summary

The authors argue that in the US SCMS, *forgery* resistance and *unlinkability* have different post-quantum (PQ) lifetimes. Butterfly key expansion hides each pseudonym certificate behind a fresh ACA offset, so BSMs, CRLs, ACA records, or an EE→RA request on its own give a CRQC adversary nothing. Unlinkability does break, retroactively, if the adversary has stored any flow that carried the offsets/certificates, linkage seeds, PLVs or caterpillar keys under classical public-key confidentiality. The authors list four paths (access-network recorder, backend recorder, RA archive, misbehaviour-report uploads) with discrete-log (DL) counts. They conclude that the confidentiality keys (TLS key exchange, RA request key, response encryption, MA key, LCI encryption, LA–ACA key establishment) need PQ deadlines set by vehicle lifetime plus retention. Signature migration runs separately. They contrast this with ETSI TR 103 949, a 5GAA/GSMA document and Chen et al. 2026, and they propose a closed-form exposure model checked against a city-scale simulator. The model would measure "linkable pseudonym-years" per path, DL budget, migration schedule and provisioning variant, together with the cost of each fix.

## 2. Strengths

- **S1. The central observation is correct and useful.** I checked the strongest privacy result for the IEEE 1609.2.1 BKM. Privacy against a corrupted RA (Boldyreva–Kumar–Sun, PoPETs 2024(4), Thm 5.1, p.574) is proven under two assumptions:
  - "communication between EEs and the RA is private and authenticated" (§1.2, p.566);
  - ECIES being IND-CPA under additive related-key attack from the **Hash Diffie-Hellman** assumption (Thm 6.1, summarised p.567).

  A CRQC voids the second outright, and voids the first for any recorded ECDHE channel. "The RA's records alone suffice later" therefore follows directly from the published proof's assumptions. Nobody seems to have spelled it out for the store-now adversary, and the authors do (claim 1c). PDF: https://crysp.petsymposium.org/popets/2024/popets-2024-0130.pdf
- **S2. The O(1) RA-archive path is the most interesting technical point.** The SCMS design paper defines an LCI as "the initial linkage seed … that LA1 … encrypts to itself, e.g., E(pk1, ls1(0)), where pk1 is the public key of LA1" (Brecht et al., IEEE T-ITS 2018, DOI 10.1109/TITS.2018.2797529, arXiv 1802.05323, §IV, p.13, Fig. 4). The RA holds the LCIs per device. Two DLs (one per LA key) applied to an RA database would therefore give every device's seed, every linkage value, and the RA's own map from LCI to enrolment certificate. No responses and no network capture are needed. If confirmed against the deployed implementation, this alone is a privacy result worth publishing.
- **S3. The claim is scoped honestly.** It drops the "confidentiality before authentication" principle and PQ butterfly as contributions. That is appropriate.
- **S4. The ETSI contrast is real.** ETSI TR 103 949 V1.1.1 (2023-05), Table 2 (p.16), says "Privacy of the user is to be maintained across the lifetime of the ITS-S". Table 3 (p.16) gives confidentiality "Protection duration: N/A" and disclosure impact "None per se", with privacy "achieved by use of pseudonymous keying … rather than by encryption". §4.4 (p.14) frames the CRQC risk purely as "masquerade (impersonation attack)". The authors' reading is accurate (https://www.etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf).
- **S5. The required re-keying is easy to miss.** Migration must force new caterpillars and re-seed the linkage chains, otherwise a recorded classical request keeps future certificates exposed. I have not seen this stated elsewhere.

## 3. Weaknesses

### W1. A factual error in claim (1): "downloads alone give no quantum gain" is false

Each ACA response is ECIES-encrypted and signed by the ACA (BKS Fig. 10, p.573: `c_i ← Enc(epk_cc[i], pk_bf ∥ cert ∥ r_i)`, `rsp[i] ← c_i ∥ σ_i`). A CRQC can take the DL of each ciphertext's **ephemeral** key and decrypt it without the cocoon private key. This needs no request and no caterpillar key.

So a recorder of downloads only, after one DL per TLS session (zero if downloads are not inside TLS, since the payload is already ECIES-protected), gets the following for one DL per certificate:
- every certificate in the device's batch;
- hence linkage of all of that device's pseudonyms to each other and to the download's network or subscriber identity.

That costs roughly 20 certs × 52 weeks × 3 years ≈ 3,000 DLs per vehicle-batch, instead of about 1 via the caterpillar key. It is a real quantum gain, and for a targeted adversary it is the cheaper *precondition* (downloads recur; the initial request may happen at the plant, outside an access-network recorder's view).

The same per-ciphertext path also applies to RA archives and RA–ACA recorders. It does not change the fix list, because PQ response encryption is already there. It does change:
- the "sufficient paths" taxonomy;
- the "one per TLS session plus one per vehicle" count, which should be "min(1 per vehicle given the request, 1 per certificate without it)";
- every exposure number in claim (4).

The claim must be corrected before any number is computed.

### W2. Claim (3) misrepresents Chen et al. 2026, and that weakens the novelty framing

Chen et al. (arXiv 2606.14008; IEEE Canadian J. ECE 2026, DOI 10.1109/ICJECE.2026.3692881) do **not** rate "one-week pseudonym certificates quantum-resistant". The passage (p.9, §III.A) says that *if* breaking P-256 ECDLP took about one week, a pseudonym validity of **one day** would expire first. This concerns the ECC holder key in a hybrid certificate, i.e. forgery.

More importantly, Chen et al.'s scheme already moves EE→RA, EE→PCA and PCA→EE confidentiality to a PQ KEM. Their privacy argument (p.10–11, §III.B.2) is explicitly conditional on that: "as long as the PCA cannot break either the PQC-based KEM or AES … it will be unable … to determine any relationship between the enrollment public key and the DSA-based public key". It concludes that no party "can infer the connection".

So prior work already:
- treats the provisioning encryption as the carrier of unlinkability;
- makes it PQ.

What Chen et al. lack is the *retroactive* (store-now) framing, the path enumeration, the RA-archive/LCI analysis and any measurement. Barreto et al.'s qSCMS (ePrint 2018/1247, abstract) likewise motivates PQ butterfly by quantum threats to SCMS. From the abstract alone I cannot tell whether it discusses passive retroactive linkage; the authors must read it in full.

The paper must restate (3) accurately. "Three bodies hold the opposite view" becomes "two standards documents treat confidentiality as irrelevant; academic PQ designs fix the channel without analysing retroactive exposure or the backend."

### W3. The adversary model is informal and mixes three different adversaries

These are bundled under "passive store-now":
- (a) a network recorder;
- (b) a backend-link recorder;
- (c) a future holder of an *at-rest database* (breach, insider, or legal compulsion of the RA operator).

(c) is not harvest-now-decrypt-later in the usual sense. It is "data retained today, CRQC tomorrow", and its likelihood is governed by retention policy and operator trust, not by traffic interception. For PoPETs I expect a formal game: extend BKS's privacy experiments with an adversary that is classical during the protocol and quantum afterwards, given a specified transcript or state subset.

That "classical-now, quantum-later" adversary already exists in the hybrid-KEM/AKE literature:
- Bindel, Herath, McKague, Stebila, PQCrypto 2017, DOI 10.1007/978-3-319-59879-6_22;
- Bindel, Brendel, Fischlin, Goncalves, Stebila, PQCrypto 2019, DOI 10.1007/978-3-030-25510-7_12.

With it, "sufficient" and "no gain" become theorems rather than assertions. Without it, claims like "requests alone give no quantum gain" rest on informal reasoning. W1 shows that kind of reasoning can miss a path.

### W4. The privacy harm is not yet put in context, and "linkable pseudonym-years" is not a privacy metric

**Classical baseline.** Linkage is only harmful relative to what the adversary already has.
- SCMS reuses ~20 certificates per week. A BSM archive therefore already links reuse of the same certificate across days within a week, and trajectory tracking defeats pseudonym change in all but dense traffic (Wiedersheim et al., WONS 2010, DOI 10.1109/WONS.2010.5437115; Petit et al., IEEE COMST 2015, DOI 10.1109/COMST.2014.2345420).
- The quantum increment is mainly cross-week and cross-parking linkage, plus binding to an enrolment certificate or subscriber.
- The proposal mentions "increment over a classical trajectory tracker". The baseline must also include certificate reuse within the week.

**Same-adversary baseline.** Path (a) assumes a cellular access-network recorder with a CRQC. The same adversary harvests the vehicle's other TCU traffic: OEM telematics, navigation, OTA and remote-app sessions. That traffic often carries GPS fixes under the VIN or account, under the same classical ECDHE. For connected vehicles this may dominate SCMS linkage entirely. The paper must argue where SCMS linkage adds information, e.g.:
- vehicles without telematics;
- the archival BSM fidelity (10 Hz, every trip);
- third parties that hold BSM archives but not telematics, such as RSU and DOT operators.

**Metric.** A count of linkable pseudonym-years does not tell a PoPETs reader how much location privacy is lost. Use an established metric (Wagner & Eckhoff, ACM CSUR 2018, DOI 10.1145/3168389): fraction of trips or home/work pairs linkable to an identity, tracking success, anonymity-set size, or expected re-identification. Report "pseudonym-years" only as an intermediate quantity.

### W5. The evaluation plan as written would not convince me

Validating a closed-form linkage count against a simulator of the *same protocol* mainly checks that the two implementations agree; it is close to circular. The analytically hard part is deterministic (which flows are recorded × which keys fall); a spreadsheet gives the same answer.

What the simulator could add, and what I would need to see:
- (i) privacy loss on realistic mobility (SUMO city plus, ideally, a real BSM trace such as a model-deployment dataset), with a state-of-the-art classical tracker as baseline;
- (ii) sensitivity to sniffer coverage, the share of provisioning sessions recorded, RA retention period, and the share of devices provisioned in-plant versus over the air;
- (iii) DL budgets tied to cited, current CRQC resource estimates for P-256, swept over time;
- (iv) fix costs in bytes and compute with concrete parameters, e.g. ML-KEM-768 at ~1.1 KB ciphertext per independent per-certificate KEM key, multiplied across ~3,000 certificates per batch.

(iv) is where the variant comparison (original, unified and CUBK butterfly; ACPC; PQ butterfly; independent KEM) is genuinely informative, and it needs no city simulation.

### W6. Several protocol facts need clause-level citations, and at least one appears wrong

- **Misbehaviour reports.** The design paper says reports go "to the MA via the RA", that "the RA which sits behind the LOP will not know the source IP", and that the report includes "the reporter's **pseudonym** certificate and corresponding signature … encrypted by the reporter to the MA" (Brecht et al., §VIII-A, p.16). That contradicts "enrolment-signed header" unless 1609.2.1 adds an outer enrolment signature for the RA. Cite the clause. Otherwise path (d) links reporter pseudonym to report content, not to enrolment, and its harm is much smaller.
- **LCI under the LA's public key.** The design paper writes "e.g., E(pk1, ls1(0))". It is an example, and an LA encrypting to itself would naturally use a symmetric key. S2 depends on what 1609.2.1 and the deployed LA actually do. The O(1) claim must be stated conditionally until verified.
- **TLS on the EE–RA path.** BKS (§3.3, p.568) note that 1609.2.1 specifies TLS 1.2, TLS 1.3 and ISO/TS 21177 as secure-channel options. Clarify:
  - whether downloads are actually inside TLS (W1);
  - whether TLS terminates at a front end (if so, the inner ECIES-to-RA layer is exposed on internal links, and one DL on the RA's static encryption key opens *all* requests, which is O(1));
  - whether TLS 1.2 RSA or static-ECDH key exchange survives on component links (O(1) per server key);
  - whether PSK resumption extends one broken handshake to later sessions.
- **"Information-theoretic" hiding.** In BKS's description of 1609.2.1 the offset is `h_i = H(r_i ∥ pk_cc[i])` (Fig. 10). The hiding is therefore statistical in the ROM, not strictly information-theoretic. Its quantum security rests on hash preimage and Grover bounds, which is fine, but state it precisely. The same applies to the comparison with lattice butterfly in claim (2).
- **The 5GAA/GSMA "PQ.08 (Oct 2026, §3.3)" quotation.** I could not locate it. The only public 5GAA–GSMA item I found is the March 2025 joint statement of intent (https://5gaa.org/content/uploads/2025/03/5gaa-joint-statement.pdf). Give a stable, accessible reference or drop the quotation.

### W7. The fix for path (a) is close to free and already being deployed

Hybrid ML-KEM TLS is a configuration change on the RA's front end. A reviewer will say "(a) is solved by default, and the remainder is a standards nit". The paper's weight must therefore sit on the parts TLS cannot fix:
- (c) RA archive and LCI/LA–ACA keys;
- 1609.2.1 response encryption;
- re-keying of existing caterpillars and chains;
- the exposure already accumulated before migration, which no future change recovers. Quantifying that accumulated exposure is the strongest "measurement" contribution available and should be the headline.

## 4. Novelty assessment (own search)

Searched: ePrint, arXiv, OpenAlex, Crossref and general web, for PQ, SCMS/butterfly, pseudonym unlinkability, harvest-now / store-now-decrypt-later, and V2X privacy.

**PQ butterfly and SCMS designs** (all address the *mechanism*, not retroactive exposure, as far as their abstracts and text show):
- Barreto et al., qSCMS, ePrint 2018/1247; USP thesis DOI 10.11606/t.3.2019.tde-17072019-135843;
- Eaton–Lamontagne–Matsakis, ePrint 2024/946 (PQ BKE with formal unlinkability game; abstract has no store-now or recorded-traffic threat);
- "Implementing Butterfly Key Expansion Using Post-Quantum Algorithms", LNNS 2022, DOI 10.1007/978-981-19-1607-6_45;
- "Code-Based Butterfly Key Expansion for Pseudonymous Certificates", IEICE ComEx 2025, DOI 10.23919/comex.2025xbl0066;
- Chen et al. 2026 (W2).

**Retroactive linkage.** I found **no** paper that:
- analyses the retroactive linkability of already-issued SCMS pseudonyms under a store-now/CRQC-later adversary;
- enumerates provisioning and backend paths with DL counts;
- identifies the LCI/RA-archive path.

General store-now-decrypt-later results for privacy (rather than secrecy) exist in other domains: everlasting privacy in voting, Moran & Naor, CRYPTO 2006, DOI 10.1007/11818175_22; ledger privacy. The authors correctly do not claim the principle.

**Verdict on novelty.** The *instantiation, path analysis and measurement* appear new. The *core observation* that SCMS unlinkability rests on breakable provisioning encryption is a short step from:
- the HDH assumption in BKS Thm 5.1/6.1;
- Chen et al.'s KEM-conditional privacy argument.

A PoPETs PC will see the delta as moderate unless the RA-archive/backend analysis, the formal model and the harm measurement carry real weight. Claiming "first to identify the mechanism" is risky given W2. "First to analyse and quantify retroactive linkability of deployed SCMS pseudonyms" is defensible.

## 5. Technical correctness

| Element | Assessment |
|---|---|
| BSMs, CRLs and ACA records give no quantum gain; the offset hides cert↔cocoon even against unbounded adversaries | **Correct.** Statistical in the ROM (W6). |
| Request alone gives no gain | **Correct** for SCMS (cocoon keys are hidden by the ACA offset). Note: this is butterfly-specific. In EU-style per-certificate authorisation requests the verification key travels in the request, so requests alone would link. The paper should say why SCMS differs. |
| Download alone gives no gain | **Incorrect** (W1): one DL per response ephemeral. |
| (a) cost "one per TLS session + one per vehicle" | Undercounts the alternatives. Add per-certificate decryption; O(1) on the RA request key if TLS terminates early; TLS 1.2 non-ephemeral cases. |
| (b) byte-identical ciphertexts undo the shuffle | **Correct.** The RA forwards the ACA-signed `c_i ∥ σ_i` (BKS Fig. 10). An RA–LA recorder with LCIs alone gives per-device pseudonym linkage if LCIs are public-key encrypted. |
| (c) RA archive: ~1 DL per vehicle if responses retained; O(1) via LCI / LA–ACA | **Correct** given Brecht et al. Fig. 4, but conditional on the deployed LCI and LA–ACA mechanism (W6). |
| (d) MBR | **Doubtful** as stated; the design paper says pseudonym-signed (W6). |
| Linkage chains need ~2^64 Grover | Plausible for 128-bit seeds. Address the truncated linkage-value length when counting candidate seeds. |
| Corrupted-RA proof assumptions removed by a CRQC | **Correct** (BKS p.566, p.567, p.574). |
| Re-keying (new caterpillars, re-seeded chains) is necessary | **Correct** and important. |
| "Signatures do not affect passive linkability" | **Correct** for passive adversaries. Note that a CRQC forger can turn active (e.g. impersonate the RA to an EE); declare this out of scope explicitly. |

## 6. Does the conclusion follow?

Mostly. Given (1) as corrected, (2) follows:
- unlinkability is a confidentiality asset with lifetime-plus-retention shelf life;
- the listed keys are the ones that matter;
- hybrid TLS fixes only the access-network path.

The ordering argument (confidentiality deadlines before, or at least independent of, signature deadlines) holds for privacy. The "two standards bodies concluded the opposite" rhetoric overreaches:
- ETSI TR 103 949 concerns the EU C-ITS model and its CAM/DENM data classes, not SCMS provisioning;
- the 5GAA/GSMA source is unverified;
- Chen et al. do not hold the opposite view (W2).

## 7. Would the evaluation convince me?

Not as planned (W4, W5). It would with:
- a formal store-now/quantum-later privacy game and proofs for the fixed variants;
- a harm metric on realistic mobility against a strong classical baseline (certificate reuse plus trajectory tracking) and against the same adversary's other harvested channels;
- explicit sensitivity analysis over coverage, retention, in-plant versus over-the-air provisioning, and CRQC budget over time;
- concrete byte, compute and re-keying costs per variant.

## 8. Required changes

1. Fix the "downloads alone" error and recompute the path taxonomy and DL counts (W1).
2. Correct the characterisation of Chen et al. and soften the novelty claim accordingly. Read qSCMS and ePrint 2024/946 in full (W2).
3. Give a formal adversary and game; separate the network-recorder, backend-recorder and at-rest-archive adversaries (W3).
4. Use a recognised privacy metric with classical and same-adversary baselines (W4).
5. Cite 1609.2.1 or CAMP clauses for: TLS coverage of downloads, the MBR signing layer, LCI construction, LA–ACA key establishment and RA retention (W6). Make O(1) claims conditional until verified.
6. Make the accumulated pre-migration exposure the headline result; treat path (a) as the easy case (W7).
7. Replace or substantiate the PQ.08 quotation.

## 9. Questions to the authors

1. In deployed SCMS (or the CAMP specification), are ACA responses retained by the RA after download, and for how long? Are LCIs ever re-issued?
2. Is the LCI actually public-key encrypted, and how is the LA→ACA PLV key established? Is it via ECIES to the ACA's certificate key, or symmetric and provisioned out of band?
3. Is the MBR outer layer signed with the enrolment certificate in 1609.2.1? If yes, which clause?
4. What fraction of initial butterfly requests are made in-plant versus over cellular? This decides whether "request + download" or "download only" is the realistic path (a).
5. How does the result change for an RSU/DOT operator that already archives BSMs but has no access-network position?
6. Does the PQ butterfly of ePrint 2024/946 preserve statistical hiding of the ACA offset, or only computational? Your claim (2) assumes the latter.
7. How large is the SCMS-specific linkage increment for a vehicle whose TCU also sends OEM telematics over the same recorded network?

## 10. Overall

**Score: Major revision.** The core idea is sound, timely and under-analysed. The RA-archive/LCI path could make it a genuinely good PoPETs paper. As stated, however:
- it contains one factual error in the attack taxonomy (W1);
- it misreads its closest academic competitor (W2);
- it has no formal adversary model (W3);
- it proposes an evaluation that counts linkable certificates rather than measuring privacy loss against realistic baselines (W4, W5).

None of these is fatal, and all are fixable within the proposed timeline.

**Confidence: 4/5.** I verified BKS, Brecht et al., ETSI TR 103 949 and Chen et al. against their texts. I could not read ePrint 2018/1247 or 2024/946 in full, IEEE 1609.2.1 itself, or the 5GAA/GSMA document.
