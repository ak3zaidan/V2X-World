# DeepQueueNet: Towards Scalable and Generalized Network Performance Estimation with Packet-Level Visibility
Qingqing Yang, Xi Peng, Li Chen, Libin Liu, Jingze Zhang, Hong Xu et al. · Proceedings of the ACM SIGCOMM 2022 Conference 2022 · https://doi.org/10.1145/3544216.3544248 · relevance: high · basis: abstract

**Problem:** DES network simulators do not scale; DL estimators lack packet-level visibility and generalisation.
**Approach:** DeepQueueNet: queueing-theoretic modelling with DNNs only for intractable parts, combined with DES for packet-level visibility.
**Tools and scale:** Arbitrary topologies and device traffic management; multi-GPU.
**Results:** Near-linear speedup with GPUs; better average and p99 RTT accuracy than end-to-end DNN estimators.
**Limitations and future work (as stated by the authors):**
- Requires sufficient training data for each device/mechanism.
**Overlap with our engine:** partial - simulator acceleration with learned components (direction 3).
**Novelty implications:** Hybrid physics/ML surrogates exist; V2X-specific ones remain open.
