# Electromagnetic World Model for 6G: A Unified Framework for Joint Environment Reconstruction and Channel Prediction
Yizhu Zhao, Li Yu, Jianhua Zhang, Yuxiang Zhang et al. · arXiv 2026 · https://arxiv.org/abs/2608.17769 · relevance: medium · basis: abstract

**Problem:** The integration of sensing, communication, and intelligence is becoming a key enabler for sixth generation (6G) wireless systems, where intelligent terminals are expected to simultaneously support efficient link establishment and reliable environmental sensing. However, existing studies mainly exploit sensing information or communication information to ...
**Approach:** Motivated by the shared dependence of optical and radio-frequency signals on the surrounding environment, we propose the electromagnetic world model (EMWM), the first unified framework for joint environment reconstruction and channel prediction.
**Tools and scale:** Moreover, a large-scale multi-modal dataset is constructed based on a campus digital twin.
**Results:** Based on the learned representation, a mixture-of-experts (MoE)-based CSI prediction head reconstructs the complete CSI, while a depth prediction head estimates multi-view depth maps that are further converted into three-dimensional (3D) point clouds. Experimental results show that EMWM outperforms conventional neural network and large language model (LLM) baselines in both CSI prediction and environment reconstruction, achieving a squared generalized cosine similarity (SGCS) of 0.9699 for CSI prediction while demonstrating ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
