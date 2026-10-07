# 07 — Threats, detection, response, privacy

Status: design draft for review (2026-09-18). Interfaces in `03-interfaces.md` §9; protocol hooks in `05-protocols.md` §2.6; metrics in `08-measurement-and-data.md`.

## 1. Attacker model

An attacker is a plug-in that sees only an `AttackerView` and acts only through an `AttackerApi` (03-interfaces §9). What it can see and do is declared in `Capabilities`; the engine enforces the declaration:

| Capability | Values | Enforcement |
|---|---|---|
| Credentials | `None` (no valid credentials), `Own(n)` (its own n concurrent pseudonyms/ATs; SCMS default 20 per week, ETSI ≤ 100 or 20 under the C2C-CC profile, 05-protocols §2.2), `Stolen(set)` (credentials extracted from k other devices), `CompromisedRsu(id)` | `AttackerApi::use_credential` only accepts handles in the declared set |
| Radio | max power (dBm), can jam (yes/no), channels, RAT | the MAC/PHY reject frames outside the declared envelope |
| Knowledge | public CRL/CTL (yes/no), map (yes/no), neighbors via own receptions (always), sensing/perception (yes/no) | the view only contains what is declared |
| Coordination | `CoalitionId` with a modeled channel (V2X or out-of-band with latency) | coalition messages are ordinary modeled messages |
| Compute | a `HardwareProfile` | signing/flooding rates are bounded by the attacker's own node runtime |
| Position | honest kinematics only; an attacker cannot teleport its body, only its claims | `AttackerApi` has no mobility control beyond driving behaviours available to any driver |

Goals and strategies are the attacker's own logic (safety disruption, tracking, DoS, evasion, framing); the catalog below fixes the mechanisms, and `AttackSchedule` gives every attacker duty cycle, onset jitter, geofence, and wave membership (from the legacy scenario events, 01-inventory §3.3).

Invariants: I-T1 no ground-truth access (compile-time sentinel); I-T3 every action that changes bytes on the air is logged on a GT channel with the true actor id.

## 2. Attack catalog

### 2.1 Ported from the legacy engine (28 types, renderings preserved as defaults)

| Family | Types (legacy names kept) | Mechanism in the new engine |
|---|---|---|
| position | ConstPos, ConstPosOffset, RandomPos, Teleport, SineWavePos | `FalsifyOutgoing` edits the position fields before signing; magnitudes as in the legacy formulas with intensity `k` |
| speed | ConstSpeedOffset, RandomSpeed, StopAndGo | same, speed field |
| heading | ReversedHeading, HeadingOffset | same, heading field |
| combined | Disruptive, PosSpeedInconsistent, PosHeadingInconsistent, EventualStop | several fields |
| timing | DataReplay (own history replay), DelayedMessages, OutOfOrder, DoS, DoSRandom | `Replay`, `Delay`, generation-time edits, rate increase bounded by the node's MAC and DCC (a DoS attacker that disables DCC is a distinct capability) |
| stealth | SlowDrift, AlongRoadOffset | small along-road offsets |
| identity | Sybil (ghost transmissions from concurrent valid pseudonyms), VruImpersonation, VruPositionSpoof | `UseCredential` cycling across the attacker's own concurrent credentials; station-type field edits |
| credential | InvalidSignature, ExpiredCert, NotYetValid | envelope-level edits (wrong key token, expired/not-yet-valid certificate) |
| event | FakeHazard (phantom DENM) | `FalsifyOutgoing` on DENM generation |
| collusion (cross-cutting) | colluding reporters framing victims with fabricated evidence | `ForgeReport` with the legacy fabricated-evidence distributions |
| evasion (cross-cutting) | CRL-aware dormancy | reads the public CRL through the view (declared knowledge), not the MA's internal state |

### 2.2 New types required by the brief

| Type | Mechanism | Needs |
|---|---|---|
| Ghost vehicles | transmit BSMs for non-existent vehicles from one radio using extra valid pseudonyms (Sybil) or from an RSU-class attacker; positions generated along lanes (map knowledge) so map-off-road checks pass | `Own(n)` or `Stolen`, map |
| Sybil with concurrent valid pseudonyms | as above, with per-ghost consistent kinematics and rotation strategies | `Own(n)` |
| Replay and delay | store received messages, retransmit later or elsewhere (relay attack between two attackers) | radio, coalition for wormhole |
| Message suppression | selective non-forwarding (GN multi-hop, DENM keep-alive, CRL/CTL rebroadcast) | RSU or forwarding role |
| PHY jamming and flooding | `TransmitRaw` with a jammer profile: constant, reactive (triggered by CCA busy), random duty cycle, power up to declared max; modeled as an interferer in the SINR sums (04-models §12.3) | can jam |
| Oversized-message flooding | high-rate transmissions of maximum-size SPDUs (PQ or threshold profiles) to exhaust receivers' verification budgets and reassembly buffers | valid or invalid credentials; DCC disabled as a capability |
| Compromised RSU | false SPaT/MAP, false CRL/CTL (signed with the RSU's real credentials or unsigned), report suppression or poisoning at the forwarding stage | `CompromisedRsu` |
| Insider with valid credentials | any falsification while holding a fully valid credential set; combined with evasion and rotation | `Own(n)` |
| Misbehavior-report poisoning | forged reports against honest vehicles with plausible evidence; report floods to exhaust MA budgets | valid credentials |
| False DENM / CPM events | phantom hazards, phantom perceived objects in CPM | valid credentials |
| Certificate misuse across regions | using credentials outside their validity region (1609.2 `region`, ETSI AT region) | `Own(n)` |
| Location tracking (privacy attacker) | passive observer with RSU-like receivers at configurable density; links pseudonyms across changes using kinematic continuity (Kalman/MHT) | knowledge: none beyond receptions |
| Coordinated campaigns | coalition of any of the above with a modeled coordination channel (V2X or cellular with latency) | coalition |

### 2.3 Foundry

The MAP-Elites foundry (01-inventory §3.5) is ported over the new `Attacker` interface: the genome becomes a scenario overlay (attack mix, magnitudes, density, topology template, weather, realism toggles, hardware profile), the descriptor keeps the four legacy axes (family, density band, topology, attacker band) and gains `rat` and `protocol`, the objective reads `det_recall`, `time_to_detect`, or `residual_harm` from the metrics, and the LLM mutation operator hook is unchanged. Elite replay is a scenario file plus a seed.

## 3. Detection

### 3.1 Local detectors (on-node)

The legacy 12-detector suite plus the Kalman soft feature is ported as the `legacy-12` plug-in with the same formulas and normalisation (`detnorm ≈ 1 at threshold`, 01-inventory §3.3), because the MA dataset's ML contract depends on those signals. Changes: per-receiver tracker state instead of a global dict; float-equality frozen check replaced by a tolerance; inputs are the node's verified messages, its own position estimate and its neighbor table; costs are charged to the node CPU per message (`Detector::cost`).

New detector families (each a plug-in with a model card): TS 103 759 observation classes 1–5 (implausible values; inconsistency with previous messages from the same station; with the local environment/LDM including map and signal state; with on-board sensors, i.e., perception cross-check; with other stations' messages); F2MD-style checks re-implemented from the paper descriptions (range, position, speed, consistency, sudden appearance, beacon frequency) with their thresholds as cited defaults in 04-models §14; perception cross-check (claimed position vs sensed objects within the sensor FOV, with occlusion in the medium tier) for ghost-vehicle research; CPM consistency (perceived objects vs own perception).

### 3.2 Misbehavior authority pipeline

`MaPipeline` receives reports through the protocol's reporting transport (real delays and batching) and runs: ingestion and validation (re-verify evidence signatures, charge the MA's service model), correlation (per-subject windows; the legacy operating point k = 3 trusted reporters, 4 distinct seconds, 3 s span, 15 s window, reporter budget 30 and reputation cap 40 ships as `legacy-window`), investigation (protocol-specific identity resolution as a flow with round trips: SCMS PCA + LA queries; ETSI EA lookup), decision (revoke, dismiss, suspend, alert), and hand-off to the `Responder`, which issues the protocol's revocation flow. Researchers can replace any stage with a Python plug-in (e.g., an ML model over the MA dataset features) and get scored by the same metrics.

### 3.3 Response

The responder emits the revocation flow and the engine timestamps every stage (05-protocols §8), so revocation latency decomposes into report transport, MA processing, resolution round trips, issuance, distribution per path, download, processing, and enforcement, per node.

## 4. Ground truth, labels, and the leakage firewall

Attack actions are recorded on `gt.attack.action` with the true actor, the fields changed and the magnitude, which yields per-message `falsified` labels (legacy rule: position error > 1 m, speed > 1 m/s, heading > 5°, extra messages, stale generation time, bad signature, bad certificate, or false station type) and per-vehicle labels. Detectors, the MA and all exporters marked `NODE` never see those channels; the leakage linter runs on every exported feature table.

## 5. Safety outcomes

Safety applications (FCW, EEBL, IMA, VRU warning, 04-models §11) consume the neighbor table; their warnings are joined with ground truth offline to count true, false (ghost-induced) and missed warnings, and surrogate safety measures (TTC, PET, DRAC) quantify the physical consequence of attacker-induced braking or lane changes when the mobility tier lets drivers react to warnings (a scenario option).

## 6. Privacy metrics

- **Observer model:** passive receivers at RSU sites with configurable density and coverage; the observer's tracker is a plug-in (default: Kalman-filter multi-hypothesis tracking as in Wiedersheim et al. 2010, whose results showed that at 1 Hz beacons a change interval ≥ 4 s and 20 % penetration already yields near-100 % tracking success [WONS 2010]).
- **Linkability rate:** fraction of pseudonym changes the observer links correctly, by change strategy (time, distance, mix zone, silent period), density and RSU density.
- **Anonymity set at change:** Ψ = target plus vehicles within the observer's confusion region at the change instant; effective size S = −Σ p_u log₂ p_u and degree of anonymity d = S / log₂|Ψ| with p_u the observer's posterior (ETSI TR 103 415 §5.1.2).
- **Tracking duration:** mean correctly tracked time per vehicle (WONS 2010 method).
- **Strategy defaults to compare:** SCMS 5 min / 2 km; C2C-CC BSP 10–30 min random after a 1-minute change at ignition; C2C-CC segment strategy (800–1,500 m then ≥ 800 m and 2–6 min); PRESERVE's 120 s plus 3–13 s silent period (05-protocols §2.4, all cited there).
- Concurrent-pseudonym count is reported alongside, since TR 103 415 §8 notes that the Sybil surface grows with it.

## 7. Conformance and evaluation harness

Every attacker ships a model card and a "minimal reproduction" scenario; every detector ships expected precision/recall on the Phase 2 reference scenario; the evaluation harness (`v2xw eval detectors`) runs a detector set against the attack catalog with seeds and produces the same tables the legacy `validate.py` and `benchmark.py` produce, so results remain comparable with the current repository's numbers.

## 8. As built and measured (2026-10-06)

What the engine runs today, and what it measured. Every number below is from
`crates/v2xw-engine/examples/mbd_eval.rs` (debug build, one seed, `0xC0FFEE_5EED`), which
prints the run report's ground-truth joins; nothing in the detection path reads them.

### 8.1 The detection path

- **Detectors** (`detect/legacy-12`, on every vehicle and roadside unit, one check per
  sender per second, two consecutive violations to fire). Every check that compares
  positions is confidence-range tolerant in the sense of CaTch (Kamel et al., IEEE WCNC
  2019): a claim is implausible only if no point inside its stated confidence range makes
  it plausible.
  - The confidence is the message's own: a BSM's J2735 `PositionalAccuracy` (one sigma,
    scaled to 95 %), a CAM's 95 % ellipse, floored at 2.5 m (half the legacy 5 m
    consistency threshold) so a sender cannot tighten its own check by claiming
    centimetres. `use_stated_accuracy: 0` restores the legacy constant 5 m.
  - Heading: the claimed heading against the bearing over the longest straight baseline
    (the sender's own headings within 10°, up to 10 s), with the threshold widened by the
    bearing error the two stated radii allow, `asin(c / Δs)`.
  - Map: each receiver holds the motor-vehicle lanes (F2MD's position-plausibility
    check); a claim is off the road when its whole confidence disc lies more than 15 m
    beyond the carriageway edge.
  - A PSM sender is checked as a vulnerable road user (no vehicle kinematic or map check).
- **Reports** (TS 103 759 / SAE J3287 shape): at most one per reporter per subject per
  second, over the vehicle's cellular link or a roadside relay, through the RA's shuffle.
- **Authority** (`threat/ma/corroborated`). Neither the SCMS design (Brecht et al. 2018
  §VI) nor TS 103 759 nor J3287 specifies the decision rule, so it is a stated design
  choice:
  - reports about one pseudonym observed within 5 s of an event's first are one event
    (F2MD's `DELTA_REPORT_TIME`), however many receivers heard it;
  - an event counts when two distinct trusted reporters witnessed it;
  - revocation takes three such events inside 60 s from at least three trusted
    reporters — ten seconds or more of sustained, independently witnessed misbehaviour;
  - reporter trust (budget 30, reputation cap 40, legacy) counts (subject, event)
    pairs inside the window, a rate rather than a run-long total.
- **Revocation**: the PCA and both Linkage Authorities resolve the device, the CRL
  Generator issues a linkage-seed entry on its cadence, the CRL Store publishes, and
  vehicles install it by cellular download or roadside broadcast.

### 8.2 Honest revocations, attackers removed

Every shipped scenario that runs detection, with `--no-attackers` (the attacker
populations and attack waves removed), at its own density and at 6,000 veh/h. A scenario
that declares no `detection.local` runs no detector and no authority, so nothing in it can
be reported or revoked.

| Scenario | Rate, veh/h | Vehicles | Reports (all honest) | Honest vehicles reported | Most corroborated events one pseudonym reached (3 revoke) | Honest revoked |
|---|---|---|---|---|---|---|
| credential-lifecycle (CAMP SCMS) | 1,500 | 55 | 17 | 9 | 1 | **0** |
| credential-lifecycle | 6,000 | 218 | 227 | 58 | 2 | **0** |
| ccms-lifecycle (ETSI CCMS) | 1,500 | 54 | 8 | 7 | 0 | **0** |
| ccms-lifecycle | 6,000 | 217 | 233 | 51 | 2 | **0** |
MANHATTAN-ROWS

The detectors do fire on honest traffic (credential-lifecycle at 6,000 veh/h: 270 fired
verdicts, naming `positionSpeedInconsistency` 247 times, `positionJump` 42 and
`headingInconsistency` 20, the map check never), because a real receiver under-reports its error inside a multipath burst and
a real fleet's detectors fire on it. What the authority's rule prevents is the revocation
that used to follow: no honest pseudonym reached a third corroborated event. The margin is
in every run report (`ma_peak_unrevoked_events`) and in the Backend view.

### 8.3 Precision, recall and latency per attack type

`scenarios/credential-lifecycle.yaml` (CAMP SCMS, one-minute i-periods, the RA's report
shuffle and the CRL cadence at a minute, 1,500 veh/h for 300 s, 55 vehicles) with 10 % of
the fleet replaced by one attack model each (`mbd_eval --attack KIND --fraction 0.1`),
acting from 60 s. Five vehicles were armed in every run and the same five every time
(nodes 18, 27, 28, 39 and 48; one seed), so the rows differ by the attack alone.

- *Reported*: attackers at least one receiver filed a report about (detection recall).
- *Decided / revoked*: the authority's revocation decisions about an attacker, and the
  distinct attackers whose CRL entry was issued before the horizon.
- *Report precision*: reports about attackers over all reports.
- *Latency*: an attacker's first falsified claim to the first report about it, and that
  report to the authority's decision. The decision waits for the RA's shuffle (a minute
  here), which dominates it.
- *Missed*: each armed attacker not revoked — how long it lied, the reports about it and
  the most corroborated events one of its pseudonyms reached.

ATTACK-TABLE

What it says:

- **No honest device was revoked in any run**, attackers present or not.
- **Blatant kinematic lies are caught**: 4 of 5 for ConstPos, RandomPos, ConstSpeedOffset,
  RandomSpeed, StopAndGo, SlowDrift, Disruptive, PosHeadingInconsistent and EventualStop,
  detected within 2–5 s and decided 28–49 s later (median). The one escape is node 18,
  which lied for 13 s before leaving the map: less than the ten seconds of sustained,
  corroborated misbehaviour the authority requires.
- **Sparse witnesses limit revocation, not detection.** Teleport, SineWavePos,
  ReversedHeading and PosSpeedInconsistent attackers were all reported, but often by one
  receiver at a time (n48 under ReversedHeading drew 113 reports and two corroborated
  events). One witness never corroborates, by design: in a 55-vehicle fleet an attacker
  often has one follower.
- **Self-consistent lies stay hard.** ConstPosOffset (25 m on both axes) went unreported
  until each receiver held a map, and is caught now only where the offset leaves the
  carriageway by more than the tolerance plus the stated confidence (1 of 5);
  AlongRoadOffset (30 m along the road) stays on the road and consistent, and no
  single-receiver plausibility check can see it — the perception cross-check
  (TS 103 759 class 4) is what would.
- **Before this round**, with `NoMap`, the 3.5 s heading baseline and the run-long reporter
  budget, the same harness measured ConstPosOffset 0 of 5 reported and HeadingOffset
  2 reported, 0 revoked; and the run report counted a device revoked twice as two.

### 8.4 Pseudonym-change strategies against a sniffer network

`scenarios/pseudonym-strategies.yaml`: a 5 × 8 Manhattan-proportioned grid (274 m × 80 m
blocks), 1,500 veh/h for 300 s, the CAMP SCMS in the loop, and passive sniffers at a
quarter of the signalised intersections, each reading every safety frame sent within
100 m (`threats.eavesdropper`). The observer links a new pseudonym to a recently silent
one by kinematics (Wiedersheim et al. 2010) and by `msgCnt` continuity when a sender does
not restart it. Linkability is correct links over pseudonym changes; the anonymity set is
the candidates the observer weighed at a link decision (0 when it saw none); the tracking
duration is how long the observer followed each vehicle's true trip, ground truth joined
afterwards.

| Strategy | Rule | Changes | Silenced frames | Linkability | Mean anonymity set | Degree of anonymity | Followed across a change | Mean tracked, s | Max tracked, s |
|---|---|---|---|---|---|---|---|---|---|
| `time` | SAE J2945/1, 300 s | 0 | 0 | - | 1.12 | 0.15 | 0 of 50 | 70 | 239 |
| `distance` | NYC pilot, 2 km | 1 | 0 | 1.00 | 1.12 | 0.15 | 1 of 50 | 70 | 239 |
| `c2c-cc` | C2C-CC BSP RS_BSP_521–524 | 48 | 0 | 0.31 | 1.14 | 0.13 | 15 of 50 | 54 | 148 |
| `mix-zone` | CMIX at signalised junctions, 60 s minimum age | 67 | 0 | 0.16 | 0.99 | 0.13 | 11 of 46 | 31 | 101 |
| `silent` | no change (control) | 0 | 0 | - | 1.12 | 0.15 | 0 of 50 | 70 | 239 |
| `time`, 60 s | | 124 | 0 | 0.27 | 1.18 | 0.19 | 27 of 50 | 53 | 142 |
| `time`, 60 s, 3–13 s silent period | PRESERVE | 124 | 10,020 | 0.06 | 1.56 | 0.35 | 7 of 49 | 38 | 94 |

What it says, with one seed and one fleet:

- **The deployed US rules do nothing inside a city trip.** A five-minute or 2 km change
  rarely falls inside a trip this long, so the eavesdropper follows every vehicle for as
  long as its sniffers hear it, exactly as with no change at all.
- **Changing often is not enough on its own.** A 60 s period changes 124 times and the
  observer still links 27 % of them by kinematics alone.
- **Where and how the change happens matters more than how often.** Mix zones bring linkability
  from the 60 s period's 0.27 to 0.16 with about half as many changes; a 3–13 s silent period after each
  change cuts it to 6 %, at the price of 10,020 safety frames nobody sent — the cost the
  silent-period literature names.
- Every change moves the certificate, the BSM `TemporaryID`, the link-layer address and
  the `msgCnt` together (`security_lifecycle::every_identifier_changes_together_at_a_pseudonym_change`);
  without the `msgCnt` restart the observer links by counter continuity
  (`v2xw-threat/tests/privacy.rs`).
