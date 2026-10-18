# Event-Driven Safe and Resilient Control of Automated and Human-Driven Vehicles under EU-FDI Attacks
Yi Zhang, Yichao Wang, Wei Xiao, Mohamadamin Rajabinezhad, Shan Zuo · arXiv 2026 · https://arxiv.org/abs/2603.02526 · relevance: medium · basis: abstract

**Problem:** This paper studies the safe and resilient control of Connected and Automated Vehicles (CAVs) operating in mixed traffic environments where they must interact with Human-Driven Vehicles (HDVs) under uncertain dynamics and exponentially unbounded false data injection (EU-FDI) attacks.
**Approach:** These attacks pose serious threats to safety-critical applications. While resilient control strategies can mitigate adversarial effects, they often overlook collision avoidance requirements.
**Tools and scale:** Not stated in the abstract.
**Results:** The event-driven approach reduces computational load while maintaining real-time safety guarantees. Simulation results, including comparisons with conventional safety-critical control methods that lack resilience, validate the effectiveness and robustness of the proposed EDSR framework in achieving collision-free maneuvers, stable velocity regulation, and resilient operation under adversarial conditions.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
