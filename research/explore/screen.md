# Screen of the 14 candidate directions (2026-10-18)

**Inputs:**
- `research/explore/ideas-needs.md` (N1–N7)
- `research/explore/ideas-engine.md` (D1–D7, by key)
- `research/explore/THOUGHTS.md` (the three criteria, the lessons and the engine limits)

**Scores:** each criterion is scored 1–5. Nov = novelty, Worth = an adopter changes a decision, Eng = the engine gives evidence that cannot be obtained otherwise, Feas = feasibility within the engine limits. A score below 3 on any criterion kills the candidate.

## 1. Searches run in this screen

**Corpus.** A regex script over the 11,315 records in `research/catalog.jsonl` (title and abstract), with 15 topic patterns. The closest hits per topic:

| Topic | Closest corpus hits |
|---|---|
| Fault versus attack | `ghosh2009misbehavior` (doi:10.1145/1614269.1614292, "integrated root cause detection", title only); `rezgui2011detecting` (doi:10.1109/lcn.2011.6115558, faulty versus malicious vehicles); `raya2007eviction` (doi:10.1109/jsac.2007.071006) |
| Backend detection | `berlin2016poster` (doi:10.1109/vnc.2016.7835978, a backend SIEM concept, not evaluated) |
| Early deployment | `park2013defense` (doi:10.1002/sec.679, Sybil defence in the initial deployment stage) |
| Malicious RSU | `abhishek2021drive` (doi:10.1109/jiot.2021.3097809), `alnasser2021trust`, `mao2023detecting`. All are RSU data-integrity schemes with no SCMS and no reporting path |
| Revocation | `scopelliti2024efficient` (doi:10.14722/ndss.2024.24017, TEE-enforced timely self-revocation) |
| Verification load | `krishnan2011verify` (doi:10.4271/2011-01-0584); `ko2021avod` (doi:10.1155/2021/2890132) |

**Fresh searches:**
- OpenAlex: 19 queries.
- Crossref: 7 queries.
- arXiv export API: 8 queries, 3 s apart.
- WebSearch: 7 queries.
- Semantic Scholar refused every query with HTTP 429, so it is not counted.

What the searches returned:
- **Zero hits** in OpenAlex for four of the queries:
  - "misbehavior report false positive honest vehicles GPS error urban canyon V2X"
  - "misbehavior authority remediation recall revocation fleet defect V2X"
  - "V2X hardware security module key extraction mass revocation re-enrollment"
  - "compromised roadside unit SPaT falsification revocation misbehavior report suppression"
- **Zero hits** in arXiv for "misbehavior AND penetration AND V2X/vehicular".
- **Nothing on point** from WebSearch on four topics: GNSS-driven false positives on honest vehicles, penetration rate versus misbehaviour detection, compromised RSUs that falsify SPaT, and cooperation between MAs of different providers.
- **Closest new items found:**
  - SPADE (arXiv 2609.02741): vehicle-side detection, with no PKI.
  - Blackchain (arXiv 1710.08891): notes that competing SCMSs will coexist and proposes a ledger. It is an architecture, with no measurement.
  - A VRU survey (arXiv 2010.15502): frames false VRU alerts and the need for credentials.
  - arXiv 2402.17763: reduces unnecessary P2V alerts using real phones.
  - Tennessee RSU/SCMS field issues (arXiv 2304.02885).
  - Patents US 12470954 (GNSS fault leads to a wrong misbehaviour decision) and US 12003966 (CRL latency of hours to days). Both name the concern but measure nothing.

**Limits:**
- Absence after these searches is not proof of absence.
- The texts of SAE J3287, J2945/9 and IEEE 1609.2.1 were not opened, so claims about their scope remain "verify".

**Engine assets checked by grep:**
- GNSS deep-canyon σ multiplier 31.02/3.07 (`crates/v2xw-mobility/src/gnss/common.rs` L60–65).
- The compromised RSU: false SPaT is "recorded and not yet felt", while report dropping is built (`crates/v2xw-threat/src/attack_rsu.rs` L11–25).
- Peer SCMS (`crates/v2xw-proto/src/scms/run/mod.rs` L1142 `new_peer`).
- `EeCache` (`crates/v2xw-node/src/rxval.rs`).
- A penetration knob (`crates/v2xw-copilot/src/catalog.rs` L209).
- `DeviceClass` exists only as a crypto-cost class (`crates/v2xw-sec/src/primitive.rs` L1452), so N1's defective-device-class model is genuinely new work.

## 2. Merges

| Merged entry | Members | Why merged |
|---|---|---|
| **A. Honest but wrong** | N1 + `benign-mbr-load` | The same pipeline: benign GNSS error or a defect class → local detection under the deployed rules → reports → MA. `benign-mbr-load` measures the volume and the crowd-out; N1 measures whether the MA can attribute a shared cause. One study, two layers. |
| **B. Systemic credential events** | N2 + N6 (+ the recovery-surge part of `scms-calendar-load`) | All three are about a whole class or provider losing credentials, and about the re-enrolment surge. N6 itself recommends the merge (ideas-needs.md N6). |
| **C. Multi-provider operations** | `cross-provider-ma` + `ctl-change-propagation` | Both concern the multi-provider rules in the Plan and CP-TR, and they share the peer-SCMS machinery. ideas-engine.md D5 suggests the fold. |

## 3. Scores

| # | Candidate | Nov | Worth | Eng | Feas | Verdict and reason |
|---|---|---|---|---|---|---|
| A | Honest but wrong (N1 + benign-mbr-load) | 4 | 5 | 5 | 3 | **Keep (1).** See note A below. |
| N4 | The compromised intersection | 4 | 4 | 4 | 4 | **Keep (2).** See note N4 below. |
| N3 | The bootstrap paradox | 4 | 4 | 4 | 5 | **Keep (3).** See note N3 below. |
| C | Multi-provider operations (cross-provider-ma + ctl-change) | 4 | 4 | 3 | 4 | **Keep (4).** See note C below. |
| B | Systemic credential events (N2 + N6) | 4 | 4 | 3 | 3 | **Alternate (5th; passes the bar but loses to C).** See note B below. |
| N5 | Unpinned VRU channel | 3 | 3 | 3 | 3 | **Passes the bar narrowly; not kept.** See note N5 below. |
| N7 | Batch of 20 simultaneously valid certificates | 2 | 3 | 3 | 4 | **Kill (novelty).** See note N7 below. |
| rsu-cert-availability | RSU certificate lifecycle and GLOSA | 4 | 2 | 3 | 3 | **Kill (worth).** See note on rsu-cert-availability below. |
| us-pki-audit | Audit of the deployed US V2X PKI | 5 | 4 | 2 | 4 | **Kill as a thesis direction (engine).** See note on us-pki-audit below. |
| scms-calendar-load | The weekly heartbeat: load on the real calendar | 3 | 3 | 2 | 3 | **Kill standalone (engine).** See note on scms-calendar-load below. |
| obu-verify-budget | Verification budget per OBU | 3 | 2 | 4 | 3 | **Kill standalone (worth).** See note on obu-verify-budget below. |
| ctl-change-propagation | (alone) | 4 | 3 | 2 | 3 | Merged into C; alone it fails the engine criterion (ideas-engine.md D5). |
| N6 | (alone) | 3 | 3 | 2 | 4 | Merged into B; alone it fails the engine criterion (ideas-needs.md N6). |

### Notes on the scores

**A. Honest but wrong (keep, 1).**
- **Novelty.** Prior work classifies one vehicle at a time as faulty or attacking: `rathore2023distinguishing`, `das2026distinguishing` (doi:10.1109/ccnc65079.2026.11366546), `rezgui2011detecting`, `ghosh2009misbehavior`. VeReMi labels faulty nodes but does not derive them (`kamel2020veremi`). Nothing was found at the population or MA level under the SCMS privacy split, and nothing on benign load under the deployed CV-pilot MBR rules.
- **Worth.** The written needs are MBD-Op p12–13 (5GAA 2024, cause-dependent remediation), the SCMS Manager MBR spec v1.0 §2.2–2.3 (a 1 m gpsDrift trigger, the decay and the quota; via `docs/design/scms-real/03-protocols-and-operations.md` L295–300) and J3287, which fixes the format only (doi:10.4271/J3287_202408).
- **Engine.** The result needs GNSS by location, who witnesses, which reports are uploaded, and the true device behind every pseudonym. Detection and reporting are open-loop or local, so replay is exact.
- **Feasibility.** About 2–3 weeks of extensions: the MBR check set, an on-device report store, the defect-class model and a GNSS-quality field.
- **To verify.** What "gpsDrift" means in the MBR spec. It extends the framing idea the owner liked.

**N4. The compromised intersection (keep, 2).**
- **Novelty.** SPADE (arXiv 2609.02741) is vehicle-side and has no PKI. I-SIG (NDSS 2018) forges BSMs. DRiVe and `abhishek2019detecting` check RSU data integrity without an SCMS. No work was found on a compromised RSU suppressing its own revocation.
- **Engine.** Report suppression is already built (`attack_rsu.rs` L24–25; `poisoning.rs` L586). Witness geometry, backhaul, the cellular path and the MA run jointly.
- **Feasibility.** The half on false SPaT harm needs GLOSA to consume the SPaT it received (1–2 weeks) and is optional.
- **Risk.** "Path diversity obviously helps." The measured contribution is how long revocation is delayed in sparse areas and how much incidental bypass occurs.

**N3. The bootstrap paradox (keep, 3).**
- **Novelty.** No study was found of penetration against the MA corroboration threshold (arXiv: 0 hits; OpenAlex and WebSearch: nothing on point). `park2013defense` covers Sybil defence in early deployment, without an MA.
- **Engine.** Witness counts depend jointly on map geometry, radio and traffic clustering, and detection is open-loop.
- **Feasibility.** No extension is needed (`catalog.rs` L209).
- **Risk.** It may be called a "sensitivity sweep". The defence is a stage-dependent policy for the MA operators and USDOT ITS JPO. It shares the proof-based MA model with A and with the framing work.

**C. Multi-provider operations (keep, 4).**
- **Novelty.** Blackchain (arXiv 1710.08891) and `brecht2018security` treat multiple SCMSs only architecturally. No measurement of evidence split across MAs, or of CTL propagation, was found (OpenAlex, WebSearch).
- **Worth.** CP-TR says an MA "should cooperate" (2025); the SCMS Manager's CTL Update Process is "in development" (`01-deployment-and-governance.md` §1.2).
- **Engine.** The split share itself is analytical (1 − Σs²). The engine adds who witnesses whom and the timing; MA policy changes are backend-only.
- **Feasibility.** A few days for per-provider MA settings. The CTL part needs vehicles to persist across days (1–2 weeks) and is optional.

**B. Systemic credential events (alternate, 5th).**
- **Why it lost to C.** Awareness loss from p % of the fleet going silent is close to analytical, and the crossover rests on an assumed attacker extraction rate. C needs less extension work.
- **Closest prior work:** `scopelliti2024efficient` (NDSS 2024); arXiv 2004.03407; 1705.06903 (CRL distribution); EUCLEAK (ePrint 2024/1380) and ROCA (doi:10.1145/3133956.3133969) as precedents for flaws that hit a whole class of devices.
- **Feasibility.** ECA revocation and bulk re-bootstrap (1–2 weeks), with week-long horizons split into backend-only runs and short traffic snapshots.

**N5. Unpinned VRU channel (passes narrowly; not kept).**
- arXiv 2010.15502 already frames false VRU alerts and the credential requirement; 2402.17763 studies unnecessary P2V alerts with real phones.
- J2945/9 (doi:10.4271/j2945/9_201703) was not read, so the adopter's decision is unverified.
- The result depends on how realistic the phone GNSS and VRU motion models are.

**N7. Batch of 20 simultaneously valid certificates (kill: novelty).**
- The trade-off is stated in `brecht2018security` p7.
- The privacy axis is the "who can follow" study the owner dropped (THOUGHTS.md), and Sybil detection is crowded (`tulay2024sybil`).
- What remains is a parameter study.

**rsu-cert-availability (kill: worth).**
- The decision already has an obvious answer: RSUs are fixed, so short RSU certificates buy no privacy, and USDOT already recommends a three-year preload (ITS KR 2024-L01214).
- Withheld RSU-hours are close to "outage fraction × hours".
- The GLOSA test was failing at `6f335bb` (ideas-engine.md D3).

**us-pki-audit (kill as a thesis direction: engine).**
- The novel core, the audit, is not simulation, and the impact half is bolted on (ideas-engine.md D6).
- Keep it as a side contribution, after responsible disclosure.

**scms-calendar-load (kill standalone: engine).**
- The RA burst is analytical unless cell contention binds (ideas-engine.md D4). That can be checked with one Manhattan peak run.
- Its recovery-surge part goes into B.

**obu-verify-budget (kill standalone: worth).**
- It is narrow, the same class as "pricing detector errors", which the owner dropped (THOUGHTS.md).
- It can become a chapter of A or B once `EeCache` is calibrated.

## 4. Finalists, in order

1. **A. Honest but wrong.** Merges N1 and `benign-mbr-load`.
2. **N4. The compromised intersection.**
3. **N3. The bootstrap paradox.**
4. **C. Multi-provider operations.** Merges `cross-provider-ma` and `ctl-change-propagation`.

**Shared machinery.** A, N3 and C all need the proof-based MA model that the owner-liked framing work requires. Together they form one thesis spine: "misbehaviour remediation under realistic deployment". N4 extends it to infrastructure.

**Must verify next:**
- The SCMS Manager MBR spec v1.0 trigger semantics.
- Whether J3287 covers reports on infrastructure messages such as SPaT and MAP.
- The CP-TR cooperation wording, against the primary PDF.
- A blind prior-art hunt for each finalist, including "misbehavior detection partial deployment / CPM penetration" in Artery and Veins work.
