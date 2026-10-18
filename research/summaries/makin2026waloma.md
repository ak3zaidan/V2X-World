# WALoMA: A Multitask Wireless Foundation Model via Adaptive Low-Rank Masked Autoencoders
Madi Makin, Asmaa Abdallah, Abdulkadir Celik, Ahmed M. Eltawil · arXiv 2026 · https://arxiv.org/abs/2607.25763 · relevance: medium · basis: abstract

**Problem:** This paper proposes a multitask wireless foundation model via adaptive low-rank masked autoencoders (WALoMA), a unified multi-task foundation model for sixth-generation (6G) wireless physical layer architectures, to address the limitations of specialized, task-specific deep learning models and the practical challenge of scarce labeled wireless datasets. By ...
**Approach:** This paper proposes a multitask wireless foundation model via adaptive low-rank masked autoencoders (WALoMA), a unified multi-task foundation model for sixth-generation (6G) wireless physical layer architectures, to address the limitations of specialized, task-specific deep learning models and the practical challenge of scarce labeled wireless datasets.
**Tools and scale:** Not stated in the abstract.
**Results:** The framework's efficacy is demonstrated across five downstream tasks, achieving individual scores of 96.47\% for LoS/NLoS classification, 80.45\% for beam prediction, 85.78\% for channel interpolation, 99.12\% for channel estimation, and 77.18\% for channel charting. Consequently, numerical results show that the proposed model achieves a composite score of 87.80\%, significantly outperforming the 59.90\% achieved by the large wireless model (LWM) baseline while training an average of only 14.68\% of total parameters, and maintaining strong ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
