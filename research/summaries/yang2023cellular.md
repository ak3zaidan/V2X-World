# Cellular-V2X QoS Adaptive Distributed Congestion Control: A Deep Q Network Approach
Wooyeol Yang, Byeongcheol Jeon, Cheol Mun, Han-Shin Jo · 2023 IEEE 20th Consumer Communications &amp; Networking Conference (CCNC) 2023 · https://doi.org/10.1109/ccnc51644.2023.10059812 · relevance: medium · basis: abstract

**Problem:** Distributed congestion control (DCC) is a representative algorithm for improving the performance of Cellular-V2X (C-V2X) in dense vehicle scenarios. Since the existing DCC algorithms are not designed to respond to the target value of the Quality of Service (QoS) parameter adaptively, they can improve communication performance to some extent but do not guarantee the achievement of the target value.
**Approach:** To solve this problem, we propose a deep Q network (DQN)-based QoS adaptive DCC algorithm in C-V2X Vehicle to Vehicle (V2V) communication. In particular, considering the limitations of the V2V network, where the exchange of state and reward information between vehicles is limited, we adopt a central controller (single agent) to train the congestion control policy and then distribute the completed policy to each vehicle for distributed execution.
**Tools and scale:** Mentioned in abstract: C-V2X. (From the abstract only.)
**Results:** Simulation results show that the proposed algorithm satisfies the target QoS more effectively than the standardized ETSI DCC algorithm.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — DQN congestion control for C-V2X.
**Novelty implications:** Radio-layer only.
