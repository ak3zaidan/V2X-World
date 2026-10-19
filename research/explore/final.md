# Final judgement: engine-aware search, round 2026-10-18

Inputs: THOUGHTS.md, screen.md, check-/review- files for finalists A, N4, N3, C. Survivors must be
(1) novel, (2) decision-changing for a named adopter, (3) dependent on engine-only evidence.

| Finalist | Review score (as pitched) | Verdict |
|---|---|---|
| A. Honest but wrong | Major revision, borderline reject (4/5) | **Survives, refined (S1)** |
| N3. Bootstrap paradox | Major revision; reject at VehicleSec (4/5) | Killed as pitched; kernel merged into **S2** |
| C. Multi-provider operations | Reject; major if reframed (4/5) | Killed as pitched; MA kernel merged into **S2**, CTL arm dropped |
| N4. Compromised intersection | Reject; major if reframed (4/5) | **Killed** (two of three premises refuted; engine cannot show the harm) |

Two survivors, sharing one proof-based MA model and one multi-day surrogate: the thesis spine
"evidence-grade remediation in the US SCMS" (S1 benign evidence, S2 forged or Sybil evidence).

## S1. Honest but wrong: the benign evidence load of the deployed US misbehaviour-reporting rules, and whether an MA can attribute a shared cause

#### Thesis
The deployed US reporting rule (SCMS Manager MBR v1.0, gpsDrift 1 m dead-reckoning
residual between consecutive *received* BSMs) fires on honest vehicles (hypothesis) through radio
gaps during turns and braking more than through GNSS error. The resulting benign reports
crowd attack evidence out of the spec's on-device store. An MA that aggregates across pseudonyms
using fields it already receives (PositionalAccuracy, issuer CA) can partly separate canyon
clusters and defective device classes from attacks; a cohort token closes the rest, at a priced
privacy and evasion cost.
#### Why it matters, to whom
- SCMS Manager owns gpsDrift, weights, 0.9/day decay and quota (MBR v1.0 §2.1-2.3, pp.4-6); a
  trigger-rate surface over gpsDrift, Δt and loss is a direct parameter input.
- 5GAA MBD4V2X: MBD-Op (2024) says remediation depends on cause (p.13) but leaves cause
  determination out of scope (p.12). S1 measures how much cause signal the MA can recover.
- OEMs and MA operators (recall-or-revoke, MBD-Op §5.3); the SAE J3287 revision.
#### Novel versus the closest work
- Per-pseudonym faulty/attacker/genuine classification exists (Kamel thesis 2020 §6.1; F2MD's 9
  faulty behaviours; Rathore 2023; Das 2026 on Tampa BSMs). Confidence-tolerant checks fix local
  false positives (CaTch). Faulty-node eviction is old (Raya 2007; Ghosh 2009).
- Not found anywhere: (i) the benign trigger rate of the *deployed US rule set*; (ii) any analysis
  of its store/quota/upload policy and crowd-out of attack evidence; (iii) *cross-vehicle*
  attribution of a shared cause (canyon corridor, device class) at a US MA under the privacy
  split, with the disclosure priced. Qualcomm patents (US 12470954, 11613264, 11082846) cover
  sender self-flagging and report routing, not MA attribution; cite them.
- Corrected premises: the rule sees the 100 ms jump, not absolute error; the MA already sees the
  accuracy ellipse; the only genuinely new field is a device-class or defect-cohort token.
#### Engine evidence, and why it cannot come from elsewhere
- Real data supplies the calibration and the first result: run the exact MBR v1.0 checks on public
  THEA and Wyoming CV-pilot BSMs (ITS DataHub). Leading with real data answers "why simulate".
- The engine supplies what logs cannot: (a) *receiver-side* reception gaps at city scale (which
  receiver decoded which BSM under channel load and DCC intervals), the radio-coupled trigger,
  for three RATs; (b) store and upload dynamics with injected attacks carrying ground truth, at
  real RSU density and cellular share; (c) counterfactuals with true cause labels (canyon,
  defect class, attack) for the same vehicles; (d) the full report → RA → MA → LA chain with
  standards-exact formats, so the disclosure cost is measured in real bytes and linkage queries.
  Pilot logs have few receivers, no cause labels and no SCMS layer; VeReMi has no SCMS and no
  canyon or device-class causes.
  Detection, reporting and MA are open-loop or local, so policy variants replay exactly.
#### Experiment plan (sized to ~2.4 h per 600 s run at ~450 vehicles)
1. Real-data baseline: benign trigger rate on THEA/Wyoming by speed, yaw rate, Δt, accuracy.
2. Calibrate the engine's 100 ms innovation model to (1); null runs (noiseless GNSS, perfect
   radio) must give zero reports, and an injected canyon jump must fire (checks can go red).
3. Radio-coupled triggers: 8 maps x 3 loads x 3 RATs on small fleets (~5 min), plus 6 full runs.
4. Crowd-out: 6 full runs with attackers; offline replay over quota, gpsDrift, RSU density, cellular.
5. Attribution: defect-class cohorts (2-3 fault signatures, prevalence swept) and canyon
   corridors; MA family (per-pseudonym, LA-linked per-device, CaTch-style accuracy-aware,
   cross-vehicle clustering on existing fields, plus a cohort token); attacker who claims poor
   accuracy or rides a defect cohort (evasion analysis).
6. Multi-day store and MA via an open-loop surrogate fed by per-corridor, per-class rates.
Total: ~15 full runs (~36 h) plus small-fleet runs and offline replay.
#### Engine extensions (about 3-5 weeks)
Spec-exact MBR check set (RandomPosition, MaxSpeed,
BrakeAcceleration, ObsDistance) 150-250 LOC; filtered-receiver 100 ms innovation model and
calibration 200-400 LOC; location-dependent sky view from building heights 300-500 LOC (today
run.rs:3699 hard-codes OPEN_SKY); defect-class model 200-400 LOC; cause-aware MA variants and
disclosure options with a linkability metric 400-800 LOC; multi-day surrogate ~300 LOC.
#### Venue
USENIX VehicleSec 2027, deadline *inferred* late February 2027 (VehicleSec'25: 27 Feb
2025; '26: 24 Feb 2026; both co-located with USENIX Security). Fallback IEEE VNC 2027 (deadline
not checked) or IEEE T-ITS.
#### Review score
Major revision, borderline reject as pitched (4/5); the reviewer would accept
at VehicleSec with the real-data result plus the radio-coupled and crowd-out results.
#### Remaining risks
(1) The GNSS artefact: current white noise makes ~91% of honest pairs fail;
nothing is reported until calibration (2) passes. (2) MBR v1.0 is a CV-pilot spec; production
may follow J3287 detectors (paywalled, unread). Present surfaces over gpsDrift and Δt, and obtain
J3287 and ETSI TS 103 759 before claiming (iii). (3) The defect-class model is a free parameter;
report results conditional on prevalence and fault signature. (4) No deployed MA rule is public;
show surfaces, never "N honest vehicles revoked".

## S2. Sound and live: which MA evidence policies survive Sybil reporters, sparse witnesses and two providers in the US SCMS

(Merges the owner-liked "which evidence can frame an honest vehicle" with the N3 and C kernels;
both reviewers and both checks pointed to this merger.)
#### Thesis
In the US SCMS, a device holds 20 concurrently valid pseudonyms and only the issuing
provider's LAs can say whether two pseudonyms are one device. For class 3-5 evidence (TS 103 759;
classes 1-2 are self-proving and cannot frame), no witness-threshold policy is both *sound* (one
device cannot frame an honest vehicle) and *live* (a persistent attacker is acted on within the
reporting cycle) at plan-level witness availability unless the MA pays a reporter-privacy cost
(LA de-duplication of reporters) or accepts revocation requests across providers, which opens a
cross-MA framing channel. S2 maps that boundary and prices each way out.
#### Why it matters, to whom
- SCMS Manager: Provider Requirements v1.1 §2.2 makes report sharing a SHALL but sets no deadline,
  format or decision owner; whether MAs may query LAs about *reporters* is a privacy-policy choice.
- 5GAA CP authors: items (24)-(26) are exactly the forward-to-issuer versus request choice.
  Also the MA operators of the two production providers and 5GAA MBD-Op.
#### Novel versus the closest work
Bißmeyer 2012 (central MA, three witnesses, 37% collusion),
Moore/Raya SECON 2008 (voting vs density), F2MD and DARE (MA thresholds, attacker share), Intel
US12489779B2 (penetration-dependent MA area, concept only), Blackchain, SECMACE and 5GAA 2022
(multi-domain MA, qualitative). Not found: the Sybil-reporter versus reporter-linkage-privacy
trade-off under 20 concurrent pseudonyms; any measurement of forward-to-issuer versus
revocation-request cooperation with heterogeneous MAs and provider shopping; the cross-MA framing
channel (forwarded reports bypassing the victim MA's reporter-reputation gate).
#### Engine evidence, and why it cannot come from elsewhere
- Per-event honest-witness distributions *by evidence class*: who decoded the attacker (radio,
  geometry, clustering on arterials) and whose sensors covered it (class 4 perception), with RSU
  detector hosts at real signalised intersections. Pilot logs give neighbour counts only, never
  attack events, perception coverage or evidence class.
- Report survival through cellular, RSU relay and store-and-forward, and the provider mix of the
  witnesses when fleets cluster by route (a closed form assumes binomial thinning; the paper must
  show where the engine departs from it, against an explicit Poisson/binomial baseline).
- Ground truth for soundness (was the revoked device honest?) and the privacy cost of reporter
  de-duplication measured on true trajectories (how much of an honest reporter's week the LA
  answers link). MA policy, cooperation and LA options are backend-only, so they replay exactly.
#### Experiment plan
1. Validation first: neighbour counts against THEA/Wyoming BSM logs; PDR versus distance per RAT
   against field data; fault injection (Sybil reporter without de-dup must frame; zero-witness
   geometry must never act; attacker-free runs must give zero revocations under a sound policy).
2. Witness maps: 10 maps stratified by signal density x 4 penetration levels (from a cited fleet
   turnover x OEM adoption trajectory, plus 1-50% sweep) x 2 RSU placements (25% of signals by
   volume vs random). ~80 full runs (~190 h; low-penetration runs are cheaper); RATs on 2 maps.
3. Strategic attacker who knows RSU sites and picks the least-witnessed links and hours; report
   worst-case escaped exposure, harm-weighted (equipped receivers reached x time until the
   decision and CRL reach them).
4. Offline MA replay of every run over a policy family (k-threshold, reputation, Bayesian,
   LA-linked accumulation across pseudonyms and days, proof-only) x reporter de-dup on/off x
   cooperation (none, forward-to-issuer, revocation request) x shares 50/50, 80/20, 95/5.
5. Day-scale time to act from the shared multi-day surrogate, labelled analytical where it is.
#### Engine extensions (about 3-4 weeks, ~1.7-2.7 kLOC)
Per-event witness metric 150-250;
MaReportRecord fields and offline replay binary 100-200; multi-pseudonym Sybil reporter plus LA
de-dup option and reporter-privacy metric 200-400; MA policy family 300-500; per-provider MA
settings 150-300 (today cloned, phase2.rs:2174); cooperation modes incl. revocation request
200-300; attacker provider pinning and route-clustered provider assignment 100-200; RSU-aware
strategic attacker 200-300. Multi-day surrogate shared with S1.
#### Venue
VehicleSec 2027 (inferred late Feb 2027) only if S1 is not also aiming for it;
otherwise VehicleSec 2028 (inferred Feb 2028). Alternatives: IEEE T-ITS or TDSC (rolling), or
PoPETs if the reporter-privacy arm leads (several deadlines a year; dates not checked).
#### Review score
N3 major revision (reject at VehicleSec as pitched), C reject (major if
reframed), 4/5 each; both named this reframing as their path to accept. Merged design unreviewed.
#### Remaining risks
(1) A possibly obvious headline ("de-duplicate reporters"); it must give a
non-obvious boundary, e.g. de-dup cheap for persistent attackers, unaffordable for intermittent. (2) MA algorithms are proprietary, so the output is a design
argument plus requirement text, not a parameter adopted directly. (3) 2028 vehicle penetration is
near zero, so early stages reduce to RSU coverage; say so. (4) All city worlds are 1.5 km downtown
boxes; no claims outside urban cores without new corridor worlds. (5) Penetration changes are not
exactly paired (radio differs); claim common traffic only.

## Killed finalists

- **N4 (compromised intersection).** MBRs are encrypted to the MA (no subject filtering); RSUs
  die by non-renewal of 169 h certificates and operator action, not CRLs; the headline is true by
  construction. The reframing (SPaT detector, compromised RSU vs faulty controller) is novel, but
  the engine cannot show harm (warnings do not change driving). Park until a driver model exists.
- **N3 standalone.** Trade-off is prior art (Bißmeyer 2012, Moore 2008, Intel patent); adopter
  vague; framing is driven by reporter de-dup, not penetration. Kernel in S2.
- **C standalone.** Report sharing is already a SHALL; the evidence split is binomial thinning;
  delay is additive; the CTL arm is roughly share x stale fraction with no deployment data for
  the stale fraction. Cooperation-mode kernel and cross-MA framing channel in S2.

## Ranked recommendation

1. **S1 first.** Concrete deployed rule, a real-data anchor, a named parameter owner, and engine
   results that logs cannot give. Feasible for late-Feb VehicleSec if calibration starts now.
2. **S2 as the thesis anchor**, built on S1's MA model and surrogate; it uses the closed loop
   (witnesses → reports → MA → LA → CRL) most fully and continues the owner-liked framing work.
3. First, obtain the J3287 and TS 103 759 texts and the SCMS Manager MA drafts.
