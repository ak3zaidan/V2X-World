# The Outline of Deception: Physical Adversarial Attacks on Traffic Signs Using Edge Patches
Haojie Ji, Te Hu, Haowen Li, Long Jin, Chongshi Xin, Yuchi Yao et al. · arXiv 2025 · https://arxiv.org/abs/2512.00765 · relevance: medium · basis: abstract

**Problem:** Intelligent driving systems are vulnerable to physical adversarial attacks on traffic signs.
**Approach:** These attacks can cause misclassification, leading to erroneous driving decisions that compromise road safety. Moreover, within V2X networks, such misinterpretations can propagate, inducing cascading failures that disrupt overall traffic flow and system stability.
**Tools and scale:** Mentions: real-world; scale not stated in the abstract.
**Results:** A U-Net generator is utilized to craft adversarial patches, which are then optimized through color and texture constraints along with frequency domain analysis to achieve seamless integration with the background environment, resulting in highly effective visual concealment. The proposed method demonstrates outstanding attack success rates across traffic sign classification models with varied architectures, achieving over 90% under limited query budgets.
**Limitations and future work (as stated by the authors):**
- However, a key limitation of current physical attacks is their lack of stealth.
**Overlap with our engine:** none — adjacent background; no integrated traffic/radio/SCMS simulation per the abstract.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract.
