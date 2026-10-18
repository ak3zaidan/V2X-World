# Resilient Neural-Variable-Structure Consensus Control for Nonlinear MASs with Singular Input Gain Under DoS Attacks
Ladan Khoshnevisan, Xinzhi Liu · arXiv 2025 · https://arxiv.org/abs/2512.09879 · relevance: medium · basis: abstract

**Problem:** This paper proposes a reliable learning-based adaptive control framework for nonlinear multi-agent systems (MASs) subject to Denial-of-Service (DoS) attacks and singular control gains, two critical challenges in cyber-physical systems.
**Approach:** A neural-variable-structure adaptive controller is developed to achieve leader-follower consensus while ensuring robustness to external disturbances and adaptability to unknown nonlinear dynamics. A reliability-assessment rule is introduced to detect communication loss during DoS attacks, upon which a switched control mechanism is activated to preserve closed-loop stability and performance.
**Tools and scale:** Lyapunov-based analysis establishes uniform ultimate boundedness of all closed-loop signals, and Matlab/Simulink simulations on a connected automated vehicle platoon demonstrate the method's effectiveness and resilience.
**Results:** Lyapunov-based analysis establishes uniform ultimate boundedness of all closed-loop signals, and Matlab/Simulink simulations on a connected automated vehicle platoon demonstrate the method's effectiveness and resilience.
**Limitations and future work (as stated by the authors):**
- "This paper proposes a reliable learning-based adaptive control framework for nonlinear multi-agent systems (MASs) subject to Denial-of-Service (DoS) attacks and singular control gains, two critical challenges in cyber-physical systems."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
