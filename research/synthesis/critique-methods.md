# Methods critique: are "exact paired reruns" exact?

Date: 2026-10-17. Role: methods critic for `top-three-directions.md`. Code was read at `main` (bfb9e237) with grep/sed only; line numbers are from that commit. Literature ids resolve to `summaries/<id>.md`; DOIs and arXiv ids are given where a work is not in `catalog.jsonl`.

## 0. Verdict in one paragraph

The engine is **entity-keyed, not event-keyed**. Every stream is `ChaCha12(SHA-256(seed ‖ domain ‖ entity))` (`crates/v2xw-core/src/rng.rs:1-23`), so one entity's draws do not depend on other entities' draws. That is already better than one global generator. But most streams are **long-lived**: the k-th draw of a `(domain, entity)` key goes to whatever event happens to ask k-th (`rng.rs:49-52`). Two more facts make things worse. Actor and node ids are handed out in the order vehicles are *actually* inserted, not by demand identity (`v2xw-mobility/src/engine.rs:3147-3150`; `v2xw-engine/src/run.rs:3168-3169`). And on the human-driver path, V2X safety warnings never change how anyone drives (§2.4).

The consequences:
- "Exact per-message, per-vehicle paired counterfactuals" hold only for interventions that do not touch transmissions, mobility or insertion.
- The directions' headline interventions (revocation that silences a victim, closed-loop detectors, a removed vehicle, post-quantum airtime, Direction 3's factor sweeps) give **coupled but not exact** pairs. They are valid as common-random-number variance reduction at the distribution level, but not as per-unit causal attributions.
- Direction 2's "net safety" outcome is not produced by the engine today for FCW/EEBL/IMA. It exists only through CACC/platoon and GLOSA.

All three are fixable, and the fixes are listed in §3. Until they land, the shared method needs re-wording. The thesis line "the same seed with one change, compared per message and per vehicle" is wrong as stated (Buffalo & co., arXiv 2603.11084).

## 1. What the engine actually does

### 1.1 Keys and their lifetimes

| Domain (code) | Entity key | Lifetime | Draw count depends on simulated state? | Evidence |
|---|---|---|---|---|
| Fading (6) | `LinkFrame{link, frame: t}` with t the frame time | single use (event-keyed by time) | No. But if a frame moves in time, it gets a **fresh, uncorrelated** draw | `v2xw-radio/src/fading.rs:215-229` |
| AbstractRx (9), drop coin | `LinkFrame{link, frame: arrival.start}` | single use | No (same caveat) | `abstract_tier.rs:1590-1597` |
| AbstractRx (9), legacy log-distance shadow | `Link(link)` | long-lived | **Yes**: one draw per evaluated frame on that link | `abstract_tier.rs:1576-1579` |
| Shadow (5), all propagation laws | `Link(tx,rx)` | long-lived | **Yes**: one draw per evaluated frame, and links are evaluated only inside the 1000 m candidate range (`run.rs:162, 9400`), so the count follows mobility. The 3GPP law also feeds a stateful correlated-shadow update | `prop.rs:1401-1406, 2076, 2127, 2536-2549` |
| MacBackoff (7) | `Node(node)` | long-lived | Partly. One draw per enqueue, deliberately unconditional (`mac.rs:500-509`). But the post-grant draw happens only `if more_queued` (`mac.rs:705-714`), which depends on channel timing | `mac.rs` |
| SpsSelection (8) | `Node(node)` | long-lived | Yes: one draw per reselection, and reselections follow sensing | `sps.rs:1186, 1208, 1294` |
| Mobility (2), LaneChange (3) | `Actor(id)` | long-lived | LaneChange **yes**: the draw is skipped on early returns that depend on traffic state (`mobil.rs:395-409`) | `ctx.rs:528`, `lanechange/mobil.rs:403-406` |
| Spawn (1), equipped flag | `Actor(spawn.actor)` | long-lived | No, but the *key* is a realised-order id (§1.2) | `run.rs:3265-3268` |
| Backend (12), ServiceTime (11), Report (15), Collusion (13), Crypto (16) | `Node(node)` | long-lived | Yes: the k-th request, report or re-enrolment gets the k-th draw | `cellular.rs:559, 823, 1071`; `threat/src/poison.rs:233, 307, 315`; `attack_rsu.rs:385-410` |
| Spawn (1), Poisson demand | `Global` | long-lived | No, provided arrivals are pre-drawn | `mobility/src/demand/poisson.rs:195-238` |
| DesiredSpeed, ReactionTime driver traits | `custom(…, trip.seq)` | long-lived, keyed by the **demand** trip | No. This is the right pattern | `mobility/src/engine.rs:386-416, 2948` |

### 1.2 Identity is assigned in realised order

- `ActorId::new(self.next_actor)` is assigned at successful insertion (`engine.rs:3147-3150`). A trip whose origin is `Occupied` waits up to 120 s and retries (`engine.rs:4929-4945`). `NodeId::new(self.next_node)` is likewise a running counter (`run.rs:1905, 3168`), and nodes beyond the radio margin are created late (`run.rs:3269-3275`).
- So any intervention that changes traffic near an origin (a braking platoon, a removed vehicle, a GLOSA cap) can delay one insertion. That **permutes the ids of every vehicle inserted afterwards**, and with them their Mobility, LaneChange, equipped flag, MacBackoff, SpsSelection, Shadow-link, Crypto and Report streams.
- Phase-parallel merges and the parallel lane-change conflict resolution are ordered by id (`rng.rs:27-30`; BUILD-STATUS "ordered parallel decide pass"). A permutation therefore also changes tie-breaks.
- A removed vehicle is the worst case: every later id shifts by one. "Remove vehicle v and compare the other vehicles" is a different random world for everyone spawned after v.

### 1.3 The scheduler is not the problem

- Events are ordered by `(time, fixed class priority, global seq)` (`v2xw-core/src/event.rs:1-13, 328-333`). Adding events re-stamps later `seq` values, but it never reorders two pre-existing events relative to each other.
- No RNG key uses `seq`, `SduId` or `FrameSeq` (grep of every `LinkFrame` call site: frame keys are times or slots).
- So event order matters only through which **draw index** an event gets. That is exactly the failure mode Buffalo et al. formalise: "any change in control flow shifts the draw index used for all downstream events" (arXiv 2603.11084, abstract).

### 1.4 The engine's own claim is already hedged

`v2xw-engine/src/sensitivity.rs:4-8` says same seed gives common random numbers "wherever the change does not reach". §1.1–1.2 show that the reach is much wider than one would guess. It runs through the candidate-range link set, the conditional lane-change draw, `more_queued`, and insertion order.

## 2. Which interventions give which kind of comparison

"Exact" here means the paired worlds share every exogenous draw for every event both worlds contain, so a per-unit difference is a causal effect within the model.

| Intervention | Used in | Today | Why |
|---|---|---|---|
| Detector or threshold scored **open-loop** on a recorded run (output does not change any action) | D2 RQ "does F1 predict…"; D3 detector conclusions | **Exact, trivially.** No rerun is needed: replay one recording through every detector | Nothing downstream consumes the decision |
| Receiver-side **discard** of messages (local MBD drop, `Revoked` drop) that feeds only CACC/platoon or GLOSA | D2 closed loop; D1 victim neighbours | **Exact up to the first kinematic divergence; then coupled, drifting.** Per-unit pairing is valid near the intervention. Elsewhere it degrades as positions change the candidate link set (Shadow index shift), lane-change draws skip, and any delayed insertion permutes ids | §1.1 Shadow and LaneChange rows; §1.2 |
| Backend-only change (MA policy, report threshold, LA timing) **before** a CRL reaches vehicles | D1 RQ1.1, RQ1.3 | **Exact for the backend timeline** if report streams are keyed by report identity. They are not today: Report and ServiceTime are keyed by node with a running index | `poison.rs:307`; `cellular.rs:823` |
| Revocation that **silences or re-enrols** the victim (it stops transmitting, or changes certificate size and airtime) | D1 RQ1.2 victim cost | **Distribution-level.** Channel occupancy changes, so contention timing changes, so frame times change. Every affected frame then gets a fresh fading and drop draw, and `more_queued` and SPS reselection indices shift for neighbours | `mac.rs:705-714`; `fading.rs:229` |
| Post-quantum or other signature-size change, DCC rate, transmit power | D3 lead case | **Distribution-level** for all radio units. Coupled for mobility only while the loop is open | Same mechanism |
| Remove or add a vehicle, change demand or density | D1 framing "victim absent"; D3 traffic factors | **Distribution-level** for every vehicle inserted later | §1.2 |
| Any Direction 3 factor (propagation law, PHY tier, driver model) | D3 | **Distribution-level** by construction: the factor changes which draws exist | Sobol and Morris need only this, so D3 is fine |

What this means for the directions:
- **D1.** RQ1.1 and RQ1.3 (cost, policy) can be exact once report streams are re-keyed. RQ1.2's safety and awareness cost of a wrongful revocation is a distribution-level estimate. Claim it as an average treatment effect over seeds, with confidence intervals, not as "the harm to this victim".
- **D2.** The "per-unit net-harm decomposition" is honest only for open-loop scoring and for the near-field of closed-loop discards. Its FP-harm leg needs the warning-to-driver loop (§2.4). That loop does not exist today.
- **D3.** Unaffected in principle, since global sensitivity analysis needs only distribution-level comparisons. But "paired reruns reduce variance" is a hope, not a fact: for radio factors the coupling largely breaks. Measure the achieved variance reduction instead of assuming it.

### 2.1 Even perfect event keying does not give per-unit effects everywhere

- **Unit identity has to be defined across worlds.** A message is (stable sender id, application message counter), not (node index, time). A vehicle is `trip.seq`, not `ActorId`. A message that exists in only one world (sent only because the victim was not silenced) has no counterpart, so its "effect" is undefined. Report it as created or destroyed, not as differenced.
- **Discrete decisions amplify.** Lane changes, gap acceptance and insertion make traffic sensitive to small perturbations. A per-vehicle difference 2 km from the intervention can be a real in-model effect that nobody would call "harm caused by the detector". The defensible estimand is spatially and temporally local (a radius and window around the intervention), or an aggregate over seeds.
- **Fading keyed by exact frame time** gives a frame shifted by one back-off slot (tens of µs) an independent draw. The coherence time at 5.9 GHz and 30 m/s is about 1 ms (maximum Doppler about 590 Hz), so physics says the draw should be nearly the same. Keying fading by (link, message id) with time-correlated evolution is both more physical and better coupled.

### 2.2 RQ2.4 as a contribution is thin

- Buffalo et al. already make the general argument with a structural-causal-model formalisation (arXiv 2603.11084).
- The engine's baseline is not the naive global generator they criticise: it is already counter-based and entity-keyed.
- An honest RQ2.4 compares *entity-keyed (current)* with *event-keyed (to build)* on V2X interventions, and measures the spurious-difference share. That is a solid validity section or a VNC short paper. It is not a CCS contribution.
- Widely used V2X tools reuse seeds through stateful per-module or per-object streams. Examples are OMNeT++ RNG mapping under Veins (https://doc.omnetpp.org/omnetpp/manual/#sec:config-sim:rng) and ns-3 `AssignStreams` (https://www.nsnam.org/docs/manual/html/random-variables.html). So the point does generalise, but only as a measurement.

### 2.3 H7 must be able to fail

Per the project's own rule (checks that cannot fail), H7 needs a **placebo intervention**: a change provably inert in the model that still perturbs control flow. Examples:
- a detector that never flags but runs the same code path and schedules a no-op event;
- a one-microsecond insertion jitter on a vehicle far away.

Then report the share of units whose outcome differs. With entity keying this share should be nonzero, which shows the check can go red. Event keying should drive it to zero.

### 2.4 The outcome variable does not exist yet for human drivers

- The applications compute FCW, EEBL, IMA, LTA, BSW, LCW and RLVW warnings and label them against ground truth (`v2xw-node/src/apps.rs:1-30`; `run.rs:4324-4333`). No warning changes how anyone drives.
- The only V2X-to-mobility couplings are the GLOSA speed cap (`run.rs:286-386`) and the CACC/platoon feed (`run/coop.rs:1-20`, which drops `Invalid`/`Revoked`). Scripted `Brake` commands come from scenario events, not apps (`run.rs:2686-2720`).
- So, for a human-driven fleet, a detector's false positive or a wrongful revocation **cannot** change TTC, PET, DRAC or hard braking today.
- Direction 2 must either restrict its safety claims to CACC/platoon and GLOSA, or add a driver warning-response model (compliance probability, reaction-time distribution, braking profile). Such a model's parameters dominate the result. They need field or driving-simulator sourcing (for example yan2014driving) and must be a Direction 3 factor, swept rather than fixed.

## 3. What must change in the engine (ordered by leverage)

1. **Stable identity keys.** Key every per-actor stream by the demand identity (`trip.seq`, as driver traits already are), and every per-node stream by a stable node identity derived from it, not by `next_actor` or `next_node`. This alone removes the global desynchronisation of §1.2. It changes every golden digest, so it needs an ADR 0004 amendment and a re-bless.
2. **Event-keyed draws where the draw count is state-dependent.**
   - Shadow: per (link, sender message id), or better a spatially consistent field keyed by position, which TR 37.885-style spatial consistency wants anyway.
   - Legacy AbstractRx shadow: per (link, frame).
   - LaneChange: per (trip, decision step).
   - MacBackoff: per (node, SDU application id, attempt).
   - SpsSelection: per (node, reselection epoch).
   - Report, ServiceTime, Backend, Crypto: per (stable reporter, report counter) or (request id).

   The existing `LinkFrame` single-use path (`rng.rs:47-58`) is the mechanism to generalise. A generic `EntityRef::Event{scope, stable_id, counter}` would cover all of these.
3. **Coupling audit.** An optional recorder of (domain, entity, event key, draw index) per draw, plus a `v2xw compare --coupling` report: first divergence time, the share of draws matched by event key, and the share of units created or destroyed. Without it, "exact" cannot be shown to a reviewer.
4. **Placebo-intervention test** (§2.3), as an injected-fault check that is shown to go red on the current keying.
5. **Warning-response driver model** (§2.4), with its parameters exposed as sweepable factors.
6. **Open-loop replay harness**: run N detectors over one recording. This gives D2's ranking-agreement experiment exactly and at a fraction of the compute (§4.3).

## 4. Critique of the shared methodology beyond pairing

### 4.1 Sim-to-real

- The engine's validation is **simulator-to-simulator** (SUMO cross-validation, `docs/validation/cross-validation.md`) and **theory-direction** checks (`sensitivity-*.md`). No security conclusion has been checked against field data. Reviewers will treat "the thesis tests other authors' conclusions" as one model judging another.
- Mitigations:
  - Anchor at least one link per layer to field data: PDR versus distance from a field operational test, a car-following calibration, and SCMS backend timings if Park 2024 or the CAMP reports give them.
  - State, for each conclusion, which layer's fidelity it depends on. This is the Direction 3 envelope, but applied to the thesis's own claims first.
- Direction 3's Arp-style framing invites the reply "your ranges are your choice". Set every range from a cited field or standards source and publish the table.

### 4.2 Surrogate safety measures as outcomes

- **Microsimulated conflicts track field conflicts only after safety-targeted calibration.** Essa & Sayed needed a two-step calibration, including a genetic-algorithm stage on the parameters that drive conflicts, and still found SSAM limitations (DOI 10.3141/2514-06). SSAM validation results are mixed (DOI 10.3141/2432-01). A 2024 reconstruction study revisits that correlation at scale (Qu & Wu, DOI 10.2139/ssrn.4918304).
- **Collision-free car-following** (IDM/Krauss families) does not produce crashes unless errors are injected, so "crashes" is not an outcome. TTC < 1.5 s counts mostly reflect the car-following and driver-error model, not V2X.
- **Engine-specific caveats**, already documented in `v2xw-metrics/src/safety.rs:5-14`: TTC ignores vehicle length, and PET is cell-based. These must be in the threats-to-validity section.
- **Better outcomes:**
  - report a *difference in surrogate counts between paired worlds*, with the driver-model factors swept;
  - add an exposure-normalised collision-probability estimate as in Joerer et al. (DOI 10.1109/TVT.2013.2287343), which was built for exactly the "what does lost awareness cost" question;
  - keep non-safety outcomes (awareness gaps, time to neutralise, linkability) as primary where possible. They are model-light.

### 4.3 Statistical power, replications and compute

- **The unit of replication is the seed (run), not the message or vehicle.** Messages and vehicles in one run are dependent through shared traffic and channel, so per-unit pairs need cluster-robust or seed-level inference. Pawlikowski et al. found most telecom simulation papers of the time reported neither confidence intervals nor run counts (pawlikowski2002credibility). The pattern persisted a decade later (sarkar2014revisiting).
- **Rare outcomes need many seeds.** Wrongful revocations and safety-critical conflicts are rare per run. Size the seed count from a pilot variance (paired-difference SD), and report the achieved CRN variance reduction.
- **Multiple comparisons.** About 10 detectors × thresholds × attack mixes gives hundreds of contrasts. Use Holm or Benjamini–Hochberg corrections. For ranking claims, bootstrap Kendall τ over seeds with confidence intervals.
- **Compute is binding.** On the owner's machine the engine runs at 1.74 wall-s per sim-s with 145 vehicles and 14.3 wall-s per sim-s with 453 vehicles (`docs/validation/test-run-cost.md`, 2 threads, one heavy job at a time).
  - A 600 s run at 453 vehicles takes about 2.4 h. A D2 design of about 120 cells × 20 seeds × 3 arms is about 7,200 runs, roughly 17,000 h at that size, or about 2,100 h even at 145 vehicles. Infeasible.
  - A Sobol design for D3 needs N(k+2) runs per conclusion (N ≈ 500, k ≈ 20 gives about 11,000).
  - **Required:** open-loop replay for detector ranking (§3.6); Morris only (r ≈ 20 trajectories, about 420 runs) with Sobol on at most 4–5 surviving factors; small fleets in the near-field; or cloud compute budgeted up front.

### 4.4 Failure modes that get simulation-based security papers rejected

**Arp et al.'s ten pitfalls** (arp2020dos; arXiv 2010.09470; published at USENIX Security 2022, not 2020 as the catalog says), mapped to these directions:
- **Sampling bias / lab-only evaluation (P1, P9):** one map and one demand profile. Use at least two networks and densities.
- **Label inaccuracy (P2):** GNSS error and benign faults labelled "attack". VeReMi-style constant-offset attacks are trivially separable (VeReMi Extension, DOI 10.1109/ICC40277.2020.9149132; a 2026 successor, VeReMi NextGen, DOI 10.1109/VNC69225.2026.11629123, signals that VNC reviewers know this).
- **Data snooping and biased parameter selection (P3, P5):** detector thresholds tuned on the evaluation seeds. Hold out seeds and maps.
- **Inappropriate baseline (P6):** compare with the plausibility checks of F2MD and VeReMi-era work, not with "no detector".
- **Inappropriate measures and base-rate fallacy (P7, P8):** attacker prevalence in the field is tiny. Report results at realistic base rates. This is in fact D2's core point, so make it explicit.
- **Inappropriate threat model (P10):** insiders with unlimited pseudonyms versus SCMS butterfly and pseudonym-pool limits, and colluders outside the reporting rate limits of J3287.

**Kurkowski et al.'s "incredibles" credibility axes** (MobiHoc 2000–2005 survey, DOI 10.1145/1096166.1096174):
- **Repeatable:** engine commit, scenario YAML, seeds and RNG keying version (the key-derivation contract in `rng.rs:16-23`) are released.
- **Unbiased:** scenarios not chosen to favour the proposed policy.
- **Rigorous:** the scenario actually exercises the mechanism. For example, the victim's neighbours depend on its BSMs, which needs the warning loop.
- **Statistically sound:** warm-up removed, independent replications, confidence intervals.

**V2X-specific rejection reasons to pre-empt:**
- Results only in your own unvalidated simulator. Mitigate with cross-simulator replication of one headline result in Veins or Artery.
- Ignoring congestion and DCC effects of security overhead.
- Short runs that never reach CRL propagation timescales. SCMS revocation latency is minutes to hours, while most V2X runs are tens of seconds.
- Attacks with no cost model.
- Undefined "harm".

### 4.5 Smaller points

- The sensitivity framework's "same seed = CRN" text (`sensitivity.rs:6-8`) should say which domains stay coupled under which changes, or link to a coupling audit.
- D1's privacy metric (CRL linkability) is model-light, close to analytical, and does not need paired reruns at all. It is the most robust result in D1, so lead with it.
- Direction 3's "re-test other authors' conclusions" needs their scenarios reproduced faithfully. Budget for the common case where the original configuration is underspecified (Kurkowski's repeatability finding again).

## 5. Recommended re-wording of the common instrument

Replace "exact paired counterfactual reruns across traffic, radio and the SCMS" with:

> Paired reruns with common random numbers, keyed by stable entity and event identity. They are exact per unit for open-loop and backend-only interventions, and for the near field of closed-loop interventions. Elsewhere they are distribution-level estimates over seeds, with coupling measured by a placebo-validated audit.

This is defensible today once fixes 1–4 land. It is also the version that survives a reviewer who has read Buffalo et al. 2026.

## Sources

- Engine: `crates/v2xw-core/src/rng.rs`, `crates/v2xw-core/src/event.rs`, `crates/v2xw-engine/src/run.rs`, `crates/v2xw-engine/src/run/coop.rs`, `crates/v2xw-engine/src/sensitivity.rs`, `crates/v2xw-radio/src/{fading,prop,mac,sps,abstract_tier,cellular}.rs`, `crates/v2xw-mobility/src/{engine.rs,lanechange/mobil.rs}`, `crates/v2xw-threat/src/{poison,attack_rsu}.rs`, `crates/v2xw-node/src/apps.rs`, `crates/v2xw-metrics/src/safety.rs`, `docs/validation/{cross-validation,test-run-cost}.md`.
- Buffalo et al., "Realizing Common Random Numbers: Event-Keyed Hashing for Causally Valid Stochastic Models", arXiv 2603.11084 (2026).
- Arp et al., arp2020dos (arXiv 2010.09470; USENIX Security 2022).
- Kurkowski, Camp, Colagrosso, "MANET simulation studies: the incredibles", DOI 10.1145/1096166.1096174.
- pawlikowski2002credibility (DOI 10.1109/35.978060); sarkar2014revisiting (DOI 10.1109/MCOM.2014.6815915).
- Essa & Sayed 2015, DOI 10.3141/2514-06; SSAM intersection validation, DOI 10.3141/2432-01; Qu & Wu 2024, DOI 10.2139/ssrn.4918304.
- Joerer et al., DOI 10.1109/TVT.2013.2287343; yan2014driving (DOI 10.3390/s140203631).
- VeReMi Extension, DOI 10.1109/ICC40277.2020.9149132; VeReMi NextGen, DOI 10.1109/VNC69225.2026.11629123.
- OMNeT++ manual, RNG configuration, https://doc.omnetpp.org/omnetpp/manual/#sec:config-sim:rng; ns-3 manual, random variables, https://www.nsnam.org/docs/manual/html/random-variables.html.
