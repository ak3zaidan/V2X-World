# Traffic-Aware Randomized Smoothing for LLM-Based Network Intrusion Detection
Zhenpeng Li · arXiv 2026 · https://arxiv.org/abs/2607.13801 · relevance: medium · basis: abstract

**Problem:** Large language model (LLM)-based intrusion detection systems (IDS) are increasingly studied for security monitoring, yet their robustness against feasible traffic manipulation remains largely empirical. We present Traffic-Aware Randomized Smoothing (TA-RS), a classifier-agnostic certified defense that injects Gaussian noise exclusively into the directly ...
**Approach:** We present Traffic-Aware Randomized Smoothing (TA-RS), a classifier-agnostic certified defense that injects Gaussian noise exclusively into the directly controllable (DC) subspace -- features a remote attacker can modify -- during both fine-tuning and certification, aligning the smoothing distribution with the attacker-controllable subspace.
**Tools and scale:** We identify a critical prerequisite: applying standard randomized smoothing to clean-trained LLM-IDS yields weak certified accuracy in three of four (model, dataset) pairs tested (14-33%, at or below random) and only 57% in the fourth (43 pp below the noise-augmented result); noise-augmented ...
**Results:** The larger gap -- up to 72 pp against an isotropic RS baseline that shares the DC-noise-augmented training recipe -- primarily reflects the training-certification mismatch rather than DC alignment alone: isotropic test-time noise perturbs uncontrollable features the attacker ... RT-IoT2022 probes the limits of the method: it fails under the default fine-tuning recipe but recovers to 76%/69% certified accuracy (LLaMA3-8B/Qwen3-8B) when noise augmentation is increased.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
