# Robustifying 3D Perception via Least-Squares Graphs for Multi-Agent Object Tracking
Maria Damanaki, Ioulia Kapsali, Nikos Piperigkos, Alexandros Gkillas, Aris S. Lalos · arXiv 2025 · https://arxiv.org/abs/2507.04762 · relevance: medium · basis: abstract

**Problem:** The critical perception capabilities of EdgeAI systems, such as autonomous vehicles, are required to be resilient against adversarial threats, by enabling accurate identification and localization of multiple objects in the scene over time, mitigating their impact.
**Approach:** Single-agent tracking offers resilience to adversarial attacks but lacks situational awareness, underscoring the need for multi-agent cooperation to enhance context understanding and robustness. This paper proposes a novel mitigation framework on 3D LiDAR scene against adversarial noise by tracking objects based on least-squares graph on multi-agent adversarial bounding boxes.
**Tools and scale:** Mentions: dataset, real-world; scale not stated in the abstract.
**Results:** Specifically, we employ the least-squares graph tool to reduce the induced positional error of each detection's centroid utilizing overlapped bounding boxes on a fully connected graph via differential coordinates and anchor points. An extensive evaluation study on the real-world V2V4Real dataset demonstrates that the proposed method significantly outperforms both state-of-the-art single and multi-agent tracking frameworks by up to 23.3% under challenging adversarial conditions, operating as a resilient approach without relying on additional defense mechanisms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
