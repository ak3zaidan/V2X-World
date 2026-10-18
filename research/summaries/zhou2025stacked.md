# A Stacked Machine Learning-Based Intrusion Detection System for Internal and External Networks in Smart Connected Vehicles
Xinlei Zhou, Yujing Wu, Junhao Lin, Yinan Xu, Samuel Woo · Symmetry 2025 · https://doi.org/10.3390/sym17060874 · relevance: medium · basis: abstract

**Problem:** In response to the escalating threat of cyberattacks on smart connected vehicles, numerous Intrusion Detection Systems (IDSs) have emerged.
**Approach:** However, existing IDSs often prioritize enhancing detection accuracy while overlooking the time needed for training and detection. Moreover, they may not fully leverage the combined utilization of CAN bus IDs and the data field with external network data.
**Tools and scale:** Not stated in the abstract.
**Results:** Experiments show that the proposed IDS achieves an average detection accuracy of 99.99% for internal CAN bus attacks and 99.81% for external network attacks, with fast detection times of 0.018 ms and 0.088 ms, respectively. These results highlight the system’s real-time capability, high accuracy, and adaptability to complex attack scenarios.
**Limitations and future work (as stated by the authors):**
- However, existing IDSs often prioritize enhancing detection accuracy while overlooking the time needed for training and detection.
- To overcome these challenges, we propose a stacked-model IDS architecture deployed across the CAN bus and central gateway, capable of detecting both internal and external vehicular network attacks.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
