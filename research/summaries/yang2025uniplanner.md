# UniPlanner: A Unified Motion Planning Framework for Autonomous Vehicle Decision-Making Systems via Multi-Dataset Integration
Xin Yang, Yuhang Zhang, Wei Li, Xin Lin, Wenbin Zou, Chen Xu · arXiv 2025 · https://arxiv.org/abs/2510.24166 · relevance: low · basis: abstract

**Problem:** Motion planning is a critical component of autonomous vehicle decision-making systems, directly determining trajectory safety and driving efficiency.
**Approach:** While deep learning approaches have advanced planning capabilities, existing methods remain confined to single-dataset training, limiting their robustness in planning. Through systematic analysis, we discover that vehicular trajectory distributions and history-future correlations demonstrate remarkable consistency across different datasets.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** Third, the Sparse-to-Dense (S2D) paradigm implements adaptive dropout to selectively suppress planning priors during training for robust learning, while enabling full prior utilization during inference to maximize planning performance.
**Limitations and future work (as stated by the authors):**
- While deep learning approaches have advanced planning capabilities, existing methods remain confined to single-dataset training, limiting their robustness in planning.
- Through systematic analysis, we discover that vehicular trajectory distributions and history-future correlations demonstrate remarkable consistency across different datasets.
- First, the History-Future Trajectory Dictionary Network (HFTDN) aggregates history-future trajectory pairs from multiple datasets, using historical trajectory similarity to retrieve relevant futures and generate cross-dataset planning guidance.
**Overlap with our engine:** none: adjacent topic in a vehicular context. Low relevance background; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
