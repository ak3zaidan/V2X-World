# Hostile critique of Direction 3: "Why V2X security studies disagree"

Date: 2026-10-17. Target: §3 of `top-three-directions.md`. The reviewer stance is a USENIX Security PC member, with a WSC/TOMACS methods reviewer behind them. Ids resolve to `summaries/<id>.md`; NEW = not in `catalog.jsonl`.

**Verdict: keep with changes — major ones.** The core gap is still open: nobody has run a many-factor sensitivity analysis of V2X *security conclusions*. But as written the direction has three serious problems:

- its headline contradiction is not a contradiction;
- its main method, Sobol over "field-bounded" ranges, does not fit adversarial and policy factors;
- its PQ hypothesis H9 is contradicted by the engine's own sensitivity table.

Re-scope it as a **pre-registered contradiction audit**. Use the engine to re-test real disagreements between published papers, with scenario discovery on flip indicators. Aim it at VNC, VehicleSec or TOMACS rather than USENIX. **Replace** it if a desk audit finds fewer than three genuine, reconcilable pairs of disagreeing papers (fix 1).

---

## 1. Kill search for prior work

**The arXiv API worked this session.** Plain `http://` redirects to HTTPS, and the sandbox Python failed certificate verification on that redirect. curl over HTTPS with a 110 s timeout works. The empty results from earlier sessions were therefore transport failures, not negatives. Real zero-hit feeds this session (`totalResults` 0):
- `abs:sobol AND abs:vehicular`
- `abs:sensitivity AND abs:misbehavior AND abs:vehicular`
- `abs:Morris AND abs:screening AND abs:network`
- `abs:post-quantum AND abs:vehicular AND abs:congestion`
- `abs:sim2real AND abs:vehicular AND abs:communication`

The nearest arXiv hit is `abs:pitfalls AND abs:intrusion AND abs:detection AND abs:evaluation`, which returns only the FL-IDS SoK (arXiv 2607.10914, in the catalog). OpenAlex ("sensitivity analysis V2X security simulation Sobol") returned nothing relevant. Web searches for a GSA of V2X MBD, a V2X MBD re-evaluation SoK, platoon-attack Sobol work and theses all returned nothing that does it. Semantic Scholar returned HTTP 429 three times, so it gave no data.

**Nothing pre-empts the systematic version.** These works narrow it:

| Work | What it already covers | Effect on Direction 3 |
|---|---|---|
| engine `docs/validation/sensitivity-catalogue.md`, `sensitivity-security.md` | One-at-a-time sweep of 217 settings and 442 perturbations under the same seed; 69 settings are dead; 16 security rows | The "own-engine sensitivity table" already exists internally. Its novelty is zero for the reviewer objection, and the contribution must be the cross-paper re-test. It is also a ready-made factor registry, which helps |
| wehmer2021practical (catalog); scitepress.org/Papers/2021/104510 | Argues that confusion-matrix evaluation of MBD has "fundamental limitations" for judging real impact | Prior "evaluation pitfalls" argument for MBD |
| anon2017evaluating (catalog stub: "Evaluating misbehavior detection for vehicular networks", KuVS 2017) | Reported by web search as a critique of MBD evaluation strategies with recommendations for authors (attributed to the Ulm group). **Not verified**: no abstract or full text | Prior reporting recommendations for RQ3.4. Obtain it |
| Joerer, Sommer and Dressler 2012, doi:10.1109/mcom.2012.6316780; Kurkowski et al. 2005, doi:10.1145/1096166.1096174; STRESS guidelines, doi:10.1080/17477778.2018.1442155 | Reporting and credibility checklists for IVC, MANET and general simulation | RQ3.4's "checklist" is old as a genre. Only "grounded in measured flips" is new |
| Teixeira and Henriques, SBSeg 2026, https://sol.sbc.org.br/index.php/sbseg/article/view/44302 (NEW) | WiLabV2Xsim 5G-V2X: large PQ signatures cause "severe radio channel saturation"; direct ECDSA replacement "unfeasible" | Another PQ system-level result on NR, which crowds RQ3.3 |
| abrar2026deployment (full text, §Conclusion) | Already a density sweep (six traffic levels of service, LOS and NLOS) on LTE Mode 4. Cites "Bazzi et al. 2021" for NR-V2X fitting Falcon "with minimal PDR degradation up to traffic level-of-service E" | The LTE half of the "regime map" is published. The NR half is claimed (Bazzi 2021, not verified) |
| ahmad2025end (OpenCAMS), mamun2026post, yoshizawa2026zero, agustina2026lightweight, wang2026reproducibility | Same Alabama/TraCR group with a PQ co-simulation testbed; a PQ size-reduction scheme; PQ at N≤100 on 802.11p; a PQ viability/spacing study | PQ remains the most crowded sub-area, confirming that dropping stand-alone PQ was right. It is also crowded as Direction 3's *lead case* |
| krishnan2011verify, bittl2016feasibility, ko2021avod (catalog) | Verify-on-demand policies | H9's "verification policy is the lever" is old, at least for compute |
| T-ITS 2024 traffic GSA, doi:10.1109/tits.2024.3372334 (confirmed via Crossref) | Sobol analysis of traffic-microsimulation inputs | Confirms that "GSA of security outcomes" is the narrow novel slice |

**Watch item:** wu2026bsmattacker is still v1.0.1 (2026-10-01, metadata-only release). Its page says "results produced with v1.0.0 are under revision" (https://zenodo.org/doi/10.5281/zenodo.20754022). It is still the most likely paper to take the detector sim-to-field flip.

## 2. Do the key assumptions hold?

**A1. "Published contradictions exist and can be explained by one factor."**
- **Fails for the lead pair.**
  - petho2024investigating (full text) studies classical ECDSA PKI processing delay. It uses two vehicles, Cohda MK5 OBUs and VSIM, and takes **PDR as an exogenous input** (Table 7: risk at PDR 10/50/100%).
  - Its conclusion is conditional: increased processing time "can only result in unsafe cases if the quality-of-service decreases below an unacceptable level".
  - abrar2026deployment and yoshizawa2023post study **PQ payload size**, which is exactly what lowers that QoS.
  - The two results **compose**; they do not contradict. Reading the two papers explains the gap, so the "reconciliation" needs no 3,000-run analysis.
- **Not contradictions either:**
  - khodaei2020scalable: a magnitude (95% within 15 s) for a different CRL scheme (vehicle-centric CRL pieces, not linkage-seed CRLs). Reproducing it in an SCMS engine is apples to oranges.
  - bello2026ns3learn: a within-paper finding.
- **RQ3.2 and H10, the "main traction driver", therefore have zero verified instances.**
- **Genuine candidate pairs found this session** (to be verified):
  1. abrar2026deployment (LTE Mode 4: Falcon meets 90% PDR only at LOS A with line of sight) vs. Bazzi et al. 2021 as cited by Abrar (NR: minimal degradation up to LOS E) vs. Teixeira and Henriques 2026 (5G-V2X: severe saturation, unfeasible).
  2. twardokus2024cryptography (the hybrid fits within available spectrum) vs. agustina2026lightweight (802.11p: Dilithium PDR 0.51%, Falcon 3.56% at N=100).
  3. VeReMi-Extension detector scores vs. hermann2026veremi (attacks harder to detect on MOSAIC/InTAS) vs. wu2026bsmattacker (field data, pending).
- Each candidate pair differs in several factors at once: stack, MCS, certificate inclusion, HARQ, scenario. Abrar itself lists missing SPS+One-Shot, the missing J3161 §8.6 subchannel rule and disabled HARQ. That is good material, but nobody has yet checked whether one factor explains any of these pairs.

**A2. "Every factor range bounded by field data."**
- **Fails for most security factors.** The cited anchors cover only:
  - radio and PHY: ylmaz2025simulated (PER for static devices); solida2026demystifying (RSSI, message count);
  - message timing: bauder2023cooperative (CAM rate, pseudonym change);
  - nothing measurable: song2024c is a dataset report with "no performance numbers".
- **No anchors exist for:**
  - MA thresholds;
  - detector settings;
  - attacker parameters;
  - CRL timing in a deployed linkage SCMS;
  - PQ timing on OBUs. mamun2026post lists the "lack of real-world PQC deployment data in ITS", and yoshizawa2026zero tested one vendor platform only.
- The "flagged and reported separately" bucket will hold exactly the factors that decide security conclusions.

**A3. "Variance-based GSA is the right instrument."**
- **Mismatched in three ways:**
  - **Adversary factors.** Attack rate, timing and target choice are chosen by the attacker. A variance share averaged over a "plausible" distribution is the wrong question; the security question is worst case.
  - **Policy factors.** Verification policy, signer-ID policy and MA policy are decision variables, to be mapped, not averaged.
  - **Model-form factors cause the decisive flips.** Examples: reception model (bello2026ns3learn), simulator, open versus closed loop, exogenous PDR (petho). These are categorical structural switches, where Sobol indices are ill-posed.
- A "flip" is a threshold event, so regional sensitivity or scenario discovery (PRIM; Bryant and Lempert 2010, doi:10.1016/j.techfore.2009.08.002) is the standard tool, not first-order and total Sobol indices.
- A methods reviewer will also cite Saltelli et al. 2019 (doi:10.1016/j.envsoft.2019.01.012) on the dependence of indices on ranges.

**A4. "The engine can reproduce the bello flip."**
- The engine's stacks are "Veins/OpenCV2X-matched" (PROTOCOL.md).
- bello2026 shows that the Veins-type reception abstraction *misses* the flood. The engine therefore reproduces the side of the flip that bello calls unrealistic.
- Reproducing the flip needs a 5G-LENA-calibrated NR reception model as an alternative model form. That is new engine work, not a preset.

**A5. "H9: PQ safety cost is negligible below a density threshold on DSRC and LTE; the verification policy, not the scheme, is the migration lever."**
- **Contradicted by the evidence:**
  - **The engine's own table** (`sensitivity-security.md`, row `security.signature`), on a 20–60 vehicle grid on 802.11p: ML-DSA-44 moves PDR 0.96 → 0.62, CBR 0.073 → 0.74, unverified share 0.016 → 0.59, all through fragmentation. That is not negligible even at small-fleet density.
  - **On LTE, Dilithium-2 does not fit the transport block at all** (abrar2026deployment). That is infeasibility, not a regime.
  - **Verification policy changes compute, not airtime.** In the engine, on-demand verification leaves 97% of messages unverified (row `security.verification_policy`). The PQ cost is airtime: Abrar and Teixeira name spectrum, not computation.
  - **The lever that does move airtime is certificate inclusion** (signer-ID and P2PCD). That is twardokus2024's published result (93% redundant certificate transmissions).
- So H9's "decision-changing" message is either already known or wrong.

**A6. Feasibility.** Effort was ruled out of the ranking, but a reviewer will ask whether the result can exist at all.
- **Run speed.** `docs/RELEASE-CHECKLIST.md` §4 records 12.1 wall-clock seconds per simulated second at 997 vehicles. That figure is itself unreproducible from the tree.
- **Small fleets.** The catalogue sweep's 250-vehicle, 90 s family takes about 5 minutes a run.
- **Morris screening.** About 150 live factors at r=10 is about 1,500 runs per conclusion family per scenario: about 125 h on small fleets, and over 1,000 h at city scale.
- **Sobol analysis.** Saltelli sampling on just 10 factors at N=512 is 11,264 runs.
- **The squeeze.** The high-density regime H9 targets is the expensive one, on an 8 GB machine.

**"Is F1 how detectors are chosen?"** For family (b), this inherits Direction 2's problem.
- The 5GAA operational white paper (2024, read here) states: "How the MBMS determines the existence and source of a misbehaviour is out of scope".
- Deployed detector choice is proprietary to OEMs and suppliers. Published simulation rankings decide nothing in deployment, so the audience is researchers.

## 3. Would the result change anyone's decision? Is a null result publishable?

- **Standards and OEMs.**
  - PQ decisions are being driven by hard size limits, not by simulated safety envelopes: transport-block fit (abrar2026deployment), certificate format (yoshizawa2023post).
  - ETSI TR 103 949 V1.1.1 (read here) is a qualitative migration study with no simulation. It notes the 50 ms per-hop processing window and the need to carry public keys with messages.
  - A regime map adds little to a choice already forced by "Dilithium does not fit".
  - **Only the corrected H9 could move a standard:** scheme × certificate-inclusion policy × stack, under the J3161 constraints that Abrar's simulator omits.
- **Researchers and reviewers.** This is the real audience.
  - Arp et al. worked because ML-in-security is large and they audited "30 papers from top-tier security conferences" (arXiv 2010.09470 abstract).
  - The catalog holds 4 USENIX Security, 12 NDSS, 25 CCS and 22 S&P entries, and almost none is a simulation-based V2X security result (twardokus2024cryptography is the exception).
  - Re-testing VTC, IEEE Access and VNC papers will not move a USENIX PC.
  - **Venue fit:** VNC or VehicleSec for the audit, TOMACS for the method, TVT for the PQ part.
- **Null result.**
  - "V2X security conclusions are robust inside field-plausible ranges" is publishable only as a TOMACS or VNC validation note.
  - H8 ("radio and density dominate") is the *expected* result: Martinez 2013 (NEW) and Ajjaj 2022 (NEW), both cited in eval-C, found density and map dominate; bello2026 shows radio decides flood visibility.
  - **The publishable value sits entirely in H10 (flips on unreported factors), which currently has no instance.**

## 4. Fatal versus fixable

**Fatal as written** (each needs a change, not a rebuttal):
1. The lead contradiction (petho vs. abrar and yoshizawa) is not a contradiction. RQ3.2 and H10 have no verified pair.
2. H9 and the "verification policy is the lever" claim are contradicted by the engine's own table, by Abrar and by twardokus2024.
3. Sobol as the primary instrument over adversary, policy and model-form factors, presented as "field-bounded", does not survive a methods reviewer.

**Fixable:**
- The USENIX venue claim is over-reached. Move to VNC, VehicleSec or TOMACS, or earn S&P SoK scale by auditing the large VeReMi-based MBD literature.
- Field anchors exist only for radio. Say so, and stratify.
- Compute cost: group screening, corridor scenarios, and an emulator for the Sobol stage.
- The bello reproduction needs an alternative NR reception model.
- The checklist genre is old. Ground each item in a measured flip, and cite Joerer 2012, Kurkowski 2005, STRESS 2018 and wehmer2021practical.
- Contribution 1 ("first many-factor GSA") stands only as "first for security conclusions". Cite the internal OAT sweep honestly.

## 5. Concrete fixes

1. **Desk audit before any runs (go/no-go).**
   - Build a table of published V2X security disagreements, starting with the three candidate pairs in A1.
   - For each pair, record every factor that differs (stack, MCS, HARQ, SPS+One-Shot, certificate inclusion, density, loop closure, PDR exogenous or not).
   - Keep only pairs where both papers report enough to reproduce a headline number.
   - **Replace the direction if fewer than three pairs survive.**
2. **Re-scope the deliverable** as "a contradiction audit of V2X security simulations": for each surviving pair, reproduce both headlines, then walk the factor path between them.
3. **Change the method.** Sort factors into three classes:
   - **Environment** (field-bounded): Morris, then Sobol.
   - **Policy** (decision variables): full sweep, reported as maps.
   - **Adversary**: worst-case search.
   - Use scenario discovery (PRIM) on binary flip indicators as the primary result, with Sobol indices secondary.
4. **Rewrite H9:**
   - The variables: scheme (ECDSA, Falcon-512, ML-DSA-44, hybrid) × signer-ID and P2PCD policy × stack, under strict J3161 (subchannel rule, SPS+One-Shot, HARQ). These are the constraints Abrar's simulator lacked.
   - The headline: what changes once those rules are enforced.
   - Credit twardokus2024 for the certificate-inclusion lever.
5. **Add model-form switches as factors:** a 5G-LENA-calibrated NR reception model (bello2026), exogenous versus endogenous PDR (petho), open versus closed loop.
6. **Make the PQ family optional.** Lead with MBD sim-to-field, the largest literature and the clearest disagreements. Hold until BSMAttacker's revision is out and engage with it rather than racing it.
7. **Budget the runs from the measured costs** (5 min per small-fleet run; 12.1× slower than real time at about 1,000 vehicles). Commit the 1,000-vehicle benchmark first (RELEASE-CHECKLIST §4).
8. **Correct the record:**
   - In `top-three-directions.md`: §3's "contradictory conclusions on PKI and PQ cost" row, H9, and the line in §5 saying the arXiv API returned empty.
   - In `eval-C`: note that the arXiv failure was a transport problem.
