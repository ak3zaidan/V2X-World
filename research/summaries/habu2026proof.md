# Proof-of-Concept of Network Digital Twin-Based Channel Prediction for mmWave V2X Communications
Shintaro Habu, Roberto Pegurri, Kui Wang, F. Linsalata et al. · IEEE International Symposium on Personal, Indoor and Mobile Radio Communications 2026 · https://doi.org/10.1109/pimrc69400.2026.11711789 · relevance: medium · basis: abstract

**Problem:** Millimeter-wave vehicle-to-everything (V2X) communications are a promising technology for supporting high-datarate cooperative sensing applications, such as LiDAR point-cloud sharing among vehicles and roadside infrastructure. However, mmWave links are highly sensitive to mobility-induced blockage, line-of-sight (LoS) transitions, and antenna ...
**Approach:** To address this issue, this work presents a real-time Network digital twin (Network DT)-based received signal strength indicator (RSSI) prediction framework for mmWave V2X communications.
**Tools and scale:** We implement the framework in an outdoor proof-of-concept testbed using 60 GHz WiGig links and evaluate its prediction accuracy by comparing predicted RSSI with field measurements.
**Results:** Furthermore, a pointcloud transmission experiment demonstrates that the predicted RSSI provides useful link-quality information for assessing the feasibility of cooperative sensing data delivery over direct and roadside unit (RSU)-assisted paths. These results indicate that real-time Network DT-based RSSI prediction can serve as a practical foundation for proactive and reliable mmWave V2X communication control.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
