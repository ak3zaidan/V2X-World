# An Edge-Assisted Private Set Intersection Scheme for Privacy-Preserving Vehicular Crowdsensing
Xin-Rui Zhang, Pin-Can Zhao, Rong-Xing Lu, Suprio Ray · IEEE Internet of Things Journal 2026 · https://doi.org/10.1109/jiot.2026.3675232 · relevance: medium · basis: abstract

**Problem:** Vehicular crowdsensing enables connected and autonomous vehicles (CAVs) to jointly contribute driving-related data to support applications such as traffic management and accident analysis. Ensuring the reliability of such data requires identifying observations that have been corroborated by multiple vehicles.
**Approach:** To address this privacy challenge, we introduce edge-assisted private set intersection (EA-PSI), a scheme that enables secure computation of the intersection among CAV observation sets without disclosing individual data elements. To support necessary polynomial operations over secret-shared data, we develop a secure multiplication mechanism based on Beaver triples, enabling the RSU and vehicles to jointly compute ...
**Tools and scale:** We analyze the security of EA-PSI under the simulation-based paradigm and formally prove privacy against semihonest adversaries.
**Results:** However, achieving this corroboration typically necessitates comparing each vehicle’s private set of observations, which can inadvertently reveal sensitive trajectory information. The RSU then performs intersection computation on the collected shares using randomized encoding techniques, without learning any private observations or intersection results.
**Limitations and future work (as stated by the authors):**
- To address this privacy challenge, we introduce edge-assisted private set intersection (EA-PSI), a scheme that enables secure computation of the intersection among CAV observation sets without disclosing individual data elements.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
