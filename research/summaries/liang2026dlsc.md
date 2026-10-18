# DLSC: A double-layer blockchain consensus algorithm based on Schnorr threshold signatures for large-scale IoT systems
Zongwan Liang, Jiali Zheng · Proceedings of the 2026 International Conference on Intelligent Internet of Things and Communication Technology 2026 · https://doi.org/10.1145/3821820.3821826 · relevance: low · basis: abstract

**Problem:** With the explosive growth of Internet of Things (IoT) devices, blockchain-based traceability systems are increasingly challenged by limited scalability, excessive communication overhead, and high computational burden in consensus procedures. To address these issues, this paper proposes a double-layer Schnorr threshold signature consensus algorithm, namely DLSC, for large-scale ...
**Approach:** To address these issues, this paper proposes a double-layer Schnorr threshold signature consensus algorithm, namely DLSC, for large-scale IoT environments. The proposed scheme adopts a hierarchical node architecture combined with a Schnorr threshold signature mechanism, where lower-layer nodes generate signature shares within each subgroup, while upper-layer nodes collaboratively complete global consensus.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Theoretical analysis and simulation results demonstrate that DLSC achieves higher throughput, stronger fault tolerance, and lower resource consumption under large-scale IoT deployments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
