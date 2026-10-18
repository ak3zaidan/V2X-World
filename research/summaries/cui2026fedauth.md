# FedAuth: A Lightweight and Privacy-Enhanced Authentication Scheme for Federated Learning in IoVs
Kaiping Cui, Xia Feng, Chun Wang, Yuan-Bo Guo · IEEE Internet of Things Journal 2026 · https://doi.org/10.1109/jiot.2026.3653998 · relevance: medium · basis: abstract

**Problem:** In the Internet of Vehicles (IoV), certificateless-based authentication has emerged as a promising approach to secure federated learning (FL) participation. However, existing certificateless-based authentication schemes still exhibit three notable limitations: 1) some schemes cannot provide security against public-key replacement (PKR) attacks and key-leakage threats; 2) most existing schemes do not take realistic ...
**Approach:** To address these challenges, we propose FedAuth, a certificateless, lightweight, and privacy-enhanced authentication scheme tailored for FL in IoVs.
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** Second, FedAuth guarantees the authenticity and integrity of gradients through lightweight signature construction, which supports batch authentication while avoiding expensive operations, such as bilinear pairings, thereby reducing latency. The experimental results confirm that FedAuth significantly reduces computational overhead by at least 43.42% and saves not less than 19.8 kB of communication cost per 100 ...
**Limitations and future work (as stated by the authors):**
- However, existing certificateless-based authentication schemes still exhibit three notable limitations: 1) some schemes cannot provide security against public-key replacement (PKR) attacks and key-leakage threats; 2) most existing schemes do not take realistic coalition attacks into account; and 3) ...
- These limitations make it challenging for certificateless authentication frameworks to simultaneously ensure high security and low latency, particularly in resource-constrained FL deployments within IoV environments.
- To address these challenges, we propose FedAuth, a certificateless, lightweight, and privacy-enhanced authentication scheme tailored for FL in IoVs.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
