# DWM-RO: Decentralized World Models with Reasoning Offloading for SWIPT-enabled Satellite-Terrestrial HetNets
Guangyuan Liu, Yinqiu Liu, Ruichen Zhang, Nan Ma, Jiawen Kang, Sumei Sun, Abbas Jamalipour, Ping Zhang · arXiv 2025 · https://arxiv.org/abs/2511.05972 · relevance: low · basis: abstract

**Problem:** Wireless networks are undergoing a paradigm shift toward massive connectivity with energy-efficient operation, driving the integration of satellite-terrestrial architectures with simultaneous wireless information and power transfer (SWIPT).
**Approach:** Optimizing transmit beamforming and power splitting in such systems faces formidable challenges, e.g., time-varying channels and multi-tier interference, which create a complex decision landscape where conventional model-free multi-agent reinforcement learning (MARL) suffers from sample inefficiency due to rarely-encountered state transitions and poor coordination as decentralized agents act independently. This paper proposes the Decentralized World Model with Reasoning Offloading (DWM-RO) framework to address these fundamental limitations.
**Tools and scale:** Mentioned in abstract: simulations.
**Results:** Specifically, each agent employs a world model to learn compact predictive representations of environment dynamics, enabling imagination-based policy training that dramatically reduces required environment interactions. Extensive simulations demonstrate that DWM-RO converges 5 times faster than state-of-the-art baselines while achieving 34.7% higher spectral efficiency and reducing constraint violations by 40%.
**Limitations and future work (as stated by the authors):**
- This paper proposes the Decentralized World Model with Reasoning Offloading (DWM-RO) framework to address these fundamental limitations.
**Overlap with our engine:** partial — touches world models, network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on world models.
