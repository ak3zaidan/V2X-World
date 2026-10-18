# H-RT-IDPS: A Hierarchical Real-Time Intrusion Detection and Prevention System for the Smart Internet of Vehicles via TinyML-Distilled CNN and Hybrid BiLSTM-XGBoost Models
Ikram Hamdaoui, Chaymae Rami, Zakaria El Allali, Khalid El Makkaoui · Technologies 2025 · https://doi.org/10.3390/technologies13120572 · relevance: medium · basis: abstract

**Problem:** The integration of connected vehicles into smart city infrastructure introduces critical cybersecurity challenges for the Internet of Vehicles (IoV), where resource-constrained vehicles and powerful roadside units (RSUs) must collaborate for secure communication.
**Approach:** We propose H-RT-IDPS, a hierarchical real-time intrusion detection and prevention system targeting two high-priority IoV security pillars: availability (traffic overload) and integrity/authenticity (spoofing), with spoofing evaluated across multiple subclasses (GAS, RPM, SPEED, and steering wheel). In the offline phase, deep learning and hybrid models were benchmarked on the vehicular CAN bus dataset CICIoV2024, with the BiLSTM-XGBoost hybrid chosen for its balance between accuracy and inference speed.
**Tools and scale:** Not stated in the abstract.
**Results:** In baseline, stealth, and coordinated modes, the evaluation achieved accuracy, precision, recall, and F1-scores all above 97%. These results demonstrate a scalable, explainable, and operator-focused IDPS well suited for securing SC-IoV deployments against evolving threats.
**Limitations and future work (as stated by the authors):**
- The integration of connected vehicles into smart city infrastructure introduces critical cybersecurity challenges for the Internet of Vehicles (IoV), where resource-constrained vehicles and powerful roadside units (RSUs) must collaborate for secure communication.
- The framework remains robust in both high-traffic and low-frequency attack scenarios, enhancing operator situational awareness through real-time visualizations.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
