# AIRMap: AI-Generated Radio Maps for Wireless Digital Twins
Ali Saeizadeh, Miead Tehrani-Moayyed, Davide Villa et al. · arXiv 2025 · https://arxiv.org/abs/2511.05522 · relevance: low · basis: abstract

**Problem:** Accurate, low-latency channel modeling is essential for real-time wireless network simulation and digital-twin applications.
**Approach:** Traditional modeling methods like ray tracing are however computationally demanding and unsuited to model dynamic conditions. In this paper, we propose AIRMap, a deep-learning framework for ultra-fast radio-map estimation, along with an automated pipeline for creating the largest radio-map dataset to date.
**Tools and scale:** Not stated in the abstract.
**Results:** Trained on 1.2M Boston-area samples and validated across four distinct urban and rural environments with varying terrain and building density, AIRMap predicts path gain with under 4 dB RMSE in 4 ms per inference on an NVIDIA L40S-over 100x faster than GPU-accelerated ray tracing based radio maps. A lightweight calibration using just 20% of field measurements reduces the median error to approximately 5%, significantly outperforming traditional simulators, which exceed 50% error.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
