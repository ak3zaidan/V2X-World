# D2-V2X: Depth-Driven Cooperative V2X Reasoning for Autonomous Driving
K. Richard, Alphin Varghese, Colin Pham, David Oh, Srijan Das · arXiv.org 2026 · https://arxiv.org/abs/2605.24098 · relevance: low · basis: abstract

**Problem:** Single-vehicle Vision-Language Models (VLMs) are fundamentally constrained by sensor occlusions.
**Approach:** While Vehicle-to-Everything (V2X) systems mitigate this, current benchmarks lack the cooperative reasoning required for resolving ambiguities in complex environments. We introduce D2-V2X, a spatially-aware Question-Rationale-Answer (QRA) benchmark featuring 8,500 triplets derived from multimodal vehicle and infrastructure sensors.
**Tools and scale:** Not stated in the abstract.
**Results:** Our experiments demonstrate that grounding VLMs in cooperative LiDAR achieves 24.4% recall in identifying occluded hazards compared to near-zero in zero-shot models and reduces spatial estimation error for visible objects by 77% compared to the zero-shot baseline. While the model achieves a functional decision-making F1-score of 53.5, we identify 3D-to-2D projection as a fundamental bottleneck in current VLM architectures, establishing a new baseline for future innovation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches V2X, vehicular networking but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on V2X.
