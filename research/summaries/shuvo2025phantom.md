# PHANTOM: PHysical ANamorphic Threats Obstructing Connected Vehicle Mobility
Md Nahid Hasan Shuvo, Moinul Hossain · arXiv 2025 · https://arxiv.org/abs/2512.19711 · relevance: medium · basis: abstract

**Problem:** Connected autonomous vehicles (CAVs) rely on vision-based deep neural networks (DNNs) and low-latency (Vehicle-to-Everything) V2X communication to navigate safely and efficiently.
**Approach:** In this paper, we introduce PHANTOM (PHysical ANamorphic Threats Obstructing connected vehicle Mobility), a novel framework for crafting and deploying perspective-dependent adversarial examples using \textit{anamorphic art}.
**Tools and scale:** Comprehensive evaluation in CARLA across varying speeds, weather conditions, and lighting scenarios shows that PHANTOM achieves over 90\% attack success rate under optimal conditions and maintains 60-80\% effectiveness even in degraded environments. Beyond individual vehicle deception, PHANTOM triggers network-wide disruption in CAV systems: SUMO-OMNeT++ co-simulation demonstrates that false emerg
**Results:** Unlike conventional attacks, PHANTOM operates in black-box settings without model access and demonstrates strong transferability across four diverse detector architectures (YOLOv5, SSD, Faster R-CNN, and RetinaNet). Comprehensive evaluation in CARLA across varying speeds, weather conditions, and lighting scenarios shows that PHANTOM achieves over 90\% attack success rate under optimal conditions and maintains 60-80\% effectiveness even in degraded environments.
**Limitations and future work (as stated by the authors):**
- "Despite their advances, these systems remain vulnerable to physical adversarial attacks."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); security overhead or cryptography (direction 2 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
