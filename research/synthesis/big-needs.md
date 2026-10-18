# Big needs: what industry, standards bodies and government say is unsolved in V2X security

Written 2026-10-18 under a 20-minute budget. Primary sources were read where they could be fetched.
Where a source could not be found, the text says so.

## What was searched and not found

- **No 5GAA 2026 work item rated "Critical but unfunded" was found.** The 5GAA work-item page
  (https://5gaa.org/work-items/) shows no priority or funding fields. Treat that claim as unverified.
- **No primary text was found for these:** a 2025–26 SCMS Manager notice, a CAMP PQC or
  misbehaviour project, a C-Roads/CCMS open-issues list newer than ETSI TR 103 460 (2020), or the
  text of an NSF SaTC/CPS call.
- **The ITS JPO plan pages return 404** while the site is redesigned. The plan targets below come
  from secondary summaries: Covington (https://www.covingtonblogs.com/2024/08/30/usdot-releases-plan-to-accelerate-v2x-deployment/)
  and a USDOT deck (https://its.dot.gov/research_areas/emerging_tech/pdf/V2X_Summit_presentation.pdf).

## Key primary sources

| Tag | Source |
|---|---|
| PQC-TR | 5GAA + GSMA, *Post-Quantum Cryptography for Automotive: Impact Assessment*, 6 Oct 2026, §5.2 and §6 (https://5gaa.org/post-quantum-cryptography-for-automotive-impact-assessment/) |
| MBD-Op | 5GAA, *Misbehaviour Detection for V2X: Operational Aspects*, v1.0, 29 May 2024, §1, §5 and §8 (https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf) |
| CP-TR | 5GAA, *Credential Management Supporting V2X Commercial Deployments* (SCMS Certificate Policy), 14 Jul 2025 (https://5gaa.org/content/uploads/2025/07/credential-management-supporting-v2x-commercial-deployments.pdf) |
| NIST | NIST IR 8547 ipd, 12 Nov 2024, still a draft. Per secondary sources, it deprecates ECDSA after 2030 and disallows it after 2035 (https://csrc.nist.gov/pubs/ir/8547/ipd) |
| FCC | FCC 24-123, Second Report and Order on C-V2X. It applies to the upper 30 MHz, sunsets DSRC two years after Federal Register publication, and is effective 11 Feb 2025 (https://docs.fcc.gov/public/attachments/FCC-24-123A1_Rcd.pdf) |
| Plan | USDOT National V2X Deployment Plan, Aug 2024: V2X on 20% of the NHS and in 25% of top-75-metro signalised intersections by 2028, 85% of intersections by 2036, and interoperable SCMS providers (secondary summaries above). Its supplement: https://rosap.ntl.bts.gov/view/dot/79091/dot_79091_DS1.pdf |
| J3287 | SAE J3287_202408, *V2X Misbehavior Reporting*, issued 30 Aug 2024 (doi:10.4271/J3287_202408) |
| CVP | USDOT CV Pilot lessons: NYC raised pseudonym certificates from 20 to 60 per week for taxis (https://www.itskrs.its.dot.gov/2019-l00889) |
| TraCR | USDOT UTC, 2025–26 call. Its thrusts are Security & Resiliency, Privacy, Society & Economy, and Quantum Threats, and it favours testbeds and pilots (https://www.clemson.edu/cecas/tracr/research/projects/) |

## Top 10 needs, ranked by documented urgency × size of the evidence gap × fit to this engine

**1. Migrating broadcast V2X to PQC without breaking the channel.**
- **Source:**
  - PQC-TR §5.2: larger keys and signatures "reduce spectral efficiency on PC5 links", "more frequent collisions", and "re-optimisation of scheduling and resource allocation". Devices last 10–15 years, so hybrid modes are needed for legacy devices.
  - PQC-TR §6: deployments must migrate by about 2030 for high-priority systems and 2035 for all. IEEE 1609 "is starting" PQC extensions to 1609.2. ETSI TC ITS is adding a cost-benefit analysis of PQC mitigations to its TVRA. C2C-CC is writing a migration white paper.
  - NIST sets the same ECDSA end dates.
- **Missing:** system-level numbers. Nobody has measured PQC and hybrid certificates through the full SCMS: certificate-versus-digest choice, P2PCD, pseudonym top-ups, CRL size, and a mixed legacy fleet, all at city density. The closest work is arXiv:2608.05087 (Aug 2026). It modelled one RSU receiver with signature sizes only. Falcon-512 met 90% PDR only at LOS-A, and nothing met it under NLOS. Funded competition: TRB RiP 2531081, *Towards Deployment-Ready PQC-Enabled V2X* (https://rip.trb.org/View/2531081).
- **Can the simulator supply it?** Yes, strongly. The engine already has PQC/hybrid signatures, real 1609.2/SCMS and matched LTE/NR PHY/MAC. This directly feeds IEEE 1609, the ETSI TVRA and the 5GAA roadmap.

**2. Rules for misbehaviour remediation: when to revoke, suspend or do nothing.**
- **Source:**
  - MBD-Op §1: deployed systems "have not yet specified the conditions to determine when a particular remediation technique is to be used", which risks "outcomes that are perceived as unfair".
  - MBD-Op §5: a full cost-benefit analysis "may be resource-intensive and unfeasible", so the paper relies on expert judgement.
  - MBD-Op §8 lists the open items: a draft policy specification, reinstatement, suspension without reports, new MBR fields (impact, footprint, duration) and new revocation mechanisms.
- **Missing:** the quantified cost-benefit that §5 calls unfeasible. For each class (easily detectable, mild, temporary), what are the damage and the CRL, verifier and reinstatement costs of each remediation tier?
- **Can the simulator supply it?** Yes, and this is the clearest "they said it can't be done" hook. It needs the closed-loop driver response (a current engine limit) to price the damage.

**3. Trust across SCMS providers: CTL, electors and cooperation between misbehaviour authorities (MAs).**
- **Source:**
  - CP-TR: "no commonly accepted Certificate Policy in North America", which "hinders interoperability and shared trust". Its draft CP says an MA "should cooperate with all SCMS Providers" on the CTL and "should support end entity revocation requests from other MAs".
  - Plan: two or more SCMS providers must demonstrate interoperable security.
- **Missing:** any failure-mode evidence for multi-provider operation. Examples are elector or root rollover, how fast CTLs and CRLs reach devices, and an MA that cannot link certificates from another provider.
- **Can the simulator supply it?** Yes. The engine is being built with a multi-provider CTL, five electors and the real trust lists, so it is near-unique here.

**4. How fast revocation reaches devices, and what it costs.**
- **Source:** MBD-Op §5.4 lists, qualitatively, the "incremental cost of generating and distributing the added CRL entries" and "of verifying the added CRL entries for all V2X recipients". It also states that the misbehaviour detection and remediation (MBDR) cycle "can take a week".
- **Missing:** measured exposure windows and costs across CRL cadence, RSU relay and connectivity.
- **Can the simulator supply it?** Yes. REDPKI work on the revocation window already exists in the repo.

**5. Airtime under 30 MHz after the FCC rule.**
- **Source:** FCC limits C-V2X to 30 MHz and sets the DSRC sunset. PQC-TR §5.2 describes the channel-load effect. Plan supplement: trust "requires support from ... security and privacy measures".
- **Missing:** how much of the channel security takes at Plan-scale densities: P2PCD learning, full certificates versus digests, misbehaviour reports, CRL broadcast.
- **Can the simulator supply it?** Yes, provided the LTE Mode 4 SPS model holds at high density.

**6. Pseudonym certificate supply, storage and connectivity.**
- **Source:**
  - CVP: 20 certificates per week was too few for taxis, so NYC raised it to 60.
  - USDOT briefing: preload three years of certificates or monitor them weekly (https://www.itskrs.its.dot.gov/sites/default/files/2025-05/executive-briefing/Executive%20Briefing%202024_V2X%20Technology_FINAL508_05302025.pdf).
- **Missing:** the trade-off between privacy, storage, backend load and outages for each fleet type.
- **Can the simulator supply it?** Yes. The engine uses the real calendar and 1609.2.1 top-ups.

**7. Required detector performance and report volume.**
- **Source:**
  - MBD-Op §5.2.1: minimum performance requirements (MPR) for local detection "may need to be considered", because device capability "can vary widely".
  - J3287 standardises the report format but not detector quality.
  - 5GAA MBD4V2X remains active for standards gaps.
- **Missing:** the system-level detection quality, and the MA intake load, at which safety is preserved.
- **Can the simulator supply it?** Partly. It needs warnings that change driving, and per-event RNG.

**8. Cost and scale of the national rollout.**
- **Source:**
  - Plan: deployment costs $7k–15k per intersection, which includes installing security credentials.
  - Georgia DOT estimated $240k of SCMS for 1,700 intersections (same USDOT briefing as need 6).
- **Missing:** how backend load (RA, PCA and MA throughput, DCM) grows with the Plan's 2028–2036 targets.
- **Can the simulator supply it?** Partly. It gives load, not dollars, and is slow: about 2.4 h per 600 s run.

**9. Privacy and linkability of the deployed stack.**
- **Source:** the Plan supplement requires that connectivity "protect personal information". TraCR Thrust 2 covers privacy. The PQC-TR taxonomy lists BSM "ID change" and "anti-tracking" (§5.2.2).
- **Missing:** cross-layer measurements of linkability on the real US stack.
- **Can the simulator supply it?** Yes. This is the earlier Approach 3.

**10. Liability, fairness and governance of revocation decisions.**
- **Source:** MBD-Op §1 on outcomes "perceived as unfair". MBD4V2X lists "legal aspects".
- **Missing:** legal and institutional decisions.
- **Can the simulator supply it?** No, except as a source of ground-truth forensics on wrongful revocation. That would join this need with need 2 and the "which evidence can frame you" question.

## Reading of the gap

- **Needs 1–4 are one opportunity.** The bodies writing the rules are 5GAA, IEEE 1609, ETSI TC ITS, the SCMS CP authors and USDOT. Each says, in writing, that it lacks system-level numbers. None has an end-to-end testbed that joins traffic, radio and a real multi-provider SCMS.
- **The literature confirms it.** By catalog keyword search, only 27 works address MA or remediation policy. The most cited is brecht2018security (325 citations). V2X PQC work is dominated by surveys and protocol proposals (shim2022survey, 66 citations). No catalog work measures multi-provider CTL operation.
- **The biggest and fastest-adopted contribution** would be a public reference evaluation of PQC/hybrid migration on the full US SCMS stack, published with the remediation cost model from need 2. Its likely audience is IEEE 1609, ETSI and the C2C-CC/5GAA migration work.
- **Main risks:**
  - Arrival timing: the Clemson/TraCR group (arXiv:2608.05087, TRB RiP 2531081) is already on PQC-V2X.
  - Radio-model credibility: reviewers will compare the PHY/MAC against OpenCV2X.
  - Run time.
