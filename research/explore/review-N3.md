# Review N3: "The bootstrap paradox: misbehaviour-authority evidence thresholds along the USDOT deployment trajectory"

**Reviewer role:** program-committee member, USENIX VehicleSec (I also judge fit for IEEE VNC, see §8).
**Recommendation:** **Major revision**, leaning reject in the form pitched. **Confidence: 4/5.**
**Date:** 2026-10-18. Every claim cites a corpus id (`research/catalog.jsonl`), a DOI, a URL with page or section, or an engine file with line range.

---

## 1. Summary of the pitch

The pitch argues that a corroboration-based Misbehaviour Authority (MA) faces a trade-off over deployment stages. A k-witness threshold that stops one attacker framing honest vehicles in a mature fleet could leave attackers untouchable in 2028, when penetration and RSU density are low. A threshold low enough to work in 2028 could let a single attacker frame honest vehicles. The planned study counts independent witnesses per attack event and the MA's time to act. It sweeps penetration (1–50%), RSU density at the levels in the USDOT plan, more than 50 US maps and three radio technologies. It promises a stage-dependent MA policy and a verdict on RSU-hosted detection.

## 2. Does it matter? Who would change what

- **The policy anchor is real.** The USDOT National V2X Deployment Plan (Aug 2024) targets V2X on 20% of the National Highway System, and 25% of signalised intersections in the top 75 metros, by 2028 (RCR Wireless, https://rcrwireless.com/20240819/featured/dot-releases-national-v2x-plan; TT News, https://www.ttnews.com/articles/usdot-connected-infrastructure).
- **The MA decision rule is unstandardised, so a gap does exist.** Brecht et al. leave global misbehaviour detection to "on-going work" (`brecht2018security`, DOI 10.1109/tits.2018.2797529, §II p.5 and §VI-B p.16). SAE J3287 standardises the report format and the architecture, not the decision (`anon2024v2x`, DOI 10.4271/j3287_202408). The engine's own module says the same (`crates/v2xw-threat/src/ma_corroborate.rs` lines 39–45).
- **The named adopters are weaker than the pitch implies:**
  - USDOT ITS JPO does not operate an MA or set its thresholds.
  - The 5GAA operational white paper (the "wi-mbd-op" deliverable, published May 2024) classifies misbehaviour by evidence, impact, local detection and footprint (https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf, §6.1–6.6, pp.20–22). Its scope is operational and does not include threshold engineering.
  - The only party that would actually change a parameter is an SCMS provider's MA operator. Its algorithms are proprietary and, per Brecht, "developed as part of another research project" (`brecht2018security` §VI-B p.16).
  - So the realistic output is a design argument, not a parameter that someone adopts.
- **The harm side is missing.** At low penetration the attacker also reaches few equipped receivers. An attacker who is "untouchable" in 2028 may also be nearly harmless. Without a harm-weighted metric (equipped receivers exposed × time to act), the "paradox" may be one-sided. See §4.5.

**Verdict on significance:** moderate, if the premise problems in §4 are fixed. As pitched, it is low, because the trade-off may be an artefact of one MA design.

## 3. Novelty (my own search)

**Searches run:** the corpus catalog (11,315 records), OpenAlex (5 queries), arXiv (`abs:misbehavior AND abs:penetration AND abs:vehic`, 0 hits), web search (standard and extended). I found **no paper that sweeps equipped-vehicle penetration against MA corroboration thresholds**. The extended web search explicitly found none that "report detection rate as a function of market penetration", and noted that VeReMi Extension fixes the attacker share at 30% (https://github.com/VeReMi-dataset/VeReMi-dataset.github.io/blob/main/veremi-extension.md). The narrow claim survives.

However, the pitch names the wrong closest work. Park et al. 2013 (`park2013defense`, DOI 10.1002/sec.679) is not the nearest neighbour. Closer and uncited:

| Work | What it already establishes | What it means for this pitch |
|---|---|---|
| Bißmeyer et al. 2012, central misbehaviour evaluation (`bimeyer2012central`, DOI 10.1145/2307888.2307902, abstract) | A central scheme in which **"at least three witnesses"** suffice to identify a ghost-vehicle attacker, and **37% colluding neighbours** can hide an attack | The witness-threshold vs collusion trade-off at a central authority is 14 years old. What remains new is only the *penetration/stage* axis. |
| Raya et al. 2007, LEAVE (`raya2007eviction`, DOI 10.1109/jsac.2007.071006) | Threshold-of-accusations eviction, motivated by **sparse infrastructure** | The concept of acting on k accusations when infrastructure is thin |
| Moore, Raya, Clulow, Papadimitratos, Anderson, Hubaux, "Fast exclusion of errant devices from vehicular networks", IEEE SECON 2008 (https://infoscience.epfl.ch/record/124927) | Voting (LEAVE) vs Stinger: LEAVE excludes fewer good devices under multiple compromised devices, and Stinger scales better **as traffic density rises** | Density vs voting threshold vs false exclusion has been analysed before, though locally and without an MA |
| Haidar et al. 2020, DARE (`haidar2020dare`, DOI 10.1109/vtc2020-spring48590.2020.9128687) | An MA report dataset across **road topologies and attacker penetration rates** | MA evaluation across topologies exists. Equipped-fleet penetration is the gap, not the topologies. |
| Kamel et al. 2019, MA for Sybil detection (`kamel2019misbehavior`, DOI 10.1109/uemcon47517.2019.8993045) | MA-side detection from **OBU and RSU** reports | RSU-hosted evidence at the MA is not new |
| Almani et al. 2024/2025 (`almani2024enabling`, DOI 10.3390/fi16030077; `almani2025distributed`, DOI 10.3390/fi17040174) | Misbehaviour reporting and **false accusations in disconnected rural areas** with sparse RSUs | Sparse-infrastructure reporting accuracy is touched on, without the USDOT trajectory |
| Doi/Perrone/Bassi/Rapelli/Casetti, ghost-object detectability (`doindquantifying`, https://www.semanticscholar.org/paper/394f43b267726b7473f56ca84a7cc38ab7610946) | Quantifies **how often real traffic lets honest vehicles expose ghosts**, using learned scene generation | Witness availability as a function of traffic is being studied right now. It is a direct competitor for the "Poisson estimates miss it" claim. |
| Henry 2019, TAC conference paper (https://www.tac-atc.ca/en/knowledge-centre/technical-resources-search/conference-papers/misbehaviour-detection-in-v2x-communications/) | Time for the SCMS to recognise misbehaviour and disseminate revocation | The "time to act" metric for the US SCMS has been at least scoped before |

**Verdict on novelty:** the specific intersection (USDOT stage targets × penetration × MA k-threshold × real US maps) is unexplored as far as I can find. But the underlying trade-off is known (Bißmeyer 2012, Moore 2008), so novelty rests entirely on the *stage-dependence result being non-obvious*. "Lower density yields fewer witnesses" is obvious. The paper must show something that is not.

## 4. Is the premise or threat model sound? (main concerns)

### 4.1 A "proof-based" MA does not need witness counts for most evidence

The pitch uses "proof-based" and "corroborated by enough witnesses" as if they were one model. They are not:
- **Self-verifying evidence (one report is enough).** ETSI TS 103 759 sorts evidence into five classes, as restated by 5GAA (op. cit. §6.2 p.20). Class 1 (implausible values in the message) and Class 2 (inconsistency with the same sender's earlier messages) are carried in the subject's *own signed messages*. An MA that re-verifies the attached signed messages needs **one** report, not k.
- **Unverifiable evidence (only here does k matter).** Witness thresholds matter only for Classes 3–5: the reporter's local environment, its sensors, or other senders. There the MA must trust the reporter.
- **The trade-off cuts the same way.** Framing an honest vehicle with Class 1/2 evidence is impossible without forging its signature. So framing, too, is confined to Classes 3–5.

The study must stratify every result by evidence class. Otherwise a large part of the "paradox" disappears on inspection.

### 4.2 The witness model is per-pseudonym, which the real US MA need not be

- **What the engine does.** It counts "distinct trusted reporters" by **reporter pseudonym-certificate digest** (`crates/v2xw-threat/src/report.rs` lines 84–91; `ma_corroborate.rs` lines 331–347). It keys evidence on one subject certificate, with a 60 s window chosen to match the pseudonym lifetime (`ma_corroborate.rs` lines 17–21, 59–66).
- **What the US design allows.** The MA can ask the PCA and one LA whether two pseudonym certificates belong to the same device (binary answer) (`brecht2018security` §VI-B, Steps 3–4, p.17).
- **Consequence for the "untouchable in 2028" result.** If the MA links a suspect's pseudonyms, evidence accumulates across pseudonym changes and across days. A persistent attacker is then not untouchable at low penetration, only slower to catch. The paper's result would be an artefact of resetting evidence at every certificate change.
- **Consequence for the "one attacker frames an honest vehicle" result.**
  - The US design deliberately issues multiple simultaneously valid pseudonyms, and Brecht notes this enables one device to pose as several (`brecht2018security` §III, p.7).
  - One attacker can therefore supply k "independent" reporters whatever the fleet penetration. This side of the trade-off is governed by **reporter de-duplication** (LA-assisted, at a privacy cost to reporters), not by penetration. So the axis the paper sweeps does not drive half of the paradox.

### 4.3 The decision rule being swept is the author's own invention

The engine says plainly that "every threshold here is a design choice with a stated reason" (`ma_corroborate.rs` lines 39–45). The defaults (event window 5 s, 2 reporters per event, 3 events in 60 s, 3 reporters in total) are tuned on one scenario at 1,500 veh/h (lines 46–66). Sweeping penetration against *this* rule characterises this rule, and a reviewer will call it a strawman. The paper needs a *family* of MA rules: threshold, Bayesian/reputation (Bißmeyer-style), LA-linked accumulation, and evidence-class-weighted. The "paradox" must hold across that family, or the paper must state which rules dissolve it.

### 4.4 2028 penetration is close to zero, so the question collapses to RSU geometry

- **Penetration.** The same plan's 2028 vehicle goal is that **two OEMs commit** to 5.9 GHz connectivity by model year 2028 (RCR Wireless, op. cit.). Equipped-vehicle penetration in 2028 is therefore likely well below the 1% floor of the sweep, apart from fleets and aftermarket units.
- **Effect.** At that level, vehicle-to-vehicle witnesses are essentially absent. The 2028 answer reduces to "is the attack inside an RSU's reception footprint". That is a coverage calculation, which needs neither the SCMS nor the closed loop.
- **What the sweep must use.** A deployment-trajectory model with a cited source (fleet turnover × OEM adoption), not a free 1–50% sweep.
- **Spatial clustering.** Early equipped vehicles are clustered (transit, public fleets, new-car buyers in particular areas), not drawn uniformly at random. The pitch says witness counts depend on "traffic clustering on arterials", but the distribution of *equipped* vehicles matters at least as much.

### 4.5 The attacker is assumed non-strategic

Detection is open-loop, and the attacker presumably follows scenario scripts. But the most important property of a thin deployment is that **the attacker chooses where and when to attack**, and RSU locations are public or easy to observe. A rational attacker picks an RSU-free, low-penetration road at off-peak times. So the 2028 question is "what fraction of *harm-relevant* locations and times have zero witnesses". It is not "how many witnesses does a random event get". Averaging over random event placement overstates MA reach.

### 4.6 "Poisson estimates miss" is asserted, not shown

The selling point of a 50-map simulation over a closed form is that geometry, reception and clustering break the Poisson approximation. That must be **demonstrated**:
- run the same witness-count question through a homogeneous Poisson or stochastic-geometry model;
- show where the simulation's distribution departs from it, and by how much it changes the policy.

If the policy conclusion is the same under Poisson, the simulator adds precision but not insight.

## 5. Would simulation evidence convince me? Validation I would require

The simulation is the right instrument *only if* witness counts are credible. The quantity under study is "how many equipped receivers decoded message m in the window, at penetration p". That depends on three things the paper has to validate: traffic demand, equipped-vehicle placement, and reception.

1. **Traffic demand.**
   - Compare per-link volumes against measured counts on a subset of the 50 maps (for example, state DOT AADT or HPMS counts).
   - Report the error on the arterials and NHS segments where the plan's RSUs sit.
   - Synthetic OD demand across 50 cities without this check is the largest validity threat.
2. **Neighbour-count ground truth.**
   - Reproduce the empirical distribution of "distinct BSM senders heard per second" from real connected-vehicle data (for example, USDOT CV Pilot or Safety Pilot BSM logs via the ITS DataHub, https://www.its.dot.gov/data/).
   - Use the same penetration, area and time-of-day. This directly validates the witness-count primitive.
3. **Reception.** For each of the three radio technologies, show the packet-delivery ratio against distance next to published field measurements. A witness exists only if a packet is decoded.
4. **RSU placement.** Use real or planned RSU locations from current deployments rather than synthetic "plan-level density". 25% of intersections chosen at random and 25% chosen by an agency (the highest-volume corridors) give very different coverage.
5. **MA family and evidence classes,** as in §4.1–4.3. Report results per TS 103 759 class.
6. **Fault injection to prove that the checks can fail:**
   - show a configuration in which the MA *does* frame an honest vehicle (for example, Sybil reporters without LA de-duplication);
   - show one in which it never acts (zero-witness geometry);
   - so the "safe region" is shown to be bounded, not assumed.
7. **Statistics.** Paired reruns are a strength. Report confidence intervals per map, and the variance across maps, before claiming one national policy.

With 1–4 done, simulation evidence would convince me of the witness-count maps. Without 1–2, I would read the results as a property of the demand generator.

## 6. Strongest reasons to reject (as pitched)

1. **The trade-off is an artefact of one invented MA rule.** Per-pseudonym witness counting and per-certificate evidence windows (§4.2–4.3) are not features of the US design, which allows LA-assisted linking (`brecht2018security` p.17).
2. **The framing side does not depend on penetration.** With multiple valid pseudonyms per device, one attacker produces k witnesses at any penetration. The control variable is reporter de-duplication (§4.2).
3. **Most evidence classes make witness counts irrelevant** (§4.1).
4. **The 2028 regime is close to zero penetration,** so the "stage-dependent policy" reduces to RSU coverage geometry (§4.4).
5. **The known trade-off is not cited** (Bißmeyer 2012, LEAVE/Moore 2008), and the headline ("fewer witnesses early") is predictable.
6. **There is no harm metric and no strategic attacker** (§2, §4.5).
7. **Thin engine use.** Detection is open-loop and the study needs "no engine extension". The SCMS closed loop (revocation → CRL → effect on traffic) is not exercised. This conflicts with the stated goal of using the engine's distinctive strengths.

## 7. What would make it an accept

1. **Reframe the question.** Drop "witness threshold vs penetration". Ask instead: "**Which MA evidence policies stay sound (no honest revocation) and live (bounded time to act) across the USDOT trajectory, given evidence class, reporter de-duplication and LA-assisted linking?**" That is a real design-space result for MA operators, and it is novel.
2. **Use a strategic adversary.** Model an attacker who knows RSU locations and chooses where and when to attack. Report the *worst-case* fraction of harm-weighted exposure that escapes the MA, not the average over random events.
3. **Use a harm-weighted outcome.** Report receivers exposed × duration until revocation reaches them. This **uses the closed loop**: MA decision → CRL issued → CRL delivered over the plan's sparse RSUs and cellular → receivers stop trusting the attacker. That is where the standards-exact SCMS backend earns its place, and where a coverage calculator cannot follow.
4. **Find a non-obvious headline.** For example: "LA-linked accumulation makes the 2028 gap close in X days for persistent attackers, but leaves intermittent attackers untouchable at any plan-level RSU density", or "plan-level RSUs placed by volume cover Y% of harm-weighted exposure but only Z% of NHS miles". Something an MA operator would not have guessed.
5. **Validate** as in §5 (items 1–4 at minimum), and **compare against an analytical Poisson baseline** to justify the 50-map simulation.
6. **Cite and position** against Bißmeyer 2012, LEAVE/Moore 2008, DARE, Kamel 2019 and Doi/Casetti.

## 8. Venue fit

- **USENIX Security, CCS, NDSS:** not competitive. There is no new attack, no new mechanism, and the evaluation depends on one simulator.
- **PoPETs:** off-topic, unless reporter de-duplication via the LAs is studied as a privacy cost to reporters. That angle is a genuinely interesting sub-question.
- **VehicleSec:** reachable if reframed as in §7.
- **IEEE VNC or IEEE T-ITS:** the natural home for the penetration-study form. At VNC as pitched, I would score it **weak accept / major revision**.

## 9. Score

| Criterion | Assessment |
|---|---|
| Significance | Moderate (the policy anchor is real; the adopters are vague; no harm metric) |
| Novelty | Narrow gap confirmed, but the underlying trade-off is prior art (Bißmeyer 2012, Moore 2008) |
| Soundness of premise | **Weak**: per-pseudonym witnesses, evidence class ignored, near-zero 2028 penetration, non-strategic attacker |
| Evaluation plan | Good scale and paired design; no validation of demand, neighbour counts or RSU placement |
| Engine leverage | Low as pitched (open-loop, no extension); high if reframed around the revocation and CRL loop |

**Overall: Major revision (as pitched, reject at VehicleSec; major at VNC). Confidence 4/5.** The confidence is not 5 because I could not read the full texts of Moore 2008 or Raya 2007, or confirm the engine's three-radio validation status.
