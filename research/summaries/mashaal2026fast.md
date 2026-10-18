# Fast Wireless Foundation Models with Early-Exits
Omar Mashaal, Hatem Abou-Zeid · arXiv 2026 · https://arxiv.org/abs/2606.29640 · relevance: medium · basis: abstract

**Problem:** While wireless foundation models (FMs) are demonstrating strong potential to enable AI-Native 6G networks, their high computational cost remains a critical barrier to deployment. The large computational cost stems from the rigid, full-depth execution of the FM backbone for every task, a process we show is not only inefficient but can also degrade ...
**Approach:** In this paper, we propose a novel early-exit FM framework that attaches lightweight, per-task heads, at the most appropriate exit-stage of a frozen wireless FM encoder, enabling variable-depth inference tailored to each task's preferred representation depth.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results demonstrate that these intermediate-layer features not only speed-up inference significantly (up to 93% fewer FLOPs), but also provide more transferable representations that exceed the full encoder accuracy on unseen tasks. We further demonstrate that a simple fixed-exit strategy per task is more effective than traditional early-exiting policies that route different samples to different exits based on their perceived difficulty levels.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
