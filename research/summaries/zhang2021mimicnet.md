# MimicNet: Fast Performance Estimates for Data Center Networks with Machine Learning
Qizhen Zhang, Kelvin K. W. Ng, Charles Kazer, Shen Yan, João Sedoc, Vincent Liu · Proceedings of the 2021 ACM SIGCOMM 2021 Conference 2021 · https://doi.org/10.1145/3452296.3472926 · relevance: high · basis: abstract

**Problem:** Full-scale data-centre evaluation is intractable for testbeds and simulators.
**Approach:** MimicNet: packet-level simulation of a portion of the network with ML models approximating the rest.
**Tools and scale:** Data centres with thousands of servers.
**Results:** Over two orders of magnitude speedup; tail FCT, throughput and RTT within 5% of true results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial - hybrid simulation plus learned surrogate (direction 3), data-centre domain.
**Novelty implications:** Hybrid sim/ML acceleration is established; vehicular counterfactual surrogates are not addressed.
