# NetDiffusion: Network Data Augmentation Through Protocol-Constrained Traffic Generation
Xi Jiang, Shi-Nan Liu, Aaron Gember-Jacobson, A. Bhagoji et al. · Proceedings of the ACM on Measurement and Analysis of Computing Systems 2023 · https://doi.org/10.1145/3639037 · relevance: medium · basis: abstract

**Problem:** Datasets of labeled network traces are essential for a multitude of machine learning (ML) tasks in networking, yet their availability is hindered by privacy and maintenance concerns, such as data staleness. To overcome this limitation, synthetic network traces can often augment existing datasets.
**Approach:** In this paper, we apply diffusion models to generate high-resolution synthetic network traffic traces. We present NetDiffusion1, a tool that uses a finely-tuned, controlled variant of a Stable Diffusion model to generate synthetic network traffic that is high fidelity and conforms to protocol specifications.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** Our evaluation demonstrates that packet captures generated from NetDiffusion can achieve higher statistical similarity to real data and improved ML model performance than current state-of-the-art approaches (e.g., GAN-based approaches).
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (learned surrogate / performance model of a network); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
