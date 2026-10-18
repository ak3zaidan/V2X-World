# Conceptualizing and Modeling Communication-Based Cyberattacks on Automated Vehicles
Tianyi Li, Tianyu Liu, Yicheng Yang · arXiv 2025 · https://arxiv.org/abs/2510.02364 · relevance: medium · basis: abstract

**Problem:** Adaptive Cruise Control (ACC) is rapidly proliferating across electric vehicles (EVs) and internal combustion engine (ICE) vehicles, enhancing traffic flow while simultaneously expanding the attack surface for communication-based cyberattacks.
**Approach:** Because the two powertrains translate control inputs into motion differently, their cyber-resilience remains unquantified.
**Tools and scale:** Therefore, we formalize six novel message-level attack vectors and implement them in a ring-road simulation that systematically varies the ACC market penetration rates (MPRs) and the spatial pattern of compromised vehicles. Across all simulation scenarios, EV platoons exhibit lower velocity standard deviation, reduced spacing oscillations, and faster post-attack recovery compared to ICE counterpar
**Results:** Across all simulation scenarios, EV platoons exhibit lower velocity standard deviation, reduced spacing oscillations, and faster post-attack recovery compared to ICE counterparts, revealing an inherent stability advantage.
**Limitations and future work (as stated by the authors):**
- "Because the two powertrains translate control inputs into motion differently, their cyber-resilience remains unquantified."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
