# Reinforcement Learning based Cyberattack Model for Adaptive Traffic Signal Controller in Connected Transportation Systems
Muhammad Sami Irfan, Mizanur Rahman, Travis Atkison, Sagar Dasgupta et al. · arXiv 2022 · https://arxiv.org/abs/2211.01845 · relevance: high · basis: full text

**Problem:** Sybil attacks with fake BSMs on waiting-time-based adaptive signal controllers.
**Approach:** RL agent trained to insert fake vehicles on approaches to induce congestion.
**Tools and scale:** Custom simulation environment at one intersection.
**Results:** Agent learns an optimal attack policy and creates congestion.
**Limitations and future work (as stated by the authors):**
- Test on different intersection architectures
- deploy against DRL-based ATSCs
- next step is attack detection and mitigation.
**Overlap with our engine:** partial — partial: Sybil BSM attacks with traffic impact; ours adds PKI where Sybil requires valid pseudonyms.
**Novelty implications:** RL Sybil against ATSC is done; open: Sybil under SCMS certificate constraints and MBD.
