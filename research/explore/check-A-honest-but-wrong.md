# Check A: "Honest but wrong" (benign GNSS error and defective device classes vs the MA)

Blind prior-art and feasibility check, 2026-10-18. Scope: the direction as pitched (merging N1 and
benign-mbr-load). Sources are cited inline: catalog ids (research/catalog.jsonl), DOIs, patent
numbers, URLs with page or section, and engine file:line.

## Verdict

**Partly done. The core question is still open, but it is narrower than the pitch claims, and two
of the pitch's premises are wrong as stated.**

| Piece of the pitch | Status | Closest prior work |
|---|---|---|
| Faulty vs malicious as distinct MA outcomes | **Done (per pseudonym)** | Kamel PhD thesis 2020 (kamel2020detection), §6.1 p.114: the MA classifies a reported station as "Misbehaving … Faulty … Genuine (false positive)", a multi-class ML problem, with reports "sorted automatically by reported pseudonym" (p.113). Faults are VeReMi Extension's per-vehicle synthetic faults (kamel2020veremi, DOI 10.1109/icc40277.2020.9149132; thesis p.54 list) |
| Faulty nodes evicted the same way as attackers | Done (old) | Raya et al. 2007, DOI 10.1109/jsac.2007.071006 (raya2007eviction) |
| Remediation must depend on cause (GPS malfunction / design error in a series / attack) | **Stated, not measured** | 5GAA "MBD for V2X: Operational Aspects" v1.0 (2024), https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf, p.13 (the three categories, verified), p.12 ("affects a particular vehicle manufacturer, year or model"), p.14 ("A defect in the series means the manufacturer must find the root cause"), p.15 ("misbehaviour needs to be classified to justify the costs of updates") |
| Honest GNSS degradation produces revocations under a counting MA | **Shown in this engine's own code comments**, not published | crates/v2xw-threat/src/ma_corroborate.rs:9-14: under the legacy MA, the `revocation-latency` scenario with attackers removed "revoked 21 honest vehicles in 300 s at 6,000 veh/h" |
| Sender flags its own faulty GNSS or sensor | **Patented** | Qualcomm US 12470954 B2 (priority 2022-03-16): a transmit-side detector "may detect the misbehavior condition in the local GNSS receiver" and the vehicle "may also send a report … to a management entity" (https://patents.google.com/patent/US12470954B2/en). US 11613264 B2 "Transmit-side misbehavior condition management": the device withholds the bad field (sends "unavailable") and may report the faulty element "to … a central authority" (https://patents.google.com/patent/US11613264B2/en) |
| MA or OEM learns the device's OEM despite pseudonyms | **Patented** | US 11082846 B2 (Qualcomm, per Google Patents): the "Misbehavior Report Catcher might be unable to independently identify the misbehaving [OBE's] OEM from the Pseudonym Certificate", so a provisioning-backend router looks up the device identity and forwards it to the OEM (https://patents.google.com/patent/US11082846B2/en) |
| GNSS-quality field in the evidence | **Already exists** | Every J2735 BSM carries `PositionalAccuracy` in coreData (the engine models it: crates/v2xw-core/src/belief.rs:36; crates/v2xw-msg/tests/j2735_psm_oracle.rs:87), and the signed BSMs are the MBR evidence. ETSI CAM has `PositionConfidenceEllipse`. The CV-pilot rule ignores it; confidence-aware checks are standard in F2MD (kamel2020detection ch.4) |
| Honest-data anomaly rates on real V2X traces | Partly done (local detector only) | VCADS, Salazar, McDaniel, Sheatsley, Petit 2022, DOI 10.4271/12-05-03-0020 (salazar2022physics): physics checks run on "four real V2X field testing datasets" |
| Cross-pseudonym aggregation at a central MA | Partly done (per suspect only) | Bißmeyer et al. 2012, DOI 10.1145/2307888.2307902 (bimeyer2012central): the central evaluator may ask whether different pseudonyms in related reports belong to the same node |

**What I did not find anywhere** (corpus keyword scan of 11,315 records; OpenAlex, arXiv and
Semantic Scholar queries, the last one rate-limited; WebSearch including patents; ETSI TS 103 759
V2.2.1):
1. A measurement of how many reports an **honest fleet** generates under the **actual US CV-pilot
   MBR rule set** (SCMS Manager MBR v1.0) with realistic receiver output.
2. Any study of the spec's **on-device store** (0.9/day weight decay, quota eviction,
   connectivity-gated upload) and whether benign reports **crowd out** attack evidence.
3. **Correlated benign causes** at the MA: a spatial cluster (one canyon corridor) or a
   **device class** (one firmware or model defect) that spans many vehicles. All MA work found
   classifies one pseudonym or one suspect at a time, with independent per-vehicle faults.
4. **Pricing** the disclosure that would let an MA attribute a series defect, in bytes and in
   anonymity-set or linkability loss. The patents above propose the plumbing but measure nothing.

**What remains open, stated exactly:** cross-vehicle cause attribution (canyon cluster vs device
class vs attack) by a US-SCMS MA under the privacy split, plus the cost of the minimal disclosure
that enables it. The honest-load and store crowd-out measurements are open but small, and they can
be partly done on real pilot data without the engine (see Risk 2).

## Premise checks

- **"Triggers a position report at 1 m GNSS drift": verified, but misread.** SCMS Manager MBR v1.0
  §2.3.1.2.2 (PDF p.6, https://scmsmanager.org/wp-content/uploads/2020/01/Misbehavior-Report-and-Application-Specification-v1.0.pdf):
  `gpsDrift` default 1 m. "Two consecutive BSMs are inconsistent if [(position in the first) +
  (velocity in the first)*(time between)] differs from (position in the second) by more than this
  parameter." The rule compares **fix to fix**, so what matters is the jump between consecutive
  positions, not the ~3 m absolute error. Production receivers smooth their output (filtering or
  INS), so open-sky fix-to-fix jumps are small. A canyon reports mainly through discrete multipath
  or satellite-set jumps, and nobody has measured their rate in BSM streams. A second benign path
  the pitch misses: "consecutive" received BSMs lose the acceleration term over a gap, and with
  packet loss that error grows as ½·a·Δt². At 3 m/s² braking, a ~0.8 s reception gap alone
  exceeds 1 m. That is a radio-determined honest trigger, and the engine is well placed to measure
  it. Store and upload rules (spec §2.2, PDF p.4-5): weight 1.0, ×0.9 daily at 05:00 Eastern,
  upload highest weight first, evict lowest weight first when over quota. The quota is
  "device-specific", so it is not set by the spec.
- **"About 3 m open sky"**: the engine's own calibration uses Reid 2019 production-automotive
  rows of 3.07/5.30/9.38 m horizontal (68/95/99 %), with deep-canyon mean 31.02 m and
  canyon-mitigated 9.57 m from Wen and Hsu (crates/v2xw-mobility/src/gnss/gauss_markov.rs:21-25;
  common.rs:61-73). UrbanNav (Hsu et al., NAVIGATION 70(4), DOI 10.33012/navi.602) is the
  open canyon dataset. I could not confirm the J2945/1 accuracy requirement text (paywalled).
- **"Every MA design judges one pseudonym at a time"**: this is too strong. CAMP/SCMS linkage
  (Brecht et al. 2018, brecht2018security) and Bißmeyer 2012 aggregate pseudonyms of one
  suspect. The defensible claim is: **no MA design aggregates across vehicles by shared cause.**
- **"Cannot see device class or GNSS conditions"**: half wrong. The evidence BSMs carry the
  sender's self-reported `PositionalAccuracy`. Device class is genuinely hidden from the
  pseudonym certificate (US 11082846 says so), though the issuing PCA or provider may correlate
  with OEM in a multi-provider deployment. So the "one added field" is really a **device-class
  or defect-cohort token**, not a GNSS-quality field.

## Engine feasibility

**Exists** (paths under crates/):
- US MBR format, COER, 1609.2 signing, encryption to the MA, the RA upload header, MA opening and
  evidence verification: v2xw-proto/src/scms/mbr.rs (951 lines; `encode_report` :483,
  `ma_open_report` :694, `verify_evidence` :764). Investigation from the accused linkage value:
  v2xw-proto/src/scms/run/report.rs:16-152.
- The spec's store, wired into the engine: `MbrQueue` (mbr.rs:820-951: decay, quota eviction,
  1-week expiry) used as `ReportStore` in v2xw-engine/src/phase2.rs:1024-1040. The quota is an
  **assumption** of 64 KiB, about 58 reports (phase2.rs:1031-1034).
- Upload over cellular or RSU relay, with counters (v2xw-engine/src/backend.rs:169-176;
  phase2.rs:3603-3607).
- Detectors (v2xw-threat/src/detect.rs, 15 `DetectorId`s :112-142). MA pipelines are a
  report-window counter (v2xw-threat/src/ma.rs, k = 3 reporters / 4 s / 15 s window) and an
  event-corroboration MA (ma_corroborate.rs). ETSI TS 103 759 reporting is in
  v2xw-threat/src/ts103759.rs.
- GNSS model with sky-view classes, outages, bursts and a faulty-sensor bias ×5
  (v2xw-mobility/src/gnss/gauss_markov.rs; views.rs:774-823). Buildings carry `height_m` and
  `levels` (v2xw-world/src/model.rs:1927-1960).
- Ground truth: actor-labelled attack channel kept apart from detection channels
  (v2xw-threat/src/records.rs:15, :122-186).

**Must be built** (rough size):
1. **The spec-exact MBR check set**, about 150-250 LOC. The engine's `PositionJump` is a
   confidence-aware residual, Δs / (v̄·Δt + Z·max(conf, 0.5c) + c) (detect.rs:1046), that is only
   *labelled* RandomPosition with gpsDrift 1000 mm (phase2.rs:6636-6648). MaxSpeed,
   BrakeAcceleration and ObsDistance have no mapping (`us_subtype` returns None).
2. **Location-dependent sky view**, about 300-500 LOC plus calibration. The engine always passes
   `GnssEnv { weather, ..GnssEnv::OPEN_SKY }` (v2xw-engine/src/run.rs:3699-3702), so canyons by
   location **do not exist today**. Building street-canyon aspect ratios from building heights is
   feasible.
3. **A filtered-receiver output model**, about 200-400 LOC plus validation (see Limit 1). Without
   it the study is invalid.
4. **Defect-class model**, about 200-400 LOC: per-vehicle device class, class-specific fault
   (frozen position, timestamp offset, heading or braking-flag bug) and activation conditions. The
   `faulty_sensor` flag exists but nothing sets it.
5. **Cause-aware MA variants plus disclosure options** (cohort token in the report, RA-assisted
   device-class lookup), with an anonymity-set and linkability metric, about 400-800 LOC.
6. **Multi-day surrogate** for the store and MA (see Limit 3), about 300 LOC offline.

Total: about 3-5 weeks, not 2-3.

**Engine limits that undermine it unless fixed:**
1. **The current GNSS model would manufacture the headline (a check that cannot fail).**
   White noise of σ = 1.6 m per axis is drawn independently for every estimate at 10 Hz
   (gauss_markov.rs:96, :239, :540-553; "Drawn per estimate at 10 Hz" :953). That σ is the
   MathWorks gpsSensor default at a 1 Hz rate (:29-34, :106). The fix-to-fix jump then has
   σ ≈ 2.26 m per axis, so P(jump > 1 m) = exp(−1/(2·2.26²)) ≈ **0.91 for every honest BSM
   pair in open sky**. Under the 1 m rule, every vehicle would "misbehave" constantly. The fix:
   calibrate fix-to-fix statistics against real 10 Hz BSM streams before any result. The THEA BSM
   sample is public (https://catalog.data.gov/dataset/tampa-cv-pilot-basic-safety-message-bsm-sample;
   full raw set via ITS DataHub sandbox). Then inject a known canyon jump to prove the check can
   go red **and** stay green in open sky.
2. **Real data partly pre-empts the engine.** Running the six MBR v1.0 checks over THEA or
   Wyoming BSMs directly measures the honest report rate, and VCADS already ran physics checks on
   four real datasets. The engine's unique share is the **cause labels**, the **canyon and
   device-class counterfactuals**, the **radio-gap trigger**, and the
   **store→upload→MA→LA chain**. The paper must lead with those.
3. **Timescale mismatch.** The store decays daily, unsent reports expire after a week, and
   revocation takes days (≤ 24 h CRL publication in the draft NA CP; see
   docs/design/scms-real/03-protocols-and-operations.md §8 rows citing [5GAA]). The engine runs
   about 2.4 h per 600 s at ~450 vehicles (THOUGHTS.md), so multi-day city fleets are out of
   reach. The defensible design: closed-loop runs produce per-corridor, per-class report-generation
   rates, which feed an open-loop multi-day replay of the store and MA. THOUGHTS.md allows exact
   counterfactuals for open-loop and backend-only changes, but reviewers will see the multi-day
   part as a Monte-Carlo.
4. **MA strawman risk.** Both engine MAs count reports or events. Earlier rounds found that
   standard MAs are evidence-based (THOUGHTS.md, "Strawman assumptions die"). The US MA is
   unspecified, so the study must sweep MA policies, including linkage-assisted per-device
   aggregation, and not claim "the MA" fails.
5. **Crowd-out depends on an assumed quota** (64 KiB). It needs a sweep and an honest statement
   that the spec leaves the quota device-specific.

## Bottom line for the thought log

- **Novel kernel:** cross-vehicle cause attribution (canyon cluster vs device-class defect vs
  attack) at a US-SCMS MA under the privacy split, priced in bytes and anonymity-set loss, with
  the honest MBR load and store crowd-out as the measured motivation. Adopters are real: 5GAA
  (p.12-15 asks for exactly this classification), the SCMS Manager MBR parameters (1 m gpsDrift,
  quota) and OEM recall decisions.
- **Not novel:** fault-vs-attack classification per pseudonym (Kamel 2020), sender self-flagging
  (Qualcomm US 12470954, US 11613264), OEM lookup for report routing (US 11082846), and the
  existence of a GNSS-quality field (J2735 `PositionalAccuracy`).
- **Must fix before any number:** the 10 Hz white-noise GNSS artefact (P ≈ 0.91 per honest pair),
  the missing location-dependent sky view, and the spec-exact check set. Budget 3-5 weeks.
- **Main reviewer attack to pre-empt:** "Run the six checks on THEA BSMs, so why simulate?" The
  answer has to be the cause labels, the device-class counterfactuals and the full chain. Real
  data should be used for calibration, not avoided.
