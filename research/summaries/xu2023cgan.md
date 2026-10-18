# A CGAN-based Few-shot Method for Zero-day Attack Detection in the Internet of Vehicles
Bingfeng Xu, Bo Wang, Xinkai Chen, Jincheng Zhao, Gaofeng He · 2023 Eleventh International Conference on Advanced Cloud and Big Data (CBD) 2023 · https://doi.org/10.1109/cbd63341.2023.00026 · relevance: medium · basis: abstract

**Problem:** Due to the limited availability of labeled attack data, the detection of zero-day attacks in the Internet of Vehicles domain often relies on an anomaly-based approach.
**Approach:** However, this approach frequently leads to a high false positive rate. In practice, we have observed that the principles behind zero-day attacks and known attacks are similar within the Internet of Vehicles environment.
**Tools and scale:** Not stated in the abstract.
**Results:** With this framework, an adaptive sampling data augmentation method is designed to augment data with known attack samples by optimizing the input samples of this model to reduce the false positive rate. The experimental results demonstrate the superiority of the proposed method compared to existing approaches, both in terms of detection effectiveness and latency.
**Limitations and future work (as stated by the authors):**
- However, this approach frequently leads to a high false positive rate.
- Moreover, to alleviate the data imbalance problem caused by a few input attack samples of this model, a collaborative focus loss function is provided in the discriminators, which focuses on discerning challenging-to-classify data.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
