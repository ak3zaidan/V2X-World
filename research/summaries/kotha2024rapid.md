# RAPID-AV: Realtime Adaptive and Intelligent Detection For AV’s Against DDoS Attacks
Aditya Kotha, U. Venkanna · International Workshop on Ant Colony Optimization and Swarm Intelligence 2024 · https://doi.org/10.1109/ants63515.2024.10898213 · relevance: medium · basis: abstract

**Problem:** Autonomous vehicles (AVs) increasingly rely on vehicle-to-everything (V2X) networks for communication.
**Approach:** To solve these problems, this paper proposes RAPID-AV, an ML-based approach that uses programmable P4 switches for real-time detection and mitigation of DDoS attacks in AV networks. The proposed system uses a threefold strategy: collect traffic statistics from AV, use P4-based programmable packet processing for anomaly detection, and use majority voting to mitigate attacks.
**Tools and scale:** Not stated in the abstract.
**Results:** In our experiments, we found that the decision tree algorithm achieves an accuracy of 94.33% in distinguishing traffic from DDoS traffic. In addition, RAPID-AV demonstrated a nearly 10% reduction in CPU load during attacks and maintained consistent communication between AV and RSU.
**Limitations and future work (as stated by the authors):**
- "This approach can help improve AV network security while addressing the limitations of existing solutions."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
