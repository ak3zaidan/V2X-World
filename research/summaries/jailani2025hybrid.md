# A Hybrid Deep Learning Framework for Multi-Modal Intrusion Detection in Internet of Vehicles
Nazir Ahmad Jailani, Rakesh Kumar, Sanjay Tyagi · 2025 3rd International Conference on Sustainable Computing and Data Communication Systems (ICSCDS) 2025 · https://doi.org/10.1109/icscds65426.2025.11166822 · relevance: medium · basis: abstract

**Problem:** The increasing complexity of in-vehicle networks has heightened the vulnerability of modern automobiles to cyber-attacks, necessitating robust and real-time intrusion detection mechanisms.
**Approach:** In this study, we propose a hybrid deep learning architecture that integrates 1D CNN-LSTM, Wavelet-based Vision Transformers (ViT), and Graph Attention Networks (GAT) to effectively model the multimodal nature of vehicular communication data—comprising binary payloads, normalized sensor readings, and hexadecimal metadata. A gated attention fusion mechanism dynamically learns the relevance of each modality during inference, enhancing classification performance.
**Tools and scale:** Not stated in the abstract.
**Results:** Evaluated on the CICIoV2024 dataset, the proposed model achieved a ROC-AUC of 99.78%, an Attack Detection Rate (ADR) of 98.9%, and reduced the False Positive Rate (FPR) to 1.2%, significantly outperforming classical machine learning baselines. These results highlight the effectiveness of modality-aware fusion and deep multimodal feature extraction in securing in-vehicle networks against diverse cyber threats.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
