# BTRFormer: Hierarchical Learning of Encrypted Traffic Using a Masked Autoencoder with Block-Based Traffic Representation
Junnan Yin, Lei Cui, Zhi-Yu Hao, Jiawei Sun, Peng Liu, Xiao-Chun Yun · IEEE International Conference on Network Protocols 2025 · https://doi.org/10.1109/icnp65844.2025.11192383 · relevance: low · basis: abstract

**Problem:** Encrypted traffic classification (ETC) is essential for ensuring network security and efficient management.
**Approach:** Despite advances in deep learning, ETC remains challenging as existing models struggle to learn robust, discriminative representations from content-encrypted, highly imbalanced traffic.To address these challenges, we propose BTRFormer, a novel ETC approach that capitalizes on the inherent properties of encryption algorithms to enhance classification accuracy. At the core of BTRFormer lies a block-based, multi-layer traffic representation that adopts a 4×4 block as the fundamental unit, inspired by the encryption algorithm’s use of 16-byte blocks for encryption operations.
**Tools and scale:** Not stated in the abstract.
**Results:** Finally, BTRFormer leverages a pre-training phase on large-scale unlabeled data, followed by fine-tuning with a minimal amount of labeled samples to improve generalization and adaptability. Experimental results show that BTRFormer significantly outperforms SOTA methods on six real-world datasets, highlighting its effectiveness in encrypted traffic classification and secure network management.
**Limitations and future work (as stated by the authors):**
- Despite advances in deep learning, ETC remains challenging as existing models struggle to learn robust, discriminative representations from content-encrypted, highly imbalanced traffic.To address these challenges, we propose BTRFormer, a novel ETC approach that capitalizes on the inherent properties of encryption algorithms to enhance classification accuracy.
**Overlap with our engine:** none — adjacent background; no V2X security, PKI or simulator component.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background.
