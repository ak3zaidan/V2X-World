# MCCS: PRIVACY-ENHANCED AND ADVERSARIAL-RESILIENT MULTI-CRITERIA CLIENT SELECTION FOR FEDERATED LEARNING IN IOV
Jadil Alsamiri¹, Khalid Alsubhi², Jadil Alsamiri · International Journal of Applied Mathematics 2025 · https://doi.org/10.12732/ijam.v38i8s.1013 · relevance: medium · basis: abstract

**Problem:** The Internet of Vehicles (IoV) generates extensive data for machine learning applications such as intrusion detection; however, centralized solutions often create privacy and security risks.
**Approach:** This study, therefore, proposes a Multi-Criteria Client Selection (MCCS) framework that selects clients based on trust, connectivity, data diversity, and adversarial behavior for secure and robust FL operation.
**Tools and scale:** Evaluated with simulations over the Car Hacking dataset, MCCS demonstrated strong results with F1 scores of 0.7590 to 0.8678, a maximum accuracy of 0.9775, convergence between 510 and 1459 seconds, and memory use of 1,964 to 3,137 MB, outperforming baselines such as FedPROM and RICA+CKA.
**Results:** Key innovations are a dynamic trust-score mechanism with cosine similarity against adversaries, a combination of data diversity with improved generalization, differential privacy for a balance between privacy and performance, and scalability for constrained devices. Evaluated with simulations over the Car Hacking dataset, MCCS demonstrated strong results with F1 scores of 0.7590 to 0.8678, a maximum accuracy of 0.9775, convergence between 510 and 1459 seconds, and memory use of 1,964 to 3,137 MB
**Limitations and future work (as stated by the authors):**
- "Federated Learning (FL) enables decentralized training while ensuring that data remain local."
- "However, in IoV contexts, it is challenged by client heterogeneity, non-independent and identically distributed data, and adversarial attacks."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area); pseudonym privacy/linkability (runner-up area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
