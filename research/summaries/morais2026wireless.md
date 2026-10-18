# Wireless Dataset Similarity: Measuring Distances in Supervised and Unsupervised Machine Learning
João Morais, Sadjad Alikhani, Akshay Malhotra et al. · arXiv.org 2026 · https://arxiv.org/abs/2601.01023 · relevance: low · basis: abstract

**Problem:** This paper introduces a task- and model-aware framework for measuring similarity between wireless datasets, enabling applications such as dataset selection/augmentation, simulation-to-real (sim2real) comparison, task-specific synthetic data generation, and informing decisions on model training/adaptation to new deployments.
**Approach:** We evaluate candidate dataset distance metrics by how well they predict cross-dataset transferability: if two datasets have a small distance, a model trained on one should perform well on the other. We apply the framework on an unsupervised task, channel state information (CSI) compression, using autoencoders.
**Tools and scale:** Not stated in the abstract.
**Results:** Using metrics based on UMAP embeddings, combined with Wasserstein and Euclidean distances, we achieve Pearson correlations exceeding 0.85 between dataset distances and train-on-one/test-on-another task performance. Across both tasks, the resulting distances outperform traditional baselines and consistently exhibit stronger correlations with model transferability, supporting task-relevant comparisons between wireless datasets.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
