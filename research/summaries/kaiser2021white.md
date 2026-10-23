# White Paper on Misbehaviour Detection and Reporting to Misbehaviour Authority
CAR 2 CAR Communication Consortium (A. Kaiser et al.) · C2C-CC WP 2091/2092 v1.0, 2021-12-17 · https://www.car-2-car.org/fileadmin/documents/General_Documents/C2CCC_WP_2092_MisbehaviourDetection_and_Reporting_V1.0.pdf · relevance: high · basis: full text (detection, fusion and implementation-results sections read)

**Problem:** Survey R&D projects and test beds on on-board MBD and reporting to an MA for Day-1 C-ITS. Propose fixed plausibility thresholds for CAMs and DENM checks.
**Approach:**
- Legacy and CaTch-style (confidence-range) checks.
- Fusion apps: threshold, aggregation and node-trust.
- C2C-CC-agreed fixed thresholds for passenger-car CAM fields, e.g. yaw rate > 1.5 rad/s and curvature radius < 3.9 m (Table 4).
- DENM checks from draft ETSI TS 103 759.
**Tools and scale:** A literature and project survey. Reuses F2MD-based comparisons. No new measurements.
**Results:** Threshold fusion is comparable to the ML fusion apps.
**Limitations and future work (as stated by the authors):**
- "the global MA should be designed to withstand a number of False Positive reports".
- "testing of advanced misbehaviour detection applications in large-scale deployment projects or field-tests (FOTs) has started, but currently there is no sufficient evaluation results" (p.23).
- RSU CAMs are excluded.
**Overlap with our engine:** partial. The EU counterpart of the US MBR rule set.
**Novelty implications:** Confirms that, as of 2021, no field evaluation of standard check sets at the MA level had been published. EU-side context for Paper 1, not pre-emption.
