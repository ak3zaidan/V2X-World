# Check 1: competing V2X security datasets and benchmarks (2025–2026)

Written 2026-10-23. Role: critical reviewer for Paper 2's novelty claim. This check also bears on Paper 1.

**The claim under test:** "same-seed counterfactual impact labels + cross-layer radio/security/SCMS labels + artefact-free design".

**Basis, per source:**

| Source | Basis |
|---|---|
| hermann2026veremi (VeReMi NextGen) | full text: OPARU PDF, all 8 pp. |
| kamel2020veremi (VeReMi Extension) | full text: HAL hal-02492739, 6 pp. Also the Zenodo 20090854 README |
| muzammil2026code | full text: the journal PDF and conference PDF inside the Zenodo 22397725 package, plus the verifier file list |
| novo2026spade | full text: arXiv 2609.02741v2, 7 pp. |
| guven2025creating | full text: Springer OA, 18 pp. Also the current GitHub README of its data repository |
| anjum2026mint | full text: arXiv 2607.22654v1 |
| stenhammar2026advancing | full text: arXiv 2604.12888v2 |
| abdelhakeem2025pq (PQ-V2X) | **The paper is paywalled** (IEEE JIoT; Semantic Scholar lists no OA copy). I read the authors' released generator `generation/generate_v1_paper.py` (GitHub ShimaaAbdelHakeem/PQ-V2X-Dataset), its README, and both DataPort pages (data needs a subscription). I also read the README of an independent leakage-aware re-evaluation (GitHub marcosFigueredo/pqc-v2x-hierarchical-detection) |
| yuce2025state | **Paywalled** (IEEE PIMRC 2025; Semantic Scholar "CLOSED"). Read: the abstract and the Mendeley record 10.17632/n975twtvx9.2 |
| jerbi2026poster | **Paywalled** (VNC 2026 poster; "CLOSED"). Read: the abstract only |

**Found by the search** (beyond the catalog):
- **V2X-PAD** (IEEE DataPort 10.21227/a0we-0f36, Oct 2026). Paywalled; read the landing page only.
- **MisbehaviorX** (DataPort 10.21227/s44z-8616). Read the landing page.
- **BSMAttacker** (wu2026bsmattacker, Zenodo 23089747 v1.0.1). Read the code and README.

The search found no NeurIPS 2025 D&B or 2026 "Evaluations & Datasets" paper, no KDD paper, and no new arXiv dataset on V2X misbehaviour or security (arXiv API, 2025–26).

---

## 0. Verdict

1. **Cross-layer labels that include the PKI/SCMS layer: still novel.** No dataset found carries any of:
   - misbehaviour-report records;
   - MA decisions;
   - CRL, linkage-seed or revocation events;
   - certificate state per message.

   DARE (2020) has reports only. It has no MA decisions and no revocation, and it predates the period checked. PQ-V2X has class names such as `Classic_SpoofedCert` and `Certificate_Transparency_Bypass`, but they are **i.i.d. random feature rows, not PKI events** (§1.9).
2. **Counterfactual impact labels: still novel as labels.** The novelty is only in the labels, not in the pairing:
   - Same-scenario attack/no-attack pairing exists already. SPADE runs every seeded scenario "seven times — once per label class", with "road layout, vehicle density, and camera conditions … held constant across the seven label runs" (§IV-B). MisbehaviorX has a benign run and an attack run of the same Boston scenario. bello2026human differences each attack arm "against a policy-matched no-attack baseline".
   - None of them turns the pair into a **per-unit label**. None can, because in none of them does a receiver act on the false data. SPADE says so outright: "vehicles are unable to operate on the falsified malicious messages … the decision the vehicle would make … is simply logged" (§VI-B). NextGen injects "in post-processing", so it "cannot capture feedback loops" (§V, §VIII-B).
   - The closest thing to an impact label is MisbehaviorX's per-message `eebl_warn`/`ima_warn` flags. Those are **factual, open-loop warning flags**, not counterfactual harm.
   - BSMAttacker replays attacks against FCW/IMA/EEBL "on real warning events". That is also open-loop, and its results are "under revision".
3. **Radio labels: partly done before, never alongside security labels.** Who has radio data:
   - VeReMi 2018 has RSSI. Muzammil measures its path-loss exponent at 0.93 against free space's 2.0, so it "cannot validate signal-strength position verification" (§IX).
   - Guven has RSSI from log-normal shadowing, but the data is **withdrawn** (§1.4).
   - MINT-V2X and Stenhammar have rich radio data and no security content.
   - NextGen has no RSSI ("not available in the current MOSAIC version", §IX).
4. **"Artefact-free": we cannot claim it.** It is unfalsifiable as worded, and it invites a Muzammil-style audit we may fail (§3). The defensible form is: "passes the published artefact audit (Muzammil S0–S6 cascade, single-scalar baselines, grouped-split leakage protocol), which ships as a CI check shown to go red under injected artefacts".
5. **"Same-seed" is not a selling point.** It must not be listed as a contribution (point 2). Our own critique (critique-methods.md; novelty-check.md D3 correction) already limits exactness to:
   - open-loop replay;
   - backend-only changes;
   - the near field of a closed-loop discard.

**Net:** the conjunction holds, and so does each of three components:
- labels in the SCMS layer (reports, MA, revocation);
- per-unit counterfactual impact labels, closed loop;
- one encoded message stream carrying all three layers.

The risk has moved. In 2025 it was "someone builds this". Now it is "someone shows our benchmark has the same disease as VeReMi". Muzammil wrote a ready-made audit tool, and the PQ-V2X re-evaluation shows people now run such audits (§1.9).

---

## 1. Per-dataset findings

Column key for the table in §2: **Msg** = per-message ground truth; **Imp** = impact/counterfactual label; **Rad** = radio-layer label; **PKI** = SCMS/PKI-layer label; **Fmt** = real message encoding; **Ins** = credentialed insider modelled as a credential; **Real** = grounding in real data.

### 1.1 VeReMi Extension (kamel2020veremi, ICC 2020)

**Labels.** Ground-truth file joined by `messageID`. Attackers are "only labeled as such when actively performing a certain attack" (§II).

**Message format and radio.**
- JSON with `sender` and `senderPseudo` (§IV-D).
- No RSSI.
- LuST subset, 1.61 km², peak 67.4 veh/km².

**Sensor error model** (§IV-B):
- Position: EP₀ = U([−5, 5]) m, μ = (EP₀ + EPₜ₋₁)/2, σ = 0.03·EP₀. **This is a nearly constant per-vehicle bias with step jitter of at most about 0.15 m.** It bears directly on Paper 1 (§4).
- Heading: EH₀ = U([−20, 20])°, decaying as e^(−0.1·V).

**Attacks and prevalence.** 19 attack families. "The misbehavior attacker penetration rate is set at 30% for all the simulations" (§IV-D).

**Benchmark.** Benchmark F1 ranges from 0.51 (DataReplaySybil) to 0.9995 (DoSRandom) (Table III).

**What the paper did not foresee.** It names "misbehavior reports dataset" as future work (§VI). It also says reaction "is still not well defined in the literature" (§III-B).

**No impact, radio or PKI labels.** Real format: no. Insider credential: none.

### 1.2 VeReMi NextGen (hermann2026veremi, VNC 2026)

**Setup.** MOSAIC 25.0, SUMO 1.22, OMNeT++ 5.5.1, InTAS. 15 attacks, 20% attackers.

**The `attacker` label is a significance-threshold label** (§V): "1 indicates a significant deviation of at least one message attribute … 0 indicates a legitimate message or a deviation below the significance threshold". So modified-but-small messages are labelled benign. The thresholds sit in GitHub, not in the paper.

**This is the nearest thing to an "impact" label in the field, and it is a heuristic proxy.** The authors justify it as "messages with sufficiently large deviations … could impact cooperative driving functions" (§V). That is a claim about impact which nobody measured.

**Post-hoc injection.** "Adding attacks in post-processing rather than in the simulation" (§V), with the known consequence: "cannot capture feedback loops" (§VIII-B).

**Format and radio.**
- JSON with `sender_id`, `sender_alias` and embedded noise fields (Listing 1).
- No RSSI (Table IV, §IX).

**Evaluation.** One MDS (CaTch), tuned to maximise F1 per scenario. Feigned Braking scores F1 0.03–0.07 and Acceleration Multiplication 0.05–0.12 (Table II).

**Internal inconsistency.** §VII says the comparison uses "Constant Position Offset, Random Position Offset, and Sudden Stop". Table III and the following text use **Random Speed Offset** instead.

**Artefacts.**
- Muzammil §X / Table XIII: the alias leak is fixed (linkage AUC 0.494–0.506).
- A new one replaces it: "Every attacker identifier exceeds 1 000 078 … The identifier alone therefore separates the classes perfectly" (sender_id is ground truth, but "published pipelines do key on that field").
- Benign identities never rotate.
- Message count alone gives AUC 0.972–0.996.
- 2–49 copies per transmission.

### 1.3 Muzammil, "What VeReMi Measures" (muzammil2026code; journal PDF in the package, ToITS-formatted, "submitted for review")

**Scope.** The eight Extension Sybil archives, plus probes of VeReMi 2018 and NextGen.

**Eight measured artefacts** (§V, Table IV):
1. **Pseudonyms encode the sender** in 86.5–100% of identity pairs ("k·10ⁿ + sender·10 + c"). A trivial decoder recovers the sender 87.6% of the time against 0.11% chance (Table V).
2. **No benign vehicle emits a second pseudonym** in any archive (§V-D).
3. **Prevalence is fixed at 30.00–30.04%** of vehicles. Attacker identities are **67.6–97.5%** of all identities.
4. **Ground truth lists up to 27.6× fewer identities** than receivers observe (4,068 vs 112,191) (§V-E).
5. **Identities per attacker vary from 1–7 to 100** across families (§V-G).
6. **The preprocessed derivative (Zenodo 14903687) cannot express Sybil.** It has no sender or pseudonym column, a 95% attacker row rate, and two receivers holding 95% of rows (§V-H).
7. **Rate and timing shortcuts.** A single untrained scalar (message count) reaches AUC > 0.99 on 6 of 8 archives. Sibling ghosts sit at exact ½ and ⅓ beacon-period phase offsets, and timing-only linkage reaches AUC 1.000 (§VI, Tables VI–VII).
8. **Receiver duplication:** a median of 4–7 copies per transmission (Table IV).

**Protocol cascade (S0→S6).** Removing ground-truth-key aggregation is "the largest and only unambiguous change" (−0.060/−0.064). At 3% prevalence, average precision falls from 0.956 to 0.620 while AUC barely moves (§VII–VIII, Fig. 3).

**VeReMi 2018 radio substrate.** Path-loss exponent 0.93. RSSI position verification reaches AUC 0.530 across folds and 0.499 out of fold (Table XII).

**On Guven.** "Its repository carries an owner takedown and authorship-dispute notice. All data and code were restricted" (§IX).

**Recommendations** (§XI-A):
- state attacker fraction, beacon rate and density;
- separate timing from geometry;
- compare against a single-scalar baseline;
- use identity-free tokens and vehicle-disjoint splits;
- publish the prevalence sweep.

**Its released cascade "accepts any detector through a model flag".** We can run it on our own export.

**Caveats on Muzammil itself:**
- single author;
- convenience corpus ("not a systematic review", §III-A);
- one classifier (RF-200, untuned);
- a lineage of two datasets only (§XII).

The measured artefacts are nonetheless re-derivable from source with 21 verifiers. Treat them as facts.

### 1.4 Guven & Taysi (guven2025creating, PPNA 2025)

**Setup.**
- Veins 5.2, SUMO 1.8, 11.2 km² of Istanbul.
- 10 Hz beacons, 1,750–2,250 active vehicles.
- Log-normal shadowing plus "Simple Obstacle Shadowing".

**Attacks.** Four data-generation methods × five power-control modes = 20 Sybil attacks (§4.4).

**Labels.** Ground-truth file with attack type, power-control type and Tx power per message. **Note:** the receiver section logs "Self Vehicle ID (real)" (Table 6). That is harmless for the receiver itself but must be dropped when the receiver also appears as a sender.

**Pseudonym change disabled.** "we simply disabled the pseudonym change policies" (§3). So Muzammil's artefact 2 is baked in by design.

**Prevalence contradiction.** "default value is 0.15" (§4.4), but "attacker vehicles, making up 1.5%" (§5.3; 119 of 7,876).

**Validation:**
- traffic only against municipal average speed (NRMSE 0.191, §5.1);
- RSSI only shown qualitatively against F2MD (Fig. 6);
- "further validation against real-world measurements would strengthen its credibility" (§6).

**Cost.** 20 days and 820 GB for 25 simulated minutes. 55–60 days for the full scenario (§4.5, §6).

**Availability: dead.** The repository README today reads "Do not use or cite this work … I did not authorize the publication of the associated article … Data is restricted for all use cases". **Paper 2 must not list it as an available comparator.** Cite it only as the stated RSSI-Sybil precedent, with the dispute noted.

### 1.5 SPADE (novo2026spade, arXiv 2609.02741)

**Setup.** MOSAIC, J2735 SPaT attacks injected at runtime from the CV's perspective. Six attack classes plus benign. 36 configurations × 5 seeds = 180 runs, each run once per class = 1,260 executions. About 1.89 M records, 40 features.

**Labels.** Per-timestep class from the "scenario schedule … joined … on timestamp" (§IV-D).

**Same-scenario pairing across classes exists, but it yields no impact label.** Vehicles do not act on the false SPaT (§VI-B). Proprioceptive data "always aligns with ground-truth and will remain indistinguishable between attack and benign scenarios".

**Run-level class artefact.** Every record of an execution belongs to one class, and classes are balanced at 270,000 per class (§V-A). Any feature that fingerprints the run (density, DoS producing "fewer" records) predicts the class. The scenario-run split (§V-A) stops cross-split leakage but not this.

**DoS is suppression of the legitimate RSU** ("without requiring ns-3 or OMNeT++ integration", §III-B). No radio layer.

**The camera "confidence" is a hand formula,** 0.4 + 0.6·F_d·F_α + N(0, 0.032), not perception (§IV-A eqs. 1–6).

**Format.** CSV features, not encoded J2735.

**PKI.** Insiders are assumed to hold 1609.2 credentials. No credential is modelled.

**Misdescribes VeReMi.** It credits VeReMi 2018 with "15" attack types and NextGen-style splits (§II-B, Table I).

### 1.6 MINT-V2X (anjum2026mint, arXiv 2607.22654)

**Setup.** SUMO + Veins + Simu5G. 1,386 vehicles over 3 h, 15 RSUs, 29 features at 10 Hz.

**No security content.**

**The validation is largely tautological — a "check that cannot fail":**
- "CQI–SINR 0.993" is "the deterministic mapping of the adaptive modulation and coding scheme" (§II-C).
- Path-loss exponent "n = 1.991 … very close to … free-space" is the configured log-distance model returned (§III-C-2).
- "PDR (mean) 0.286 … expected 0.08–0.99 … PASS" (Table II).

**It mixes stacks.** Simu5G LTE Rel-14 metrics (CQI/MCS) sit beside "802.11p, 3–27 Mbps" PASS rows (Table II).

**Limitations, by its own account:** "does not constitute field-measurement validation" (§V). Idealised line of sight, with NLOS excluded.

### 1.7 Stenhammar et al. NDT (stenhammar2026advancing, arXiv 2604.12888)

**Setup.** Sionna RT + ns-3, Munich 500×500 m, 12 base stations at 3.5 GHz. A **cellular Uu** network, not sidelink V2X.

**Data.** 1 s FlowMonitor samples: SINR, RSRP, LoS, latency, PER.

**No security content.**

**Random 80/20 split of 1 s time-series samples** for the QoS prediction (§III-B). That is a temporal-leakage design.

**No real-data comparison:** "A comparison … and the real world is inherently difficult … Future work might … conduct a targeted measurement campaign" (§IV).

**Relevance.** A radio-layer realism competitor, not a security one.

### 1.8 Yuce et al. (yuce2025state, PIMRC 2025): paywalled; abstract and dataset record only

**Claim.** It claims "the first publicly available dataset … supporting misbehavior detection over standardized CPMs".

**Provenance.** The Mendeley record says the GNN and tabular sets are "generated from BurST-ADMA" by "adding clustering information". Guven §2 describes BurST-ADMA as "generated solely with a traffic simulator without a network simulator", so it has **no radio layer**, and it contains "only faulty behavior attack types".

**Labels.** Not described in any readable source.

**Rival critique.** Jerbi's abstract calls "the only available CPM misbehavior dataset" unreproducible, without "precise message-level labels, and explicit ground truth for perceived objects". This almost certainly refers to Yuce.

### 1.9 PQ-V2X (abdelhakeem2025pq, IEEE JIoT 2025): paper paywalled; generator code read

**The released generator shows the dataset is not a V2X simulation** (`generation/generate_v1_paper.py`):

**Every row is an i.i.d. draw from `generate_sample()`:**
- latitude/longitude `np.random.uniform` over a San Francisco box;
- speed `normal(60, 18)`;
- zone `np.random.choice`;
- SNR and packet loss `uniform` per zone;
- latency `uniform(10, 400)`.

There are no trajectories, no messages, no network and no time series.

**Features are drawn conditional on the label:**
- `Shor_Signature_Recovery` → `sig_generation_time = uniform(200, 400)`;
- `Classic_DDoS` → `message_frequency = randint(1500, 5000)`;
- Grover/Shor/QuantumNoise rows only get `adversarial=True` noise (SNR uniform(1, 5), packet loss +0.4, latency ×2.5).

**The label is written into features:**
- `risk_score = … + 0.25·is_attack + …`;
- `synthetic_anomaly_score = 0.25·risk_score + 0.25·is_attack + …`;
- `detected_flag` is thresholded on that score.

So the abstract's "LightGBM attains perfect detection" is the label read back.

**"Tunnel zones amplify risk" holds by construction.** The tunnel SNR profile is (2, 10) and `risk_score` adds 0.2·(1 − snr/30).

**The "10.05% real traces" are a keypair plus a signature over the constant `b"V2X_SECURE_MESSAGE"`.** They come from liboqs, not from V2X traffic. The README agrees that `is_real_trace` "should not be described as a field-collected vehicle trajectory or field-measured V2X network trace".

**Independent re-evaluation** (marcosFigueredo/pqc-v2x-hierarchical-detection):
- "Neither PQ-V2X release holds five million independent observations".
- Under a random split, macro-F1 is "above 0.98, which measures what duplicate-pattern leakage contributes".
- Under grouped splits, LightGBM macro-F1 is **0.35–0.59** on the original release.

**Implication for us.** Do not cite PQ-V2X as a PQ V2X benchmark in the same class as ours, except as a cautionary example. Our PQ certificate-cycle runs (Falcon 380 B BSMs, ML-DSA-44) are a different category: executed protocol, not label-conditioned sampling.

### 1.10 Jerbi et al. (jerbi2026poster, VNC 2026 poster): paywalled; abstract only

**What it is.** An Artery framework with a standard Collective Perception service and a "central, service-agnostic misbehavior engine … independent of ASN.1 message schemas and encodings".

**No dataset is released per the abstract.** It is a framework poster. Injecting independently of the encoding means attacks act on decoded fields, not on wire bytes.

**Threat.** It prepares Artery, an ETSI stack, for CPM misbehaviour datasets. It is a potential future rival for the CPM part, with no SCMS.

### 1.11 Other 2025–2026 releases found

| Release | What it is | What it lacks |
|---|---|---|
| **V2X-PAD** (DataPort, Oct 2026; paywalled, no companion paper) | NS-3 802.11p, 830,706 BSM rows from **five runs**. Six families including Identity-Clone, "multi-label annotations", difficulty levels L0–L3 | No radio, PKI or impact labels stated. "Physical-aware" is unexplained |
| **MisbehaviorX** (Qualcomm + VT, VASP/Veins, Boston) | 68 attacks, 25% persistent attackers, per-message `attack_type`. Receiver-app flags `eebl_warn`, `ima_warn` | No radio, no PKI, CSV. Benign run plus attack run |
| **BSMAttacker** (wu2026bsmattacker, Zenodo v1.0.1, 2026-10-01) | **The only release grounded in real data**: 11 attacks injected post hoc into benign trajectories from five field corpora (Montreal, **SPMD Ann Arbor**, **AMCD Tysons**, **V2AIX Aachen**, BME Budapest); nine detectors trained on VeReMi Ext, tested on real data; FCW/IMA/EEBL replay | Open-loop, no PKI. "The results produced with v1.0.0 are under revision" |
| **Catalog-only (abstract)** | bodkhe2025indian (AhmST, kinematic injections), khan2025context, aldahlan2026veremivndn (NDN forwarding attacks), wang2026trust (trust interactions, 96 vehicles), masoodi2026veremi (VeReMi-Graph, a derivative), malik2025security (CACC jamming benchmark with safety outcomes) | None has PKI/SCMS labels or counterfactual labels |

---

## 2. Comparison table

| Dataset | Msg | Imp | Rad | PKI | Fmt | Ins | Real | Open now |
|---|---|---|---|---|---|---|---|---|
| VeReMi Ext 2020 | GT file | no | no | no | JSON | assumed | LuST only | yes |
| VeReMi NextGen 2026 | threshold flag | proxy threshold, unmeasured | no | no | JSON | assumed | InTAS only | yes |
| Guven 2025 | GT file | no | RSSI (log-normal, unvalidated) | no | text | cert pool assumed, rotation off | traffic NRMSE 0.191 | **withdrawn** |
| SPADE 2026 | per-step class | no (paired runs, no reaction) | no | no | CSV | assumed | OSM geometry | yes |
| MINT-V2X 2026 | n/a | no | SINR/CQI/PDR (internal checks) | no | n/a | n/a | none | GitHub |
| Stenhammar 2026 | n/a | no | SINR/RSRP/LoS (Uu) | no | n/a | n/a | none | DataPort |
| Yuce 2025 | ? (paywalled) | no | no (BurST-ADMA) | no | ? | ? | none | Mendeley |
| PQ-V2X 2025 | i.i.d. row | no | random draws | class names only | CSV | n/a | liboqs blobs only | subscription |
| Jerbi 2026 | framework | no | (Artery) | no | encoding-agnostic | ? | none | no data |
| V2X-PAD 2026 | multi-label | no | ? | no | ? | ? | none | subscription |
| MisbehaviorX | per message | warning flags (factual) | no | no | CSV | assumed | none | subscription |
| BSMAttacker 2026 | Is_Attack | FCW/IMA replay (open-loop) | no | no | CSV | no | **5 field corpora** | yes, results under revision |
| **Ours (target)** | per message, by layer | per-unit counterfactual, closed loop | PHY/MAC per reception | MBR, MA, CRL/linkage, cert state | encoded J2735 + 1609.2 | real credential lifecycle | must add (§4) | — |

---

## 3. Artefacts our benchmark must avoid

Each item cites its source and says what the engine has to do. "Check" means a released test that **must be shown to go red** on an injected copy of the artefact (memory: checks-that-cannot-fail).

1. **Receiver-visible identifiers that encode the sender.** Sources: Ext pseudonyms with sender digits (Muzammil §V-A); NextGen attacker ids > 1,000,078 in a range disjoint from benign ids (Table XIII).
   - `veremi.rs:132-143` writes `sender` as a SHA-256 fold of the pseudonym, and the cert digest is random. Good.
   - **Verify** that `messageID` leaks no grouping. It is the global SDU arena index (`run.rs:5445`, `u64::from(desc.sdu.index())`), so ghosts one Sybil emits in the same tick may get **consecutive ids**. That would be a linkage shortcut of the same kind as Muzammil's phase artefact.
   - Check: linkage AUC from (pseudonym, messageID) alone ≈ 0.5.
2. **Benign vehicles that never rotate** (Ext, NextGen, Guven by design).
   - `pseudonym_policy.rs` implements J2945/1 `CERTCHG` 300 s, NYC 2 km, C2C-CC and mix-zone. The released scenarios must run rotation on, not `silent`.
   - Check: benign identities per vehicle > 1 in every scenario longer than 300 s.
3. **Fixed, inflated prevalence** (30% of vehicles, 68–98% of identities in Ext; 20% in NextGen; 25% in MisbehaviorX).
   - Sweep prevalence down to ≤ 1% of vehicles.
   - The benign population must be large enough that the floor sits below Muzammil's 3%/5% (it was bound by 718 benign identities).
   - Report average precision, not just AUC.
4. **Ground truth that omits emitted identities** (Ext: 27.6×).
   - The ground-truth log must list every certificate digest every actor used, with validity intervals.
5. **Rate and count shortcuts** (Muzammil: count AUC > 0.99; NextGen 0.972–0.996).
   - Benign beacon timing must carry the real variance: 10 Hz, DCC/congestion control, loss.
   - Publish single-scalar baselines (message count, median inter-arrival) per scenario as the floor any entrant must beat.
6. **Deterministic transmit phase** (sibling ghosts at exactly ½ or ⅓ of the period).
   - Attacker transmit scheduling must go through the same MAC and jitter as benign traffic.
   - Check: phase-residual linkage AUC.
7. **Receiver duplication counted as independent data** (4–49 copies per transmission).
   - Ship a transmission-level view.
   - Splits are disjoint by vehicle **and** transmission.
8. **Ground-truth-key aggregation in loaders** (the largest effect in the S0→S6 cascade).
   - Ship the loader, so the only identity key it offers is receiver-visible.
   - Assert split disjointness at run time.
9. **An unphysical radio substrate** (VeReMi 2018 path-loss exponent 0.93; MINT's free-space exponent 1.991 reported as validation).
   - Publish the fitted path-loss exponent and scatter per scenario against a measured reference, as a check that can fail. Do not report "matches the configured model".
10. **Label-conditional synthesis and label-derived features** (PQ-V2X `risk_score`/`synthetic_anomaly_score` contain `is_attack`; adversarial noise only on attack rows).
    - `leakage.rs` checks forbidden **keys** and true-identity **values** only (header lines 1–30). It would not catch a feature *computed from* the label, or structural encodings like items 1, 5 and 6.
    - Add a value-level test: mutual information between each node-visible column and the label on attack-free (placebo) runs ≈ 0.
11. **Duplicate-pattern leakage under random splits** (PQ-V2X: macro-F1 > 0.98 random vs 0.35–0.59 grouped; Stenhammar 80/20 random on 1 s series).
    - Grouped or temporal splits only.
    - Publish the random-split number next to it, to show the gap.
12. **Run-level class identity and forced balance** (SPADE: one class per execution, 270k per class).
    - Attack runs must also contain benign traffic.
    - Keep natural imbalance.
    - Never let a run fingerprint predict the label.
13. **Post-hoc injection and receivers that do not react** (NextGen §V; SPADE §VI-B).
    - The closed-loop tier is the differentiator.
    - But FCW/EEBL/IMA warnings do not yet change driving in the engine (`run.rs:4324-4333`, critique-methods). **Until they do, our "impact" labels are as open-loop as MisbehaviorX's warning flags.** This is the single biggest gap between the claim and the code.
14. **Heuristic "consequential" labels presented as impact** (NextGen significance thresholds).
    - Ship both a manipulation label and a measured counterfactual-impact label.
    - Report how often the two disagree. That disagreement is itself a result.
15. **Over-smoothed honest error models** (Ext: EP₀ ~ U[−5, 5] m bias, σ = 0.03·EP₀).
    - This hides honest false positives (§4).
    - Offer raw-GNSS and Doppler-smoothed receiver profiles as an explicit factor.
16. **Unavailable or disputed data** (Guven), and **subscription walls** (PQ-V2X, MisbehaviorX, V2X-PAD on DataPort).
    - Use an open licence and an open host (Zenodo), and give hashes.
17. **Validation that cannot fail** (MINT's CQI–SINR mapping, PDR "PASS" over 0.08–0.99).
    - Every validation row needs a tolerance taken from an external measurement, and a demonstrated failure.

---

## 4. What this changes

### Paper 1 ("Honest but wrong")

**The VeReMi-family noise model supports the thesis, and it is citable.**
- kamel2020veremi §IV-B models honest position error as a slowly varying bias with σ = 0.03·EP₀ (≤ 0.15 m).
- So the consecutive-BSM jump |p₁ + v₁Δt − p₂| stays well below 1 m, and an honest vehicle never trips gpsDrift.
- NextGen inherits this model (§IV, "adapted from VeReMi Extension"). BSMAttacker reuses it (`noise_model.py`).
- So every detector threshold tuned on these datasets has never seen raw-fix jitter. That is exactly the blind spot Paper 1 measures (usfp.md: 26 honest vehicles revoked with raw GNSS vs 0 with Doppler smoothing).

**Real benign BSM corpora exist, and BSMAttacker uses them** (SPMD Ann Arbor, AMCD Tysons, V2AIX Aachen, Montreal, BME).
- Paper 1 must compute the SM-MBR §2.3 RandomPosition trigger rate per honest vehicle-hour on these real logs. That decides between our two receiver profiles.
- **The test could go against us.** If deployed OBUs already emit smoothed fixes and the real-log trigger rate is near zero, the raw-GNSS arm shows a configuration risk, not a current deployment harm. The claim would then have to be reworded.
- A caution: real logs may be down-sampled or post-filtered, and SPMD's rate and processing must be checked before use. **Do this before writing.**

**Measure in the right units.** Report trigger rates and wrongful-revocation rates per vehicle-hour, swept over prevalence. Do not report F1 at one operating point (Muzammil Fig. 3: average precision collapses with prevalence).

**No competitor.** No dataset or benchmark found carries MBRs, MA decisions or revocation, so nothing scoops Paper 1.

### Paper 2 (engine + benchmark)

**Claims to drop.**
- Drop "first same-seed / paired counterfactual runs". Cite SPADE (§IV-B), MisbehaviorX and bello2026human.
- Drop "artefact-free". Replace it with the audited form in §0.4.
- Do not compare against Guven as available, or against PQ-V2X as a peer benchmark.

**Claims to keep, worded tightly.**
- The first V2X misbehaviour benchmark with **SCMS-layer labels**: reports, MA decisions, CRL/linkage revocation, and per-message certificate state.
- **Per-unit counterfactual impact labels** whose divergence comes from receivers reacting in closed loop.
- Radio-reception labels for the same messages.
- All in encoded J2735/1609.2.
- Each claim is scoped to the exactness regimes in critique-methods.md.

**Prerequisites that are now mandatory:**
- warnings that change driving (item 13);
- the messageID adjacency check (item 1);
- rotation on in released scenarios (item 2);
- a value-level leakage test (item 10);
- a NextGen-format exporter, which does not exist yet (refine-bench §1);
- running Muzammil's released S0–S6 cascade on our export and reporting it;
- a real-data anchor: replay our detectors on BSMAttacker's corpora.

**Watch list.**
- Ulm (closed-loop NextGen is announced, §VIII-B/§IX).
- BSMAttacker v2 (real data + safety replay).
- V2X-PAD's companion paper.
- Jerbi/Artery CPM datasets.
