# Détection d'anomalies comportementales pour les systèmes de transport intelligents et coopératifs (Misbehavior detection for C-ITS)
Joseph Kamel · PhD thesis, Institut Polytechnique de Paris / HAL 2020 · https://hal.science/tel-03102396v1 · relevance: high · basis: full text

**Problem:** Detecting misbehaviour in C-ITS end to end, from local plausibility checks and fusion through reporting to a global Misbehaviour Authority (MA).
**Approach:**
- F2MD, an extension of VEINS.
- CaTch confidence-range-tolerant checks, which take the sender's confidence fields into account.
- A comparison of local fusion apps.
- The VeReMi Extension dataset with sensor-error models and faulty behaviours.
- A misbehaviour-report format with evidence containers.
- An ML MA that classifies reported stations as "Misbehaving ..., Faulty ..., Genuine (false positive)" (§6.1, PDF p.114).
- A Sybil-aware global MA with an AI linkage mechanism (§6.2).
**Tools and scale:** F2MD/VEINS/SUMO on 24 h LuST (Luxembourg) for training and Paris-Saclay for testing. ETSI CAM/IEEE BSM-style beacons. No US SCMS, no LA linkage revocation, no CRL.
**Results:**
- CaTch inside an MLP fusion app: precision +21.0%, recall −3.8% (§4.1.3.3, PDF p.84).
- MA multi-class XGBoost: macro F1 up to 0.98 (Tables 6.2-6.5).
- Fig. 3.12 (PDF pp.71-72) counts reports per pseudonym **about genuine and misbehaving nodes** received by the MA under different pseudonym-change periods. The "simple MA is still based only on the number of received reports".
- Sybil attacks make local vehicles "simultaneously report other genuine vehicles" (PDF p.129).
**Limitations and future work (as stated by the authors):**
- Ignoring confidence "may result in a high number of false positives in a realistic scenario" (PDF p.19).
- "the global MA should be designed to withstand a number of False Positive reports ... The effect of the local detection quality on the global MA should also be evaluated" (PDF p.92).
- Count-based MAs become "obsolete" with evidence-based reporting, so an "intelligent MA" is needed (PDF p.72).
**Overlap with our engine:** strong. In-simulation detection, reporting and an MA at city scale, including reports about genuine vehicles.
**Novelty implications:** Rules out "first city-scale count of false-positive reports at an MA", "first genuine/faulty/malicious MA classification" and "first confidence-aware checks". Leaves open the US SM-MBR rule set, linkage-based revocation under the US SCMS, and wrongful-revocation counts. See `synthesis/check-paper1-preemption.md`.
