# Check 3: public CV-pilot BSM data for Paper 1, and the VESNOS paper

Written 2026-10-23. Role: critical reviewer. Read-only on engine code; nothing built or run in the engine.
Scripts: `research/synthesis/check-realdata/` (pure Python, standard library; run with `python3 -I <script> <data_dir>`).
Data: a 15 MB sample, kept outside the repository in the session scratchpad (`ref/cvpilot`). Section 1.4 lists the queries that rebuild it.

---

## 0. Verdict

**(a) Real data: usable now, and the result is better and more dangerous for Paper 1 than expected.**
- Public BSM streams exist with per-ID consecutive messages at 100 ms. Gaps of 90–110 ms make up 70% of Tampa gaps, 53% of Wyoming's, 46% of Orem's and 96% of NYC's. Gaps of 0.5 s or less make up 75–100%.
- The SM-MBR RandomPosition check (`|p₁ + v₁Δt − p₂| > 1 m`) can be run on them today, with no data request.
- **Honest vehicles trigger it in real data, at very different rates by device.** At v ≥ 10 m/s:
  - 0 of 4,946 pairs in Wyoming;
  - 4.5% of pairs (20 triggers per ID-minute) for 142 Tampa IDs;
  - 44% of pairs (195 per ID-minute) in Orem, Utah.
- **The mechanism is not the one the engine models.**
  - 73–96% of triggers have a displacement that is ≈0 or ≈2× v·Δt: the position belongs to a different epoch than the timestamp.
  - Errors are along-track. The cross-track component dominates in only 5 of 361 Tampa triggers, 4 of 3,246 Orem triggers and 1 of 717 NYC triggers.
  - Triggers occur only on devices whose secMark is *not* locked to GNSS epochs. The 48 epoch-locked Tampa IDs gave 1 trigger in 2,803 fast pairs.
  - The engine's raw-GNSS case (white noise, the same size along- and cross-track; `testing/usfp.md` §1) is the wrong mechanism. Its smoothed receiver reproduces only the good device class.
- **The time base decides the result.** With Wyoming's header time (`metadata_generatedat`, documented as "taken from the communication header") in place of secMark, the same 4,939 pairs go from 0 to 1,814 over 1 m (37%).
- **Consequences.**
  - Paper 1's headline survives and becomes real-data-first.
  - The engine's honest-trigger model must be recalibrated: an epoch-misalignment device class plus a header-clock choice, not GNSS noise.
  - The "why simulate?" attack (check-A §2) is now concrete, because the benign trigger rate comes from data alone.

**(b) VESNOS full text: still not obtained.**
- Tried: ACM PDF, ePDF, fullHtml and `?download=true` (403, Cloudflare); Unpaywall and OpenAlex (publisher copy only, CC-BY, `has_repository_copy: false`); the Semantic Scholar PDF cache (202, empty).
- Also tried: the eScholarship and NSF PAR searches (no VESNOS item; PAR lists 4 other Abdo papers); arXiv (no copy); the Wayback Machine and scholar.archive.org (offline or behind a captcha); ResearchGate (403).
- One new thing was learnt, from search-engine excerpts only (unverified): **the paper claims to model "bootstrapping, certificate provisioning, misbehavior reporting, and CRL retrieval", and to measure "detection performance against the man-in-the-middle attack".** The released code has none of the first four flows (`vesnos-full-read.md` §3.7). See §5.

---

## 1. What exists, and how it is accessed (a)

All on data.transportation.gov (Socrata; SODA API `https://data.transportation.gov/resource/<id>.json|csv`, `$select/$where/$order/$limit`; metadata `/api/views/<id>.json`). Row counts were measured with `$select=count(*)` on 2026-10-23.

### 1.1 Dataset table

| Dataset (id) | What it is | Rows | 10 Hz per-ID sequences? | Accuracy field | Licence |
|---|---|---|---|---|---|
| Tampa CV Pilot BSM Sample (`nm7w-nvbm`) | RSU-received BSMs (`metadata_recordgeneratedby = rsu`, `kind = in`), raw J2735 units, one night 2021-03-19 00:17–06:59 UTC, 226 temp IDs | 34,464 | Yes, with RSU gaps: dt p50 104 ms, p90 305 ms | semiMajor/semiMinor/orientation (0.05 m units; p50 0.30 m) | CC BY-SA 4.0 |
| Wyoming CV Pilot BSM One Day Sample (`7qu7-j8rx`) | OBU-logged own BSMs (`bsmLogDuringEvent`, 6,506; `bsmTx`, 91), `metadata_sanitized = true`, 2022-06-30, 16 IDs, I-80 only | 6,597 | Yes in bursts: dt p50 100 ms, but 25% of gaps over 0.5 s | metres (0.45–0.6) | CC BY-SA 4.0 |
| NYC CV Pilot EVENT One Month Sample (`vxmm-9bi5`) | One record per ASD warning; host and remote BSMs over a ±~10 s window; obfuscated | 10,533 events | Yes, within an event: 96% at 100 ms | semiMajor (0.05 m) | NYC.gov Terms of Use + CC BY 4.0 (attachment `TermsOfUse.txt`) |
| NYC CV Pilot IE Processed Data (`gu5j-r7xj`) | Volpe-processed event summaries | n/a | No BSMs | n/a | Terms of Use |
| UDOT/Panasonic BSM, Orem UT (`ev64-jn8c`) | Deduplicated RSU-received J2735 BSMs, 7 days, 2025-02-02..08, 3 intersections, 2,914 temp IDs | 4,137,684 | Yes: dt p50 112 ms | semiMajor/minor/orientation (p50 2.0 m) | none stated |
| Utah Pooled Fund BSM Trajectories (`e9cn-jdnu`) | "timestamped BSMs … at 10 Hz … anonymized", Mar–Jun 2025 | 157,806,337 | Timestamps yes, but see §2.5 | none; no secMark or msgCnt | none stated |
| SPMD (`deww-aa3r`) | Ann Arbor 2012–13 BSMs, blob download | not inspected | (known 10 Hz) | n/a | none stated |
| LTE-V2X Wyoming test runs (`2pc4-cx48` etc.) | KML-like name/description/coordinates rows | 5,084 | not BSM records | no | none |

### 1.2 Full raw data

- **The public S3 sandbox is no longer listable.** `usdot-its-cvpilot-publicdata.s3.amazonaws.com` returned **AccessDenied** on 2026-10-23, although the Wyoming dataset page still links it.
- **Full raw Tampa and NYC data go through a form** at its.dot.gov/data/data-request. The form says "There are no restrictions on who can use this form", gives no turnaround and names no DUA.
- **Raw sizes could not be verified,** because the listing is denied. Do not quote a size.

### 1.3 Sanitisation that matters for consecutive-BSM checks

- **NYC.**
  - Lat/long are replaced by X_m/Y_m, translated (not rotated) to the warning BSM, and secMark by T_s (`DataProcessing.txt`). That is harmless for the residual.
  - But records exist only around warnings: all 12 sampled are FCW, 7 of them on Manhattan freeways. That is a biased sample.
  - The coarse time and location bins hide where and when, so no corridor-level analysis is possible.
- **Wyoming.**
  - Every sampled BSM is at 15.8–35.8 m/s. There is no stopped traffic, so ConstantPosition cannot be tested, whether because of sanitisation or because the road is I-80.
  - 25% of gaps exceed 0.5 s (event logging).
  - Temp IDs live for exactly about 219 s. This breaks one pair per change, which is harmless.
- **Tampa.** The data are an RSU-received sample, so reception gaps are mixed with the sender's behaviour. msgCnt is +1 for 78% of kept pairs.
- **Utah pooled.** No secMark or msgCnt, and there are duplicate rows: 19,558 rows hold 16,988 unique (ID, time) pairs.
- **No dataset truncates positions.** Wyoming has 7 decimals, and J2735 units are 1e-7°.

### 1.4 Sample used

| File | Query |
|---|---|
| `tampa.csv` | `nm7w-nvbm`, all rows, 16 columns |
| `wyoming.csv` | `7qu7-j8rx`, all rows, 16 columns |
| `utah_orem.csv` | `ev64-jn8c`, `tmstp_utc between '2025-02-04T15:00' and '15:30'`, 15,793 rows |
| `utah_pooled.csv` | `e9cn-jdnu`, 6 IDs, 2025-04-15 15:00–15:20, 19,558 rows |
| `nyc_events.json` | `vxmm-9bi5`, first 12 events |

---

## 2. Feasibility result: SM-MBR-style checks on real BSMs

### 2.1 Method

Method (`mbr_residual.py`):
- **Pairs.** Consecutive BSMs of one temp ID, de-duplicated on (ID, secMark, msgCnt). Δt comes from secMark (generation time in the BSM), disambiguated by wall time. A pair is judged if Δt ≤ 0.5 s (the engine's `max_gap_s`).
- **Residual.** Heading is clockwise from north, in a local ENU frame.
- **Other checks.** ConstantPosition (v ≥ 0.02 m/s on both BSMs, moved < 1 cm) and MaxAcceleration (|Δv|/Δt > 10).

### 2.2 Results

| Dataset | Pairs judged | p50 / p99 residual | > 1 m | ConstantPosition | IDs with ≥ 1 trigger |
|---|---|---|---|---|---|
| Wyoming (OBU log) | 4,946 | 0.017 / 0.175 m | 0 | 0 | 0/12 |
| Tampa (RSU rx) | 32,291 | 0.019 / 1.12 m | 361 (1.1%) | 447 | 99/216 |
| Orem (RSU rx) | 15,188 | 0.160 / 2.84 m | 3,246 (21%) | 1,554 | 16/17 |
| NYC (event, host + remote) | 2,172 | 0.040 / 17.4 m | 717 (33%) | 102 | 10/22 |
| Utah pooled | 16,683 | 0.002 / 3.20 m | 2,495 (15%) | 1,194 | 6/6, **unreliable** (§2.5) |

### 2.3 Why the rule fires, from `diag.py`, `diag2.py` and `diag5.py`

- **The vehicles are honest and the positions are right.** The kinematics agree: |Δp|/(v·Δt) has p50 0.98–1.00 everywhere.
- **The trapezoid residual is identical to the SM-MBR residual.** It uses the mean speed and heading of both BSMs (Tampa 353 vs 361 over 1 m; Orem 3,243 vs 3,246). So turning and braking explain nothing at 100 ms.
- **Triggers are epoch slips.** Two examples:
  - Tampa: v = 15.16 m/s, Δt = 102 ms, |Δp| = 3.04 m, which is two fixes' worth.
  - NYC: alternating 0.01 m and 2.2 m steps at 10–12 m/s, a fix every other BSM.
- **Orem positions repeat while moving.** In 13% of moving pairs the position is frozen while v ≥ 5. Each one is a ConstantPosition report, and the next pair is a RandomPosition report.
- **Device class predicts it.** IDs whose secMark sits within 1 ms of a 50 ms epoch on more than 80% of BSMs are "locked"; IDs under 30% are "free". At v ≥ 10 m/s:

| Device class | IDs | Fast pairs | > 1 m | Triggers per ID-minute |
|---|---|---|---|---|
| Tampa locked | 48 | 2,803 | 1 (0.04%) | 0.2 |
| Wyoming locked | 12 | 4,946 | 0 | 0 |
| Tampa free | 142 | 7,644 | 344 (4.5%) | 20.3 |
| Orem free | 14 | 6,485 | 2,856 (44%) | 195 |

### 2.4 The time base (`diag4.py`)

- **Wyoming's secMark is epoch-aligned.** Its modes are xx00/xx50 ±1 ms.
- **The header time jitters against it.** Per pair, header Δt minus secMark Δt is p10 −46 ms and p90 +53 ms.
- **At 35 m/s that is ±1.8 m.** Using header time as Δt, 1,814 of 4,939 pairs exceed 1 m.
- **The engine reads SM-MBR as using 1609.2 generationTime** (`us_mbr.rs` docs; `usfp.md` §1). If deployed OBUs do the same, even good devices fire constantly at highway speed.
- **One caveat.** Whether `metadata_generatedat` is the signed generationTime or the ODE's log time needs the raw `.bin` frames (the logfile names are `.bin`, so they exist).

### 2.5 Not trusted

- **Utah pooled.** Speed is 0 while the position moves 1.7–5 m per 100 ms (34–50 m/s implied; e.g. ID `4bc9c637`). Some IDs look sequential (`00004290`), and there are duplicates. Do not use it for residuals.
- **NYC.** Its large residuals at > 15 m/s (p50 8.5 m) come from frozen runs of up to 8 BSMs on freeway FCW events. That is real, but the sample is selected around warnings.

### 2.6 Limits

- **Units are temp-ID periods, not vehicles.** "Per ID-minute" is per observed ID-minute.
- **The samples are tiny.** Tampa is one night, Orem 30 minutes, Wyoming 16 IDs and NYC 12 events.
- **Pilot OBUs predate production.** J2945/1 may require secMark to equal the position time; that is unverified (paywalled). If it does, the free class are non-conformant devices, which is exactly S1's "defect class".
- **Prior work on this mechanism is not checked.** One search found nothing on secMark/fix-epoch misalignment as an MBD false-positive cause. That is not a novelty check.

---

## 3. What it changes for Paper 1 ("Honest but wrong")

1. **Lead with real data. It now carries the headline.**
   - Under the deployed 1 m rule, honest pilot devices of one class produce 20–195 RandomPosition triggers per ID-minute at speed; another class produces about 0.
   - That is stronger than anything the engine has shown, and it needs no simulation.
2. **The engine's "26 honest revoked with raw GNSS vs 0 with Doppler smoothing" is not evidence about real fleets.**
   - Its raw case uses the wrong error shape: isotropic white noise, where real triggers are along-track epoch slips.
   - Its smoothed case equals only the locked class.
   - Claiming "smoothing fixes it" would be contradicted by the Orem and Tampa-free data. Remove that framing.
3. **Recalibrate before any engine result.** Add a per-device timestamp model with these parameters, fitted to Tampa free and locked and to Orem, with Wyoming held out:
   - P(position epoch ≠ timestamp epoch);
   - the duplicate/skip pattern;
   - the header-clock jitter.

   Then inject it to prove the check goes red (memory: checks that cannot fail).
4. **The attribution story gets cheaper.** The MA receives the evidence BSMs with secMark, so device class is observable without a new cohort token, by the secMark phase. Two consequences:
   - S1's "only new field is a cohort token" premise (final.md S1) weakens.
   - Evasion: an attacker can fake the phase. Analyse it.
5. **Decide the time base explicitly.** Report results under both secMark and 1609.2 generationTime. The choice flips Wyoming from 0% to 37%.
6. **What remains engine-only:** receiver-side gaps at density, the store/quota crowd-out, report → MA → LA, and the counterfactual cause labels. The paper must say so (check-A §2).

## 4. What it changes for Paper 2 (benchmark)

- **Real streams supply priors, not labels.** They give device-class timestamp behaviour and RSU reception-gap distributions (Tampa: 70% of gaps at 100 ms, 95% ≤ 0.5 s). They cannot supply same-seed counterfactual or cross-layer labels, so the benchmark's label claim stands.
- **The benchmark should ship a real-data calibration split.** Use Tampa/Orem → engine fit, with Wyoming held out, and report the honest-trigger rate per device class. Otherwise reviewers will say its benign class is synthetic white noise.
- **Licences allow redistribution of derived statistics** (CC BY-SA 4.0 for Tampa and Wyoming; CC BY 4.0 plus NYC terms for NYC). Share-alike applies if raw rows are redistributed. The Utah sets state no licence; ask ITS JPO before redistributing their rows.

## 5. VESNOS: what changes

- **Basis unchanged.** It is still code plus manual (`vesnos-full-read.md`), and the summary is not changed.
- **Excerpt claims (unverified).** The paper claims SCMS "bootstrapping, certificate provisioning, misbehavior reporting, and CRL retrieval", plus a MITM detection measurement. Commit `997913b5` implements none of the four flows (§3.7 there).
- **Paper consequence:** the engine cannot claim to be the "first simulator with misbehaviour reporting in the loop" against VESNOS's text. Only the narrowed claim holds: report → RA → MA → LA linkage → revocation, verified in code. Cite the paper alongside the commit.
- **Owed, unchanged:** the full text through a library or interlibrary loan, or from the authors.
