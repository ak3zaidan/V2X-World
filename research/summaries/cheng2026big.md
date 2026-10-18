# How Big Should a Wireless Foundation Model Be?
Wei-Lun Cheng, Wanjiun Liao · arXiv 2026 · https://arxiv.org/abs/2605.07266 · relevance: medium · basis: abstract

**Problem:** Wireless foundation models are rapidly emerging as a key enabler of AI-native communication systems, yet a fundamental question remains unanswered: how large should these models be? We present a principled, physics-grounded answer, showing that the intrinsic dimensionality (dNL, the nonlinear manifold dimension of the channel) acts as the fundamental ...
**Approach:** We present a principled, physics-grounded answer, showing that the intrinsic dimensionality (dNL, the nonlinear manifold dimension of the channel) acts as the fundamental bottleneck, defining the scaling ceiling once a data-sufficient regime is reached.
**Tools and scale:** This dimensionality is not a design choice but a physical constraint: Maxwell's equations, finite scatterers, and antenna aperture inherently constrain wireless propagation environments to a limited number of degrees of freedom -- spanning 5-35 across both real-world OTA measurements and ...
**Results:** Beyond this ceiling, inference-time adaptation via pilot-aided test-time training (TTT) is far more effective: a compact 12M-parameter model surpasses a static 96M model by 9.9 dB (NMSE, SNR = 20 dB) / 7.6 dB (MCM, SNR = 10 dB) at one-eighth the parameters. With dNL distributions validated across real-world indoor massive MIMO measurements, our scaling laws and TTT gains are demonstrated through NTN satellite simulations, reframing wireless AI design: channel geometry -- not model size -- fundamentally governs the scaling laws of ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
