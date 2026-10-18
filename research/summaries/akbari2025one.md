# One task to rule them all: A closer look at traffic classification generalizability
Elham Akbari, Zihao Zhou, M. A. Salahuddin, N. Limam, R. Boutaba, Bertrand Mathieu, S. Moteau, Stéphane Tuffin · arXiv.org 2025 · https://arxiv.org/abs/2507.06430 · relevance: low · basis: abstract

**Problem:** Existing website fingerprinting and traffic classification solutions do not work well when the evaluation context changes, as their performances often heavily rely on context-specific assumptions.
**Approach:** To clarify this problem, we take three prior solutions presented for different but similar traffic classification and website fingerprinting tasks, and apply each solution's model to another solution's dataset. We pinpoint dataset-specific and model-specific properties that lead each of them to overperform in their specific evaluation context.
**Tools and scale:** Mentioned in abstract: dataset, simulates.
**Results:** We show that, even when abundant labeled data is available, the best solutions'performances under distribution shift are between 30% and 40%, and a simple 1-Nearest Neighbor classifier's performance is not far behind.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
