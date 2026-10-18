# WiMamba: Linear-Scale Wireless Foundation Model
Tomer Raviv, Nir Shlezinger · arXiv 2026 · https://arxiv.org/abs/2603.26367 · relevance: medium · basis: abstract

**Problem:** Foundation models learn transferable representations, motivating growing interest in their application to wireless systems. Existing wireless foundation models are predominantly based on transformer architectures, whose quadratic computational and memory complexity can hinder practical deployment for large-scale channels.
**Approach:** In this work, we introduce WiMamba, a wireless foundation model built upon the recently proposed Mamba architecture, which replaces attention mechanisms with selective state-space models and enables linear-time sequence modeling. We further develop a dedicated task-agnostic, self-supervised pre-training framework tailored to wireless channels, resulting in a genuine foundation model that learns transferable channel representations.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** Leveraging this architectural advantage combined with adaptive preprocessing, WiMamba achieves scalable and low-latency inference while maintaining strong representational expressivity. Evaluations across four downstream tasks demonstrate that WiMamba matches or outperforms transformer-based wireless foundation models, while offering dramatic latency and memory reductions.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
