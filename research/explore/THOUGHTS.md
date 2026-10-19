# Research-direction thought log

A running record of what has been considered, why ideas were kept or killed, and the criteria
every new idea must meet. Newest entries at the bottom.

## Owner's criteria (2026-10-18)

A direction must do all three at once:
1. **Novel and underexplored**, judged against the 11,315-work corpus and fresh searches, and
   surviving a blind prior-art hunt and a hostile reviewer.
2. **Worthwhile and relevant**: a named adopter (standards body, operator, industry, regulator,
   research community) would change a decision because of the result. Not a narrow, safe gap.
3. **Uses the engine**: the engine must produce evidence that could not be obtained analytically
   or from existing datasets. It need not be the whole contribution, but the paper must be weaker
   without it.

Owner preferences so far: liked "which evidence can frame an honest vehicle"; found "pricing
detector errors", "who can follow an honest vehicle" and the three-way robustness study too
small; rejected "harvest now, link later" because the core is analytical and barely uses the
engine.

## Lessons from earlier rounds (what kills an idea)

- **Generic framings are taken.** First SCMS simulator (VESNOS, 2018 emulator); closed-loop
  detection-to-safety (A4MD, Kamel 2020); "security costs safety" (since 2008; Pethő found the
  effect small); PQ migration order (Twardokus 2024, Chen 2026, Fikry 2026); LLM agents that
  configure simulators; cross-layer linkability channels (mostly published).
- **Strawman assumptions die.** A report-counting MA is not what standards specify (they are
  proof-based). Real revocation takes days, so "in-run revocation" claims need care.
- **Engine claims must match the engine.** RNG streams are keyed per entity, not per event, so
  exact per-unit counterfactuals hold only for open-loop replay, backend-only changes and local
  effects. V2X warnings do not yet change driving (only GLOSA and platooning close the loop).
  Cost: ~2.4 h per 600 s run at ~450 vehicles locally; small fleets ~5 min.
- **Purely analytical cores underuse the engine** (harvest-now-link-later).
- **Verify sources.** Several earlier "facts" were wrong: the FROST claim, the 5GAA misreading,
  NIST dates, Chen's scope.

## The engine's unique assets (what only it can evidence)

- Standards-exact US SCMS: real 1609.2/1609.2.1 formats, official test vectors, real SCMS Manager
  trust lists, multi-provider CTL, ECQV, P2PCD, real one-week calendar (realism project).
- The full loop in one deterministic run: traffic → radio (DSRC, LTE-V2X, NR-V2X) → 1609.2 →
  misbehaviour detection → reports → MA → linkage authorities → CRL → RSU/cellular distribution →
  enforcement.
- Per-message ground truth across traffic, radio and PKI (true vehicle, pseudonym, certificate,
  linkage value, loss cause, attacker label).
- Backend realism: cellular TCP/TLS provisioning and top-ups, RSU backhaul, eMBMS, real
  provisioning schedules; post-quantum and threshold variants.
- Scale and breadth: 50+ US city maps, a ~300-setting sensitivity/expectation harness.

## Documented field needs (from big-needs.md and earlier rounds)

MA remediation policy is unspecified (5GAA MBD-Op 2024); multi-provider trust and MA cooperation
("should", not "shall"); revocation reach and cost (qualitative only); PQ channel load in the
30 MHz band; pseudonym supply (NYC raised 20 → 60 per week); detector performance and report
volume requirements (J3287 fixes the format only); backend load at the USDOT 2028-2036 targets.

## Status of candidates considered so far

| Candidate | Status | Why |
|---|---|---|
| Which evidence can frame an honest vehicle | Alive (owner liked) | Narrow but real; needs a proof-based MA model |
| SCMS-Bench (safety-scored benchmark) | Alive, risk | Partly done (Zhang 2024, Kalogiannis 2022, Wehmer 2021); Ulm group closing in |
| Harvest now, link later | Parked | Holds up but core is analytical; engine supporting only |
| Pricing detector errors | Dropped by owner | Too small |
| Who can follow an honest vehicle | Dropped by owner | Too small; mostly published |
| Why V2X security studies disagree | Dropped | No real contradictions found |
| Threshold SCMS authorities | Dropped | Too narrow; FROST VPKI exists (ajibuwa2023) |
| Standalone PQ channel study | Dropped | Crowded; funded USDOT projects |

## Round 2026-10-18: engine-aware search

Sources: ideas-needs.md (N1-N7), ideas-engine.md (D1-D7), screen.md, check-/review- files for the
four finalists, final.md (the judgement and full survivor plans).

| Candidate | Status | One-line reason |
|---|---|---|
| **S1. Honest but wrong** (A = N1 + D1 `benign-mbr-load`) | **Survived, refined** | Deployed US MBR rule's benign trigger rate, store crowd-out and cross-vehicle cause attribution are unmeasured; engine gives receiver-side gaps, cause labels and the full SCMS chain. Must calibrate GNSS first |
| **S2. Sound and live MA evidence policies** (owner-liked framing + N3 kernel + C kernel) | **Survived, new merger** | Sybil reporters (20 concurrent pseudonyms) vs reporter de-dup privacy, and forward-to-issuer vs revocation-request cooperation, are unpublished; engine gives witness availability by evidence class and ground-truth soundness |
| N3. Bootstrap paradox (standalone) | Killed | Trade-off is prior art (Bißmeyer 2012, Moore 2008, Intel US12489779B2); adopter vague; 2028 penetration ~0 so it collapses to RSU coverage; framing driven by de-dup, not penetration |
| C. Multi-provider operations (standalone; D2 + D5) | Killed | Sharing is already SHALL (Provider Req §2.2); split is binomial thinning; delay additive; policy shopping follows from architecture |
| D5 `ctl-change-propagation` (CTL arm of C) | Killed | ≈ provider share x stale fraction under well-mixed traffic; stale fraction has no deployment data; concept from web-PKI root-store studies |
| N4. Compromised intersection | Killed | Encrypted MBRs mean no subject filtering; RSUs neutralised by non-renewal (169 h) and operators, not CRLs; headline tautological; engine cannot show harm. Parked until a driver-response model exists |
| B. Systemic credential events (N2 + N6) | Killed (alternate) | Awareness loss is near-analytical; crossover rests on an assumed extraction rate |
| N5. Unpinned VRU channel | Killed | Framed already (arXiv 2010.15502, 2402.17763); adopter decision unverified (J2945/9 unread); depends on unvalidated phone GNSS/VRU models |
| N7. Batch of 20 simultaneous certificates | Killed | Trade-off stated in Brecht 2018 p7; privacy axis is the dropped "who can follow"; its Sybil-reporter angle lives on in S2 |
| D3 `rsu-cert-availability` | Killed | Answer already obvious (USDOT recommends 3-year preload); ≈ outage fraction x hours |
| D6 `us-pki-audit` | Killed as thesis direction | Core is an audit, not simulation; keep as side contribution after responsible disclosure |
| D4 `scms-calendar-load` | Killed standalone | RA burst analytical unless cell contention binds; recovery surge went to B |
| D7 `obu-verify-budget` | Killed standalone | Narrow, same class as the dropped "pricing detector errors"; possible chapter of S1 |
| N6 (alone) | Killed | Fails the engine criterion alone; merged into B |

### New lessons from this round

- **Read the spec rule's exact statistic.** gpsDrift is a fix-to-fix dead-reckoning residual, so
  absolute GNSS error figures say nothing about it. Check what the rule actually computes first.
- **Engine noise models can manufacture headlines.** The 10 Hz white GNSS term (σ 1.6 m, a 1 Hz
  default) makes ~91% of honest BSM pairs fail the 1 m rule. Calibrate against real logs (THEA,
  Wyoming) before any count-type result, and run noiseless null runs.
- **Check what the adversary can see under the real formats.** MBRs are signed then encrypted to
  the MA; an engine attacker reading plaintext fields is a modelling error (attack_rsu.rs:377).
- **"The MA cannot see X" claims need the evidence format checked.** Evidence BSMs already carry
  PositionalAccuracy; issuer CAs may reveal a coarse class.
- **Use real data as the anchor, the engine for what data cannot give** (receiver-side reception,
  counterfactual causes, the SCMS chain). This answers "why simulate" up front.
- **Separate self-proving evidence (TS 103 759 classes 1-2) from witness-dependent evidence
  (3-5)** in any MA study; framing and thresholds only matter for the latter.
- **Day-scale SCMS timing vs 600 s runs:** measure evidence and rates in the engine, extend to
  days with a labelled open-loop surrogate; never present extrapolated revocation times as measured.
- **Several reframings converge on one MA model**; build it once (shared by S1 and S2).
