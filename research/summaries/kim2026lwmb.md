# LWM-TF: A Wireless Foundation Model on 5G NR Time-Frequency Resource Grids
Namhyun Kim, Sadjad Alikhani, Ahmed Alkhateeb · arXiv 2026 · https://arxiv.org/abs/2610.21013 · relevance: medium · basis: abstract

**Problem:** Received OFDM grids preserve both pilot and payload observations, yet wireless foundation models are rarely pre-trained on them. We introduce LWM-TF, a 9.58M-parameter factorized masked autoencoder pre-trained on complex 5G NR uplink grids taken after the FFT and before channel estimation.
**Approach:** We introduce LWM-TF, a 9.58M-parameter factorized masked autoencoder pre-trained on complex 5G NR uplink grids taken after the FFT and before channel estimation.
**Tools and scale:** The encoder is pre-trained on 850,496 physical uplink shared channel (PUSCH) slots from a 10^6-slot dataset simulated with the NR transmit chain under various channel conditions.
**Results:** With 10,000 labels the probes reach 0.94, 0.96, and 0.92, well above engineered receiver statistics, and detect mobility where the CNN stays at chance. Without any impairment labels, distances in the frozen feature space flag unseen waveform impairments with 0.78 macro AUROC, against 0.68 for receiver statistics.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
