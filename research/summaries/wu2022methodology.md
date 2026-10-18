# A Methodology for Abstracting the Physical Layer of Direct V2X Communications Technologies
Zhuofei Wu, S. Bartoletti, Vincent Martinez, Alessandro Bazzi · Italian National Conference on Sensors 2022 · https://doi.org/10.3390/s22239330 · relevance: medium · basis: abstract

**Problem:** Recent advancements in vehicle-to-everything (V2X) communications have greatly increased the flexibility of the physical (PHY) and medium access control (MAC) layers. This increases the complexity when investigating the system from a network perspective to evaluate the performance of the supported applications.
**Approach:** The goal of this paper is to develop a new approach for modeling the PHY layer of V2X communications that can be extended to a wide range of configurations without leading to extensive measurement or simulation campaigns at the link layer. signal-to-interference-plus-noise ratio (SINR) related to a subset of possible configurations, we first approximated the curves with step functions characterized by a given SINR threshold, and we then derived one parameter, called implementation loss, that was used to obtain the SINR threshold and evaluate the network performance under any configuration in the same scenario.
**Tools and scale:** Mentioned in abstract: IEEE 802.11p, LTE-V2X. (From the abstract only.)
**Results:** The proposed methodology, leading to a good trade-off among the complexity, generality, and accuracy of the performance evaluation process, was validated through extensive simulations with both IEEE 802.11p and LTE-V2X sidelink technologies in various scenarios. The results first show that the curves can be effectively approximated by using an SINR threshold, with a value corresponding to 0.5 PER, and then demonstrate that the network-level outputs derived from the proposed approach are very close to those obtained with complete curves, despite not being restricted to a few possible configurations.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — PHY abstraction methodology for direct V2X technologies.
**Novelty implications:** Relevant to our PHY abstraction.
