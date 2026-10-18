# DDoS Mitigation Based on Space-Time Flow Regularities in IoV: A Feature Adaption Reinforcement Learning Approach
Zhong Li, Yubo Kong, Cheng Wang, Changjun Jiang · IEEE Transactions on Intelligent Transportation Systems 2022 · https://doi.org/10.1109/tits.2021.3066404 · relevance: medium · basis: abstract

**Problem:** With the development of 5G technology, mobile edge computing (MEC) is introduced into the construction of internet of vehicles (IoV). However, the distributed denial of services (DDoS) attacks become a serious problem in IoV under MEC.
**Approach:** By employing reinforcement learning, we propose a feature adaption reinforcement learning approach based on the space-time flow regularities in IoV for DDoS mitigation, named FAST. In FAST, we elaborately design a combinational action space, and a reward function based on Kalman filter method and historical data traffic flows, which can make FAST to recognize DDoS attacks more quickly and accurately.
**Tools and scale:** In experiments, we evaluate the performance of FAST based on Shenzhen taxicab dataset.
**Results:** We simulate and inject DDoS attacks into Shenzhen taxicabs through two DDoS simulation tools named ‘ddosflowgen’ and ‘hping3’. The experimental results show that FAST has a high quality in detecting multiple types of DDoS attacks compared with other detection methods.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — V2X/VANET attack or intrusion detection; overlaps our misbehaviour-detection component only at the detector level, without SCMS reporting, revocation or closed-loop safety measurement.
**Novelty implications:** Does not rule out any candidate direction; it does not model SCMS certificates, linkage-based revocation, per-message joint ground truth or counterfactual reruns.
