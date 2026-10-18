# Deep Learning-Based Modeling of 5G Core Control Plane for 5G Network Digital Twin
Zhenyu Tao, Yongliang Guo, Guanghui He, Yongming Huang et al. · IEEE Transactions on Cognitive Communications and Networking 2023 · https://doi.org/10.1109/tccn.2023.3324634 · relevance: medium · basis: abstract

**Problem:** Modelling 5G core control plane for a twin.
**Approach:** 5GC-Seq2Seq and 5GC-former models of signalling.
**Tools and scale:** Spirent C50 signalling data: registration, handover, PDU sessions.
**Results:** 5GC-Seq2Seq 99.997% F1 for single UE, but significantly reduced in multi-UE cases.
**Limitations and future work (as stated by the authors):**
- Performance drops in multi-UE scenarios.
**Overlap with our engine:** none.
**Novelty implications:** Learned protocol-message twins exist; a PKI-protocol twin is not addressed.
