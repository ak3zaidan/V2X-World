# Traffic-Driven Scaling of Digital Twin Proxy Pool in Vehicular Edge Computing
Hao Zhu, Shuaili Bao, Li Jin, Guoan Zhang · Electronics 2025 · https://doi.org/10.3390/electronics14244898 · relevance: low · basis: abstract

**Problem:** This paper presents a traffic-driven scaling framework for a digital twin proxy pool (DTPP) in vehicular edge computing (VEC), designed to eliminate the latency and synchronization issues inherent in conventional digital twin (DT) migration approaches.
**Approach:** The core innovation lies in replacing the migration of vehicle DTs between edge servers (ESs) with instantaneous switching within a pre-allocated pool of DT proxies, thereby achieving zero migration latency and continuous synchronization. The proposed architecture differentiates between short-term DTs (SDTs) hosted in edge-side in-memory databases for real-time, low-latency services, and long-term DTs (LDTs) in the cloud for historical data ...
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The scaling mechanism operates on a cloud–edge collaborative principle: a cloud-based predictor, employing a TCN-Transformer fusion model, forecasts hourly traffic arrival rates to set a baseline proxy count, while edge-side managers perform monotonic, 5 min scale-ups based on real-time monitoring to absorb sudden traffic bursts without causing service jitter. The TCN-Transformer predictor significantly outperforms single-model baselines, ...
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
