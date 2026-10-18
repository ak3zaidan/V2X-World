# CENTILE: A Telemetry Foundation Model Evaluated by the Decisions It Drives
Zifan Zhang, Zhichao Hou, Tingxiang Ji, Yuchen Liu · arXiv 2026 · https://arxiv.org/abs/2608.01725 · relevance: low · basis: abstract

**Problem:** Modern computing and networking infrastructure emits telemetry continuously, yet operators convert it into decisions with a separate predictor per task, entity, and horizon.
**Approach:** One generative model, pretrained once over an operator's own event streams, could replace this fleet, an approach that already scales to high-cardinality streams in recommendation systems. However, point-forecast error on operational telemetry saturates near simple last-value baselines, so lower error alone need not improve the decisions it feeds.
**Tools and scale:** Not stated in the abstract.
**Results:** To our knowledge, \sys is the first pretrained telemetry model to improve both HPC scheduling and network provisioning decisions under replay, its runtime estimator transferring zero-shot across months and its pretrained weights across domains from hours of target data.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — adjacent background; no V2X security, PKI or simulator component.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background.
