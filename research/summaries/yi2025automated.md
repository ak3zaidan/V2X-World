# Automated, Cross-Layer Root Cause Analysis of 5G Video-Conferencing Quality Degradation
Fan Yi, Haoran Wan, Kyle Jamieson, Oliver Michel · ACM/SIGCOMM Internet Measurement Conference 2025 · https://doi.org/10.1145/3730567.3764434 · relevance: medium · basis: abstract

**Problem:** 5G wireless networks leverage complex scheduling, retransmission, and adaptation mechanisms to maximize their efficiency. These mechanisms interact to produce significant fluctuations in uplink and downlink capacity and latency, markedly impacting the the performance of real-time communication and multimedia applications, such as video conferencing.
**Approach:** In this paper, we present a cross-layer view of 5G networks and their impact on and interaction with video-conferencing applications. We conduct novel, detailed measurements of both private CBRS and commercial carrier cellular network dynamics, capturing physical- and link-layer events and correlating them with their effects at the network and transport layers, and the video-conferencing application itself.
**Tools and scale:** Mentioned in abstract: 5G. (From the abstract only.)
**Results:** Our two datasets comprise days of low-rate campus-wide Zoom telemetry data, and hours of high-rate, correlated WebRTC-network-5G telemetry data. Based on these data, we trace performance anomalies back to root causes, identifying 24 previously unknown causal event chains that degrade 5G video conferencing.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — Automated cross-layer RCA of 5G video quality degradation.
**Novelty implications:** Cross-layer RCA exists for 5G apps; not for V2X/PKI.
