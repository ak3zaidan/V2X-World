# Plug-and-Play Reweighting for Resilient Collaborative Decision-Making in Connected Autonomous Driving
Jiewen Liu, Rui Liu, Matthew Lee, Ming C. Lin et al. · arXiv 2026 · https://arxiv.org/abs/2607.10037 · relevance: low · basis: abstract

**Problem:** Collaborative decision-making is a fundamental capability in multi-robot systems, such as connected autonomous vehicles.
**Approach:** In this paper, we propose a novel Resilient Collaborative Decision-Making (RCDM) framework that consists of an attention-based encoder for extracting individual robot perceptual embeddings and an attention-based decoder for fusing collaborator perceptions and making decisions. To improve resilience to corrupted observations, we design a novel plug-and-play reweighting module that down-weights the influence of corrupted inputs by analyzing the consistency of neighborhood points relative to the local structure and assigning smaller weights to points that deviate strongly from the local median.
**Tools and scale:** Mentions: simulation (details not in abstract).
**Results:** Overall, existing methods typically rely on retraining with attack-specific defenses or on restrictive perturbation assumptions to improve resilience, which limits their practicality. Experimental results demonstrate that our approach consistently outperforms existing methods by up to 26% and achieves state-of-the-art resilient performance.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
