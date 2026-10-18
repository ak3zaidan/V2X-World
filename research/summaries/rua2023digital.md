# Digital Twin Technology Enabled Proactive Safety Application for Vulnerable Road Users: A Real-World Case Study
Erik Rua, Kazi Hasan Shakib, Sagar Dasgupta, Mizanur Rahman, Steven Jones · arXiv 2023 · https://arxiv.org/abs/2312.10041 · relevance: medium · basis: abstract

**Problem:** While measures, such as traffic calming and advance driver assistance systems, can improve safety for Vulnerable Road Users (VRUs), their effectiveness ultimately relies on the responsible behavior of drivers and pedestrians who must adhere to traffic rules or take appropriate actions.
**Approach:** However, these measures offer no solution in scenarios where a collision becomes imminent, leaving no time for warning or corrective actions. Recently, connected vehicle technology has introduced warning services that can alert drivers and VRUs about potential collisions.
**Tools and scale:** Mentions: real-world; scale not stated in the abstract.
**Results:** Three Encoder-Decoder LSTM models, namely pedestrian-LSTM, vehicle-through-LSTM, and vehicle-left-turn-LSTM, are trained and validated using field-collected data, achieving corresponding root mean square errors (RMSE) of 0.049, 1.175, and 0.355 meters, respectively. Experimental results confirm that DT-enabled safety alert systems were succesfully able to detect potential crashes and proactively generate safety alerts to reduce potential crash risk.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
