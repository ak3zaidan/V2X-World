# Check N3: the bootstrap paradox (MA evidence thresholds along the USDOT deployment trajectory)

Blind prior-art and feasibility check, 2026-10-18. I read nothing under `research/synthesis/` or `research/explore/` except `explore/THOUGHTS.md` (engine limits).

## Verdict

**Partly done; the specific question is open.** Nobody I found has swept OBU penetration and RSU density against a central MA's corroboration threshold, or measured the MA's time to act and its framing exposure along a deployment trajectory. The ingredients are all published separately:

- witness-count thresholds at a central MA (Bißmeyer 2012);
- report-count thresholds at the MA (F2MD 2020);
- threshold event validation by witnesses (Hsiao 2011, WEVAN 2013);
- penetration sweeps of *local* CPM detection (Zhang 2024);
- Sybil defence with RSUs in early deployment (Park 2013);
- a patent saying a local MA's area should grow when penetration is low and that corroboration needs "enough independent observers" (Intel US12489779B2).

So the novelty is the joint, quantitative trajectory study, not the concept. Reviewers will cite Bißmeyer 2012 and the Intel patent first.

**Scope correction that the pitch must absorb.** Under a proof-based MA, witness thresholds matter only for some of the TS 103 759 evidence classes:

- **Classes 1 and 2 need no witnesses.** These are implausible values and self-inconsistency. The suspect's own signed messages prove them, and an attacker cannot forge an honest vehicle's signature, so framing is impossible. One report is enough. The engine already implements this check: `v2xw-proto/src/scms/mbr.rs:764 verify_evidence`.
- **Classes 4 and 5 need witnesses.** These are the sensor cross-check and conflicts with other stations. The MA cannot verify them from signatures alone, so they need independent witnesses.

The classes are listed in the 5GAA MBD-Op white paper (2024, §6.2, p.20). ETSI TR 103 460 V2.1.1 (p.29) even lists "CAMs of neighbours" as an evidence level. The paradox is therefore real only for kinematically self-consistent attacks: ghost vehicles, Sybil and along-road offsets. The paper must say so, or a reviewer will say "just use self-incriminating evidence".

**A sharper point the pitch misses: Sybil reporters.** A US SCMS vehicle holds 20 pseudonym certificates that are valid concurrently each week (Brecht et al. 2018, arXiv 1802.05323). One attacker can therefore sign reports as up to 20 "independent" witnesses unless the MA de-pseudonymises reporters through the linkage authorities. That costs privacy. Today's engine budgets reports per pseudonym digest (`v2xw-threat/src/poison.rs:78-82`).

This turns the second half of the paradox into a three-way trade-off between the evidence threshold, reporter-linkage privacy and penetration. I found no published treatment of it. It is the most novel part of the direction.

## (1) Prior art, closest first

| Work | What it does | What it leaves open | Source |
|---|---|---|---|
| Bißmeyer, Njeukam, Petit, Bayarou, VANET 2012, "Central misbehavior evaluation…" | A central MA uses reputation from vehicle and RSU reports. At least 3 benign witnesses suffice to sink a ghost; colluders are studied. | Assumes a benign majority of reporters. No penetration or RSU-density sweep, no deployment stages, no SCMS pseudonym semantics. | doi 10.1145/2307888.2307902; catalog `bimeyer2012central` (abstract basis); thesis tuprints-00004257 (PDF blocked, not read) |
| Kamel et al., F2MD, IEEE TVT 2020 | MA with "a threshold on the number of reports for every reaction level" (p.8). Shows that reports per pseudonym collapse with frequent pseudonym change (Fig. 11, p.12). | Varies pseudonym-change period, not penetration or RSUs. Counting MA, not proof-based. | hal-02527873 (full text grepped); catalog `kamel2020simulation` |
| Haidar et al., DARE, VTC 2020-Spring | Report dataset for global MA evaluation. Varies *attacker* penetration rate on 2 EU maps (Luxembourg 104.5 veh/km², Paris-Saclay 17.1 veh/km², p.6). | Equipped-vehicle penetration is not varied, there is no RSU axis, and the threshold is only a toy example (p.7). | doi 10.1109/VTC2020-Spring48590.2020.9128687; hal-02491465 (full text grepped) |
| Kamel et al., UEMCON 2019 | ML at the MA to classify Sybil types from OBU and RSU reports. | No penetration or threshold trade-off. | doi 10.1109/UEMCON47517.2019.8993045; catalog `kamel2019misbehavior` |
| Zhang, Liu, Ben Jemaa, Bassi, VTC 2024-Fall; Zhang thesis 2024 | Local CPM misbehaviour detection plus an intersection-movement-assist app at **20% vs 80% penetration** (Table II, p.5). Thesis Fig. 5.10 shows collision alerts vs CPS penetration. Zhang IV 2024 notes that redundancy "is very much dependant on … a high penetration rate" (p.6). | Local detection only. No MA, no thresholds, one 1.24 km² map. | doi 10.1109/VTC2024-Fall63153.2024.10757840 (hal-04672473, grepped); catalog `zhang2024misbehavior`, `zhang2024simulation` (local PDFs grepped) |
| Park, Aslam, Turgut, Zou, SCN 2013 | Sybil defence "in the initial deployment stage" using RSU-issued timestamps and temporary certificates. | No MA, no SCMS, no threshold or penetration trade-off. This is the "closest work" the pitch names. | doi 10.1002/sec.679; catalog `park2013defense` (abstract) |
| Hsiao, Studer, Dubey, Shi, Perrig, WiSec 2011 (ES-TeV) | Threshold-based event validation: an event counts when the number of witnesses passes a threshold, with caps on alerts per sender. | Local validation of event alerts, not MA revocation, and no deployment stages. | https://www.csie.ntu.edu.tw/~hchsiao/pub/2011_ACM_wisec.pdf; cited in Kamel thesis p.37 (catalog `kamel2020detection`) |
| de Fuentes et al., WEVAN, J. Syst. Arch. 2013 | Witness testimonies as evidence. About 38 testimonies on highways when requested 5 s later; witnesses decline with request delay. | Varies delay and road type, not penetration. No MA policy. | doi 10.1016/j.sysarc.2013.07.009 (abstract via researchportal.uc3m.es/display/act413679) |
| Intel patent US12489779B2 (filed 2021-09-23, granted 2025-12-02) | Local MAs: "when V2X penetration rate is low, an LMA may cover a larger geographic area". Corroboration when "reported by enough independent observers". An "evidentiary threshold for a misbehavior type". | Concept only, no numbers. It anticipates the *idea* of stage-dependent MA policy, so claim the measurement, not the idea. | https://patents.google.com/patent/US12489779B2/en (first 100k characters read) |
| Raya et al., IEEE JSAC 2007 (LEAVE) | Neighbour voting to evict nodes when infrastructure is sparse. | Local voting, no SCMS MA, no deployment trajectory. | doi 10.1109/jsac.2007.071006; catalog `raya2007eviction` (abstract) |
| 5GAA WI MBD-Op white paper (2024) and 5GAA MBD white paper (2022) | Remediation conditions are "not yet specified" (MBD-Op p.4). Asks whether remediation should depend on the MA's confidence and how to avoid reporter free-riding (MBD 2022, p.27). | No quantitative answer. This is the field need the study would address. | https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf ; https://5gaa.org/content/uploads/2022/07/5GAA-Misbehaviour-detection-Final.pdf (both grepped) |
| arXiv 2607.07114 / 2607.05889 (2026) | Penetration vs *non-malicious* ghost objects in sensor and V2X fusion: higher penetration finds more objects but raises ghosts under GNSS error. | Not misbehaviour, not MA. | arXiv 2607.07114 (search snippet only) |
| CAMP VSC6 "V2V Misbehaviour Detection" V2V-CR final report | Event-based global MBD with linkage-value report counting and thresholds (described secondhand in patents US11695574 and US12192383). | **Not publicly retrievable, so this is a residual risk:** CAMP may have studied penetration internally. | patent citations only; not found on rosap.ntl.bts.gov |

The "Quantifying Ghost Object Detectability…" item (Doi, Perrone, Bassi, Rapelli; catalog `doindquantifying`) measures how often honest sensor vehicles can expose ghosts across 1,000 generated scenes per condition. It is the closest *witness-availability* measurement, but it has no MA and no thresholds. I could not locate its full text (summary only, abstract basis).

Searches with no closer hit:
- OpenAlex: "misbehavior detection penetration rate", "misbehavior authority reports threshold revocation", "sybil detection penetration rate roadside unit", "misbehavior detection sparse traffic", "ghost vehicle detection penetration rate".
- arXiv: misbehavior AND "penetration rate"; misbehaviour AND authority AND witnesses (both returned 0).
- WebSearch: 5 queries, one of them extended.
- Catalog: a regex over 11,315 entries for penetration or early deployment combined with misbehaviour, Sybil or trust (71 hits, all screened by title).

Semantic Scholar was rate-limited (429).

**What remains open, concretely:**
- (a) Measured witness counts per attack event for class 4/5 attacks as a joint function of OBU penetration, plan-level RSU density, map geometry and radio technology.
- (b) The MA's time to act and its missed-attacker rate against its false-revocation and framing rate, for each threshold at each deployment stage.
- (c) Whether trusted RSU reporters at signalised intersections substitute for missing vehicle witnesses.
- (d) The Sybil-reporter threshold trade-off under SCMS concurrent pseudonyms, and what reporter linkage costs in privacy.

The USDOT plan targets cited in the pitch are confirmed in secondary sources: 20% of NHS and 25% of major-metro signalised intersections by 2028, with phases 2024-28, 2029-31 and 2032-36 (fleetowner.com/technology/article/55134128; globalfleet.com). The "top 75 metros" wording is not in those sources; take it from the plan text (supplement https://rosap.ntl.bts.gov/view/dot/79091/dot_79091_DS1.pdf).

## (2) Feasibility in the engine

**Exists (no code change needed for a first pass):**

- **MA with tunable corroboration.** `crates/v2xw-threat/src/ma_corroborate.rs` (731 lines). It correlates reports into events and counts distinct trusted reporters per event and per window. All thresholds are parameters (`CorroborationParams`, lines 87-121): `event_min_reporters`, `min_events`, `window_s`, `min_reporters`. It also handles trusted-infrastructure reporters (`trust_infrastructure`, line 187). The legacy counting MA is in `ma.rs`.
- **Proof-style evidence check.** `v2xw-proto/src/scms/mbr.rs:734,764` (`verify_evidence` on the suspect's signed BSMs) and `v2xw-threat/src/resolve.rs:144` (`reverify_evidence`).
- **TS 103 759 classes 1-5**, with class 4 refusing to run where the sensor has no coverage: `v2xw-threat/src/ts103759.rs` (lines 13-39). Perception for class 4 and CPMs: `v2xw-engine/src/perception.rs`.
- **Attacks.** Sybil ghosts, AlongRoadOffset, SlowDrift and others (`v2xw-threat/src/attack.rs`, roughly lines 160-200). Report poisoning (framing) by colluders: `v2xw-threat/src/poison.rs`.
- **Penetration.** `actors.vehicles.equipped_fraction` (`v2xw-engine/src/scenario/schema.rs:984`). The draw is keyed per actor (`run.rs:3384-3393`, ADR 0004 §3), so equipped sets are likely nested across fractions, which suits paired comparison. I did not verify that `.bool()` is a monotone threshold.
- **RSUs at signalised intersections.** `RsuPlacement { count, roles, min_spacing_m }` (`schema.rs:939-967`) places units at signals, most approach lanes first. The roles include `DetectorHost` and `ReportForwarding` (`v2xw-node/src/rsu.rs:115-135`).
- **Radio technologies.** `Rat::{Dsrc80211p, LteV2xPc5, NrV2xPc5, Hybrid}` (`schema.rs:~2048-2055`).
- **Measurement harness and maps.**
  - `crates/v2xw-engine/examples/mbd_eval.rs` covers `--fraction`, `--ma-params JSON`, `--no-attackers` and the ground-truth precision/recall joins.
  - Records: `MaReportRecord` and `MaDecisionRecord` (`v2xw-threat/src/records.rs:85-117`).
  - `worlds/cities.yaml` has 73 cached city extracts.

**Must be built (rough size):**

1. **Per-event witness metric.** For each ground-truth attack event, record the distinct reporters, the equipped vehicles and RSUs in radio range (the potential witnesses) and the trusted ones. Roughly 150-250 lines in phase2/metrics.
2. **Offline MA replay.** `MaReportRecord` lacks observation time and the reporter's certificate digest and trust flag, all of which `CorroboratedMa::ingest_evidence` uses. Adding them, plus a small replay binary, lets one run feed a full threshold sweep instead of one rerun per threshold. Roughly 100-200 lines. Without it, threshold sweeps multiply run cost.
3. **Sybil reporter.** A colluder that signs reports with several concurrently valid pseudonyms, and an MA option to link reporters (with the linkage disclosure counted as a privacy cost). Roughly 200-400 lines. Rotation exists (`poison.rs:78-82, 181`); concurrent use does not appear to.
4. **Plan-level RSU presets** (fraction of signals as `count`) and a deployment-stage scenario family: configuration only.

**Limits that undermine parts of the pitch:**

- **"Outside urban cores" cannot be evidenced as stated.** Every city entry is "a downtown box of about 1.5 km × 1.5 km chosen to take in the core's signalised arterials" (`worlds/cities.yaml` header). The only highway is a procedural 1.5 km six-lane road (`scenarios/highway-congestion.yaml`). There are no real NHS corridors, rural arterials or suburban roads. The RSU-outside-cores verdict and the "20% of NHS" axis need new imported corridor or suburban worlds (an import job, not engine code), or the claim must be dropped.
- **Box edges cut off witnesses.** A 1.5 km box truncates witness sets near its border, so events must be restricted to an inner region.
- **"Paired reruns are exact" is overstated.** Traffic is identical across penetration only if the GLOSA and platooning loops are off (THOUGHTS.md: only those close the loop). Radio is not paired: changing penetration changes the set of transmitters, channel load and DCC. RNG streams are keyed per entity, not per event (THOUGHTS.md), so the reception draws of the same witness differ between runs. Claim common traffic, not exact pairing.
- **Cost.** About 2.4 h per 600 s run at about 450 vehicles (THOUGHTS.md). Penetration (about 6 levels) × RSU (about 3) × radio (3) × maps (50+) is roughly 2,700+ runs. Low-penetration runs are cheaper (fewer radios), but the full grid is still out of reach. Use about 10 maps stratified by signal density, with the full grid on 2-3 maps, plus offline MA replay (item 2).
- **Partial engine claim.** Detection is open-loop for driving, but revocation feeds back into who transmits. Fine, as long as the paper says so.

## Bottom line

The direction is worth pursuing only if it is reframed:

- **Scope.** Restrict it to class 4/5 evidence; say why classes 1 and 2 are self-proving.
- **Lead.** Lead with the Sybil-reporter versus reporter-linkage-privacy trade-off, which I found unpublished, and with measured witness availability at plan-level penetration and RSU density.
- **Credit.** Credit Bißmeyer 2012 and the Intel patent for the concept.
- **Claims.** Drop or defer the "outside urban cores" claim until corridor or suburban worlds exist.

Engine fit is good: the corroborating MA, RSU detector hosts, penetration, three radio technologies, perception and framing are all present. About 0.5-1 kLOC of measurement and attack code is needed.
