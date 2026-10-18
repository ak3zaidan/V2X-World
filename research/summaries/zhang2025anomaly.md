# Anomaly Detection of Vehicle Data Stream Based on LSTMAM
Jin-Feng Zhang, Yue-Zhong Zhang, Ya-Bin Peng et al. · International Conference on Parallel and Distributed Systems 2025 · https://doi.org/10.1109/icpads67057.2025.11323157 · relevance: medium · basis: abstract

**Problem:** During the transmission of Controller Area Network (CAN) data streams in intelligent connected vehicles, security risks such as non-compliant data reporting and cross-border data leakage inevitably arise.
**Approach:** To overcome the limitation, this paper proposes an anomaly detection approach for CAN data streams that integrates a heterogeneous Long Short-Term Memory (LSTM) model with an Attention Mechanism (AM). Based on the periodicity of each byte-level data stream, a heterogeneous LSTM model incorporating varied parameters-such as time step size and network width-is designed.
**Tools and scale:** Not stated in the abstract.
**Results:** Comparative results demonstrate that the proposed method significantly improves detection accuracy over existing approaches, validating its effectiveness at identifying anomalies in CAN data streams.
**Limitations and future work (as stated by the authors):**
- "To overcome the limitation, this paper proposes an anomaly detection approach for CAN data streams that integrates a heterogeneous Long Short-Term Memory (LSTM) model with an Attention Mechanism (AM)."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
