# Language of Network: A Generative Pre-trained Model for Encrypted Traffic Comprehension
Di Zhao, Bo Jiang, Song Liu, Susu Cui, Meng Shen, Dongqi Han, Xingmao Guan, Zhigang Lu · arXiv 2025 · https://arxiv.org/abs/2505.19482 · relevance: low · basis: abstract

**Problem:** The increasing demand for privacy protection and security considerations leads to a significant rise in the proportion of encrypted network traffic.
**Approach:** Since traffic content becomes unrecognizable after encryption, accurate analysis is challenging, making it difficult to classify applications and detect attacks. Deep learning is currently the predominant approach for encrypted traffic classification through feature analysis.
**Tools and scale:** Mentioned in abstract: dataset.
**Results:** First, their performance is highly sensitive to data quality, where the highcost manual labeling process and dataset imbalance significantly degrade results. Since traditional tokenization methods are primarily designed for natural language, we propose a protocol-aware tokenization approach for encrypted traffic that improves model comprehension of fields specific to network traffic.
**Limitations and future work (as stated by the authors):**
- However, these methods face limitations due to their high dependence on labeled data and difficulties in detecting attack variants.
**Overlap with our engine:** partial — touches attack modelling but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on attack modelling.
