# ET-BERT: A Contextualized Datagram Representation with Pre-training Transformers for Encrypted Traffic Classification
Xinjie Lin, Gang Xiong, Gaopeng Gou, Zhen Li et al. · Proceedings of the ACM Web Conference 2022 2022 · https://doi.org/10.1145/3485447.3512217 · relevance: medium · basis: abstract

**Problem:** Encrypted traffic classification requires discriminative and robust traffic representation captured from content-invisible and imbalanced traffic data for accurate classification, which is challenging but indispensable to achieve network security and network management. The major limitation of existing solutions is that they highly rely on the deep features, which are overly dependent on data size and hard to generalize on unseen data.
**Approach:** In this paper, we propose a new traffic representation model called Encrypted Traffic Bidirectional Encoder Representations from Transformer (ET-BERT), which pre-trains deep contextualized datagram-level representation from large-scale unlabeled data. Notably, we provide explanation of the empirically powerful pre-training model by analyzing the randomness of ciphers.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** The pre-trained model can be fine-tuned on a small number of task-specific labeled data and achieves state-of-the-art performance across five encrypted traffic classification tasks, remarkably pushing the F1 of ISCX-VPN-Service to 98.9% (5.2%↑), Cross-Platform (Android) to 92.5% (5.4%↑), CSTNET-TLS 1.3 to 97.4% (10.0%↑).
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for network traffic); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
