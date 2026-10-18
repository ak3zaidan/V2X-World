# Deep Learning Based Traffic Prediction Method for Digital Twin Network
Junyu Lai, Zhiyong Chen, Jun-Hong Zhu, Wanyi Ma et al. · Cognitive Computation 2023 · https://doi.org/10.1007/s12559-023-10136-5 · relevance: medium · basis: abstract

**Problem:** Network traffic prediction (NTP) can predict future traffic leveraging historical data, which serves as proactive methods for network resource planning, allocation, and management. Besides, NTP can also be applied for load generation in simulated and emulated as well as digital twin networks (DTNs).
**Approach:** This paper focuses on accurately predicting background traffic matrix (TM) of typical local area network (LAN) for traffic synchronization in DTN. In addition, this paper further optimizes the proposed model from both spatial and channel-wise dimensions.
**Tools and scale:** Mentioned in abstract: digital twin. (From the abstract only.)
**Results:** Comparative experiments demonstrate that the eConvLSTM model outperforms all the baselines. It can improve the prediction accuracy by reducing the mean square error (MSE) up to 10.6% for one-hop prediction and 16.8% for multi-hops prediction, compared to the legacy CovnLSTM model, with still satisfying the efficiency requirements. The further enhancement of the eConvLSTM model can additionally reduce the MSE about 2.1% for one-hop prediction and 4.2% for multi-hops prediction, with slightly degrading efficiency.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (network digital twin (learned or data-driven)); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
