# Ideas from documented field needs (lens: field needs first)

Written 2026-10-18 in about 20 minutes. I started from `research/synthesis/big-needs.md`,
`open-questions.md` and `critique-blind-proposals.md`, and checked the three criteria and the
status table in `research/explore/THOUGHTS.md`. None of the seven directions below is on that
table. N1 is the "substantially stronger reframing" of the framing idea that the task allows.

**Primary sources read in this session:**
- **5GAA, *MBD for V2X: Operational Aspects*, v1.0, 2024** ("MBD-Op"; https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf). Relevant pages: p9 (fairness, and no specified conditions for remediation), p12 ("Localised" misbehaviour, e.g. "a particular vehicle manufacturer, year or model"), p13 (cause categories: malfunctioning EE such as GPS, HW/SW design error, cybersecurity incident, misuse; monitoring after remediation) and p16/p19 (suspension techniques ST0–ST3 and their costs).
- **5GAA, *Credential Management Supporting V2X Commercial Deployments* (CP-TR), 2025** (https://5gaa.org/content/uploads/2025/07/credential-management-supporting-v2x-commercial-deployments.pdf). Relevant sections: §4.10 (revocation for compromise, items 238–251), §5.7 (compromise and disaster recovery: 5.7.3 key compromise, 5.7.4 business continuity) and §5.8 (termination and transfer).
- **Brecht et al., SCMS, arXiv 1802.05323** (`brecht2018security`). Relevant pages: p7 (simultaneously valid certificates "would enable masquerading as multiple devices… Sybil attack"; batch size at least 20, valid 1 week, covering 1–3 years), p20–21 (re-enrolment: "a device should be re-bootstrapped" after an ECA is revoked; per-OEM ECAs in Fig. 7) and p22 ("the SCMS needs the ability to heal itself").
- **EUCLEAK, IACR ePrint 2024/1380.** A side channel in Infineon's ECDSA library: "all Infineon security microcontrollers (including TPMs) that run the Infineon cryptographic library… are vulnerable". Infineon sells V2X HSMs (SLS37 V2X HSM, https://www.electropages.com/2021/10/hardware-security-module-safeguards-vehicle-everything-communication; SLI 97 in Autotalks V2X, https://auto-talks.com/autotalks-introduces-highly-integrated-v2x-security-with-trusted-v2x-signing-firmware-on-infineon-sli-97-security-controller/). **No source says those V2X parts are affected.** EUCLEAK is cited only as a precedent for vulnerabilities that hit a whole class of secure elements. ROCA (Nemec et al., CCS 2017, doi:10.1145/3133956.3133969) is the earlier precedent.
- **Engine, read with grep and sed only:**
  - GNSS model with sky-view classes (deep canyon) and jamming: `crates/v2xw-mobility/src/gnss/common.rs` L64, `ou_bias_legacy.rs` L72–75 and L203–212.
  - Compromised-RSU attacker: `crates/v2xw-threat/src/attack_rsu.rs` header. Its SPaT/MAP falsification "is recorded and not yet felt"; the report-forwarding attack already works.
  - Attack kinds include Sybil, VruImpersonation, VruPositionSpoof and FakeHazard: `attack.rs` L148ff.
  - Metrics: awareness and age of information (`v2xw-metrics/src/awareness.rs`), plus `safety.rs` and `vru.rs`.
  - MA and corroboration: `v2xw-threat/src/ma.rs`, `ma_corroborate.rs` and `ts103759.rs`. The test `crates/v2xw-threat/tests/poisoning.rs` L586 is a compromised unit suppressing evidence.

**Search gaps:**
- arXiv API queries came back nearly empty, so some arXiv-only prior work may have been missed.
- OpenAlex gave little for most queries. Queries run: GNSS or fault false reports, Sybil with simultaneously valid pseudonyms, mass revocation, MBD versus penetration, reporting volume, SCMS outage, SPaT spoofing, VRU/PSM spoofing.
- The texts of SAE J3287, J2945/9 and IEEE 1609.2.1 were **not** opened. Statements about their scope are marked "verify".

Every direction below respects these engine limits:
- It uses open-loop, backend-only or local effects, or it names and sizes the extension it needs.
- Safety is measured as **awareness or alert exposure** (who knew what, when, against ground truth), never as crashes avoided, because warnings do not yet change driving.

---

## N1. Honest but wrong: finding the root cause of misbehaviour across a population at the MA (a reframing of "framing")

**Problem.** MBD-Op p13 says the remediation decision "depends on the reason of the misbehaviour". It lists four causes: accidental malfunction (e.g. GPS), a HW/SW design error, a cybersecurity incident, and misuse. At p12 it allows that misbehaviour may be confined to "a particular vehicle manufacturer, year or model". Yet every MA design and evaluation in the corpus judges one sender at a time. The MA receives J3287 reports about pseudonyms. Under the SCMS privacy split it cannot see make, model or GNSS environment without cooperation from the LA, PCA and RA.

That raises three questions:
- Can an MA tell a deep-canyon GNSS cluster, or a firmware bug in one OEM's units, from an attack?
- How fast can it tell?
- How many honest vehicles get revocation-grade evidence before it does?

Framing by an attacker is then one special case of a broader failure: **honest vehicles that are wrong for a shared reason**. That failure is likely far more common than attacks.

**Decision it informs:**
- MBD-Op §8 open items: draft policy specification and new MBR fields (p9, and big-needs need 2).
- Which remediation tier to use: update/recall (R1) versus suspension or revocation.
- What minimum population-level signal J3287 reports or MA–RA cooperation must carry, for example a GNSS-quality field or a coarse device-class hint, and at what privacy cost.

**Engine evidence.** City-scale traffic with the engine's sky-view GNSS model (deep canyons, jamming) and a new "defective device class" model. One example is a heading or time bug in x % of units that triggers under a condition. These feed the TS 103 759 detectors, real J3287-style reports, the proof-based MA and the LA linkage, all with per-message ground-truth cause labels.

Only the engine joins:
- where GNSS degrades (map geometry);
- who is near enough to witness it (radio);
- which reports reach the MA (reporting path).

None of that comes from a formula or from the Tampa or VeReMi data, which have no cause labels or SCMS layer.

**Expected headline (hypothesis):**
- Under per-sender MA rules, a measurable share of honest vehicles in urban-canyon corridors or in one defective class collect revocation-grade evidence within a week.
- Spatial and temporal clustering, together with one extra MBR field, lets the MA attribute a common cause at precision P with a delay D.
- Each extra field is priced in bytes and in linkability.

**Adopter:** 5GAA MBD4V2X / MBD-Op authors, MA operators of SCMS providers, the SAE J3287 revision, and OEMs, who carry recall liability.

**Closest prior work:**
- Per-vehicle fault-versus-attack classifiers on Tampa CV Pilot BSMs: `das2026distinguishing` (doi:10.1109/ccnc65079.2026.11366546) and `rathore2023distinguishing` (doi:10.1109/mobisecserv58080.2023.10329092).
- VeReMi Extension's split of message labels into "faulty" and "attack" (`kamel2020veremi`).
- Eviction of faulty nodes (`raya2007eviction`, doi:10.1109/jsac.2007.071006).
- A patent noting that GNSS faults cause false misbehaviour decisions (US 12470954, https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/12470954).

None of these works at the MA or population level under the SCMS privacy split, or ties the result to a remediation tier.

**Engine role:** central. It provides GNSS by location, witnessing, reporting, the MA and LA, and ground truth.

**Extension:** a device-class attribute plus a conditional defect model, and a GNSS-quality field in reports. About 1–2 weeks.

**Risk:** a reviewer may say "cluster analysis is obvious". The rebuttal is the measured honest-vehicle harm and the privacy price of each signal.

---

## N2. Class break: responding to a fleet-wide key-extraction flaw

**Problem.** Secure-element flaws arrive by class:
- ROCA (CCS 2017) made a national ID-card fleet unsafe.
- EUCLEAK (ePrint 2024/1380) reports an ECDSA side channel in "all Infineon security microcontrollers… that run the Infineon cryptographic library". Infineon also sells V2X HSMs; whether those are affected is not claimed here.

The SCMS gives an operator three levers:
- revoke the ECA, which forces "re-bootstrapping" (brecht2018security p21);
- stop issuing certificates (MBD-Op ST0, p16), so certificates lapse by the end of the week;
- revoke individual units as they are caught.

Each lever trades **attacker exploitation** of extracted keys against **silencing a whole class of honest vehicles**, which loses awareness of them, plus CRL growth and a re-enrolment surge. CP-TR §5.7.3 and §4.10 (item 238) say compromised enrolment certificates "shall be revoked", but give no basis for choosing timing or scope.

**Decision it informs:**
- The incident playbook of the SCMS Manager and providers (CP-TR §5.7).
- Whether field re-enrolment needs a standard path in 1609.2.1 (brecht p21 assumes a re-bootstrap in a secure facility).

**Engine evidence.** All measured jointly, and only in the engine:
- awareness loss among true neighbours (`awareness.rs`) when p % of a city's fleet goes silent, by city, penetration and radio technology;
- Sybil or ghost exploitation with valid extracted keys until each lever takes effect;
- CRL bytes and distribution time over RSU backhaul and cellular, and the verifier's CRL-check load;
- the RA, cellular and TLS surge when the class re-enrols.

**Expected headline (hypothesis).** There is a crossover in the attacker's extraction rate. Below it, waiting for ST0 to lapse at the week boundary dominates immediate ECA revocation. Above it, revoking at once is worth the awareness loss. Results are given per city type.

**Adopter:** SCMS Manager, SCMS providers, OEM security teams, IEEE 1609.2.1 (re-enrolment).

**Closest prior work:**
- CRL scaling and activation codes: `smith2020let`; ePrint 2018/324.
- Birthday attacks on SCMS revocation: ePrint 2018/185.
- Self-healing of root compromise, argued qualitatively: brecht p22.
- The blind proposal "resilience and crypto-agility", which is about authority compromise, not device classes (`critique-blind-proposals.md`, Direction C).

No measured class-break response study was found (OpenAlex "mass revocation V2X compromised devices" query; WebSearch).

**Engine role:** central for awareness and exploitation; backend for CRL and re-enrolment.

**Extensions:**
- ECA-level revocation and a bulk re-bootstrap flow (check `crates/v2xw-proto/src/scms/enrolment.rs`), about 1–2 weeks.
- Week-long horizons must be split: backend-only week runs, plus short traffic runs at chosen snapshots.

---

## N3. The bootstrap paradox: MA evidence rules along the USDOT deployment trajectory

**Problem.** A proof-based MA needs independent witnesses. bimeyer2012central assumes a benign majority of reporters, and our framing work shows single-reporter rules can be abused. The USDOT plan starts thin: V2X on 20 % of the NHS and at 25 % of top-75-metro signalised intersections by 2028 (big-needs "Plan"), with low on-board-unit (OBU) penetration.

At early penetration an attack event may have zero or one equipped witness. A threshold that resists framing at maturity would leave attackers untouchable in 2028. A threshold that acts in 2028 lets one attacker frame honest vehicles. Nobody has measured where that crossover lies, or whether RSU-hosted detection at Plan intersections closes it.

**Decision it informs:**
- MA corroboration thresholds by deployment stage (MBD-Op §8 draft policy).
- Whether public agencies should fund RSU-side detection and reporting as "witnesses of last resort" under the Plan's intersection targets.

**Engine evidence.** Counts of independent witnesses per attack event, and the MA's time to act, swept over:
- penetration from 1 % to 50 %;
- RSU density at Plan levels;
- city type (50+ maps);
- radio technology.

Detection is open-loop, so exact paired reruns are valid. Witness counts depend on map geometry, radio reception and traffic density together. A Poisson approximation misses the clustering on arterials, which is exactly what the engine resolves.

**Expected headline (hypothesis).** Below about X % penetration, most incidents off arterials have fewer than k independent witnesses. RSU witnesses at the Plan's 25 % of intersections lift only urban cores. So any fixed k is either framing-prone or toothless before about year Y of the Plan.

**Adopter:** USDOT ITS JPO (Plan), SCMS MA operators, 5GAA MBD-Op.

**Closest prior work:**
- RSU-assisted Sybil defence in the initial deployment stage (`park2013defense`, `park2009defense`).
- Central evaluation that assumes a benign majority (`bimeyer2012central`).
- A4MD (`willecke2023a4md`), which models reporting but not deployment stage.

No penetration-versus-MA-threshold study was found (OpenAlex queries above).

**Engine role:** central.

**Extensions:** none beyond penetration and RSU placement sweeps, which the configuration harness already exposes (`v2xw-copilot/src/catalog.rs` L209 lists "penetration").

**Risk:** reviewers may call it a sensitivity study. The answer is that it produces a concrete stage-dependent policy.

---

## N4. The compromised intersection: an RSU that lies, filters reports and relays the CRL that should revoke it

**Problem.** Under the Plan, signalised intersections become the trust anchors of V2I: SPaT/MAP comes from the RSU, and reports and CRLs often travel through RSU backhaul. A compromised RSU is therefore an attacker on three surfaces at once:
- it sends false SPaT/MAP with valid credentials;
- it drops the reports that would accuse it;
- it controls CRL relay to the vehicles near it.

The engine already models the first two surfaces (`attack_rsu.rs`, and `poisoning.rs` L586). MBD-Op p13–14 treats RSU remediation as remote or on-site work. No source found sets requirements for path diversity of reports and CRLs.

**Decision it informs:**
- Whether OBUs must upload reports and fetch CRLs over a path that does not go through the RSU (cellular).
- Whether vehicles should run MBD on SPaT/MAP and report RSUs. The J3287 scope for infrastructure messages needs to be verified.
- RSU certificate validity and the revocation cadence for infrastructure.

Adopters include the ITE/AASHTO/NEMA Connected Intersections programme and the USDOT RSU specification.

**Engine evidence:** time to neutralise a compromised RSU, with and without path diversity; the share of reports suppressed; the number of vehicles exposed to false SPaT before revocation. All three come from joint traffic, radio, backhaul and MA, with ground truth.

**Expected headline (hypothesis).** Without path diversity a compromised RSU can delay its own revocation indefinitely in low-penetration areas. One cellular reporting path restores detection within the MBDR cycle.

**Adopter:** Connected Intersections / USDOT RSU specification, SAE J3287 revision, state DOTs.

**Closest prior work:**
- SPADE, a vehicle-side SPaT attack dataset in MOSAIC, with no PKI or reporting path (`novo2026spade`, arXiv 2609.02741).
- I-SIG congestion attack (Chen et al., NDSS 2018), which forges BSMs into signal control rather than compromising an RSU.
- Intersection MBD for collective perception (Zhang et al., VTC2024-Fall, doi:10.1109/vtc2024-fall63153.2024.10757840).

**Engine role:** central for report suppression and the time to neutralise.

**Extension:** to make false SPaT matter to driving, GLOSA must consume the SPaT it received rather than ground truth (`attack_rsu.rs` says the harm is "recorded and not yet felt"). About 1–2 weeks. The suppression results do not need it.

---

## N5. An unpinned VRU channel: credential and trust policy for phone-based pedestrian messages

**Problem.** Vehicle-to-pedestrian safety will rely on phones and other low-assurance devices sending PSMs. These devices have no automotive HSM and have worse GNSS. A receiver faces a two-sided error:
- phantom pedestrians (spoofed or poorly located PSMs) trigger false alerts;
- discounting low-assurance senders misses real VRUs.

No study was found that sets a reception and credential policy for VRU messages against measured alert exposure. A search for "PSM smartphone spoofing assurance J2945/9" returned only patents and general MBD. The J2945/9 text was **not** read; verify its security clauses.

**Decision it informs:**
- Receiver trust weighting by assurance level (the 1609.2 subject-assurance field; verify).
- PSM credential issuance policy for phones: batch size, rate limits.
- Whether VRU MBD needs its own detector thresholds given phone GNSS error.

Adopters: SAE J2945/9 committee, 5GAA VRU work, USDOT Safe System / VRU programmes.

**Engine evidence.** Open-loop alert exposure needs no closed-loop driving: false VRU alerts per vehicle-hour against missed true VRU conflicts. It is measured over the engine's VRU agents with sky-view GNSS error, PSM delivery through the radio, and the existing `VruImpersonation` and `VruPositionSpoof` attacks, with `vru.rs` metrics. Only the engine has true VRU-vehicle conflict geometry, the radio and spoofing together.

**Expected headline (hypothesis):** a Pareto curve. A simple assurance-plus-GNSS-confidence rule cuts phantom alerts by an order of magnitude at a bounded miss rate.

**Closest prior work:**
- VASP (NDSS VehicleSec 2023, https://ndss-symposium.org/wp-content/uploads/2023/02/vehiclesec2023-23071-paper.pdf), a V2X spoofing platform.
- General MBD surveys.

No VRU-credential policy study was found.

**Engine role:** central.

**Risk:** the realism of phone GNSS and VRU motion. The GNSS sky-view model helps.

---

## N6. When an SCMS provider goes dark: credential starvation and recovery under the multi-provider plan

**Problem:**
- The Plan requires interoperable SCMS providers (big-needs "Plan").
- CP-TR has sections on business continuity (§5.7.4) and termination and transfer (§5.8).
- Brecht allows 1–3 years of certificate coverage (p7), and a USDOT briefing weighs a three-year preload against weekly monitoring (big-needs need 6).

Nobody has measured what happens when one provider's RA or MA is down for days. A ransomware incident is the obvious case. The following are all unmeasured:
- when each vehicle class and each RSU runs out of valid pseudonyms, given real top-up schedules and real connectivity;
- what awareness is lost;
- what happens to revocation while the provider's MA and CRL are frozen;
- how large the surge is when the RA returns.

**Decision it informs:**
- The minimum preload depth and top-up lead time in the CP.
- Whether cross-provider failover (re-enrolling with provider B) must be specified.
- Which priority order the RA should serve when it recovers.

**Engine evidence:**
- certificate-inventory trajectories per vehicle under the real calendar and 1609.2.1 top-ups, with connectivity driven by traffic and the cellular model;
- the RA, TLS and cellular recovery surge;
- awareness loss as silent vehicles accumulate.

The starvation curve alone is close to analytical. The engine adds heterogeneous connectivity and the recovery queueing.

**Expected headline (hypothesis):** the minimum preload, in days, below which a provider outage of D days silences more than X % of a class. The recovery surge exceeds RA capacity unless it is paced.

**Adopter:** SCMS Manager / CP-TR authors, USDOT, providers.

**Closest prior work:**
- VPKI scalability and on-demand pseudonym acquisition (`khodaei2023secmace`, `khodaei2016evaluating`).
- SCMS healing, argued qualitatively (brecht p22).

No provider-outage study was found (OpenAlex "SCMS unavailability pseudonym certificate expiry" returned 1 unrelated hit).

**Engine role:** supporting to central.

**Weakest on criterion 3.** It is stronger merged with N2 as "systemic credential events".

---

## N7. How many identities should a car hold? Re-deciding the batch of 20 simultaneously valid certificates

**Problem.** Brecht p7 states the trade-off openly. Many simultaneously valid certificates "would enable masquerading as multiple devices… Sybil", and the batch is set at a minimum of 20 per week. In practice NYC raised it to 60 a week for taxis (big-needs "CVP"). A 2024 thesis shows ghost vehicles built from legitimate pseudonyms evade state-of-the-art MBD (https://scholar.utc.edu/theses/975). IFAL (doi:10.1109/eurosp.2019.00029) changes when certificates activate, not how many are valid at once.

No one has measured the joint frontier, for the same traffic:
- privacy against tracking under J2945/1-style change rules;
- Sybil capacity per compromised device, against what local MBD and weekly MA linkage catch;
- supply exhaustion per fleet type.

**Decision it informs:** the SCMS Manager's batch size and overlap parameters; an HSM-enforced limit on concurrent use, for example "at most 2 active per 5 min"; fleet-specific supply (taxis).

**Engine evidence:** all three axes measured on one deterministic traffic and radio run with true identities. Sybil detection uses the existing `Sybil` attack, TS 103 759 detectors and LA linkage.

**Expected headline (hypothesis).** Going from 20 to 60 adds little privacy for taxis but triples undetected ghost capacity. A concurrency cap restores most of the Sybil margin at negligible privacy cost.

**Adopter:** SCMS Manager, IEEE 1609.2.1, SAE J2945/1, fleet operators.

**Closest prior work:**
- Sybil detection on VeReMi Extension variants, which is crowded (`tulay2024sybil`, `park2013defense`).
- IFAL.
- The UTC thesis.
- Privacy-tracking work, which the owner judged small ("who can follow").

**Engine role:** central for the joint frontier.

**Risk:** the owner may see it as a parameter study. It is the weakest of the seven.

---

## Ranking and notes

1. **N1** is the strongest: it has a written need (MBD-Op p12–13), extends the idea the owner liked to a population scale, and needs no closed loop.
2. **N2**: a high-stakes incident decision that is not written down. The precedents are real.
3. **N3**: an original tension that only appears along the deployment trajectory.
4. **N4**: the engine already has two of the three surfaces.
5. **N5**: a fresh area; needs verification of J2945/9.
6. **N6**: better merged with N2.
7. **N7**: weakest.

**Natural thesis core.** N1, N2 and N3 share one MA-and-remediation machine, which makes them three chapters of a single "remediation under realistic failure" thesis. N4 extends that to infrastructure.
