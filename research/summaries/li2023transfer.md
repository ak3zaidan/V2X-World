# A Transfer Double Deep Q Network Based DDoS Detection Method for Internet of Vehicles
Zhong Li, Yubo Kong, Chang-Jun Jiang · IEEE Transactions on Vehicular Technology 2023 · https://doi.org/10.1109/tvt.2022.3233880 · relevance: medium · basis: abstract

**Problem:** Distributed denial of service (DDoS) attacks have become one of the main factors restricting the development of internet of vehicles (IoV). Although some intelligent reinforcement learning based methods have been introduced to mitigate DDoS attacks, there are still many constraints in the training process, such as the long training time and dependence on large labeled data.
**Approach:** In this paper, we propose a transfer double deep Q-network (DDQN) based DDoS detection method for IoV. Furthermore, by utilizing the knowledge obtained by adjacent similar base stations, we design a transfer DDQN method based on traffic flow similarity to speed up the training of DDQN for a newly added base station. We compare our proposed method with other popular machine learning based DDoS detection methods.
**Tools and scale:** Not stated in the abstract.
**Results:** The experimental results show that the transfer DDQN based DDoS detection method improves F1-measure and detection accuracy by 79.4% and 17.5% on average. Meanwhile, by means of traffic flow similarity based transfer learning, the time consumption and convergence time can be reduced by 41.3% and 31.1%, which makes our method adapt to the dynamic network.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — V2X/VANET attack or intrusion detection; overlaps our misbehaviour-detection component only at the detector level, without SCMS reporting, revocation or closed-loop safety measurement.
**Novelty implications:** Does not rule out any candidate direction; it does not model SCMS certificates, linkage-based revocation, per-message joint ground truth or counterfactual reruns.
