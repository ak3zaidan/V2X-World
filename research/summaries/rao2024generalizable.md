# Generalizable One-Way Delay Prediction Models for Heterogeneous UEs in 5G Networks
Akhila Rao, Hassam Riaz, Aleksandr Zavodovski, Rami Mochaourab et al. · NOMS 2024-2024 IEEE Network Operations and Management Symposium 2024 · https://doi.org/10.1109/noms59830.2024.10574985 · relevance: medium · basis: abstract

**Problem:** From a 5G operator’s perspective, accurate estimates of key User Equipments (UEs) performance metrics, especially One-Way Delay (OWD), can provide valuable information. These estimates can trigger management tasks such as reconfiguration to prevent violations of Service Level Objectives (SLOs).
**Approach:** We use advanced machine learning over data gathered at the base stations to predict OWD from UEs and show that we are able to predict OWD with over a 2× reduction in percentage error compared to the considered baseline. We discover the close coupling between the performance of the OWD model and the type of UE, which poses a model generalization challenge.
**Tools and scale:** Mentioned in abstract: 5G, testbed. (From the abstract only.)
**Results:** Addressing this problem, we demonstrate the shortcomings of the commonly used fine-tuning approach and develop a novel method based on domain adversarial neural networks, that can adapt to a target domain without compromising on the performance of the source domain. Our results show that we can adapt our source model to provide OWD prediction performance within 1-4 percentage points of the ideal scenario when the source and the target domains are the same. Also, our work is grounded in empirical experiments conducted within a 5G testbed, using commercially available hardware.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (learned surrogate / performance model of a network); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
