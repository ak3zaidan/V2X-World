# Ns3 meets Sionna: Using Realistic Channels in Network Simulation
A. Zubow, Yannik Pilz, Sascha Rösler, Falko Dressler · arXiv.org 2024 · https://arxiv.org/abs/2412.20524 · relevance: medium · basis: abstract

**Problem:** Network simulators are indispensable tools for the advancement of wireless network technologies, offering a cost-effective and controlled environment to simulate real-world network behavior. However, traditional simulators, such as the widely used ns-3, exhibit limitations in accurately modeling indoor and outdoor scenarios due to their reliance on simplified statistical and stochastic channel propagation models, which often fail to accurately capture physical phenomena like multipath signal propagation and shadowing by obstacles in the line-of-sight path.
**Approach:** We present Ns3Sionna, which integrates a ray tracing-based channel model, implemented using the Sionna RT framework, within the ns-3 network simulator.
**Tools and scale:** Mentioned in abstract: ns-3, NS-3, Sionna, ray trac, real-world. (From the abstract only.)
**Results:** It allows to simulate environment-specific and physically accurate channel realizations for a given 3D scene and wireless device positions. Additionally, a mobility model based on ray tracing was developed to accurately represent device movements within the simulated 3D space. Ns3Sionna provides more realistic path and delay loss estimates for both indoor and outdoor environments than existing ns-3 propagation models, particularly in terms of spatial and temporal correlation.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — ns-3 + Sionna realistic channels.
**Novelty implications:** Same.
