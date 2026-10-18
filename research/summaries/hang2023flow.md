# Flow-MAE: Leveraging Masked AutoEncoder for Accurate, Efficient and Robust Malicious Traffic Classification
Zijun Hang, Yu-Liang Lu, Yongjie Wang, Yi Xie · International Symposium on Recent Advances in Intrusion Detection 2023 · https://doi.org/10.1145/3607199.3607206 · relevance: medium · basis: abstract

**Problem:** Malicious traffic classification is crucial for Intrusion Detection Systems (IDS). However, traditional Machine Learning approaches necessitate expert knowledge and a significant amount of well-labeled data.
**Approach:** To address these challenges, this paper presents Flow-MAE, a pre-training model that employs Masked AutoEncoders (MAE) from the Computer Vision domain to achieve accurate, efficient, and robust malicious network traffic classification.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** Experimental results from six datasets reveal that Flow-MAE achieves new state-of-the-art accuracy (>0.99), efficiency (>900 samples/s), and robustness across diverse network traffic types. In comparison to the state-of-the-art ET-BERT, Flow-MAE exhibits improvements in accuracy and speed by 0.41%-1.93% and 7.8x-10.3x, respectively, while necessitating only 0.2% FLOPs and 44% memory overhead.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for network traffic); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
