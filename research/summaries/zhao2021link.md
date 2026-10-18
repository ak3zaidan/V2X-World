# Link Scheduling Using Graph Neural Networks
Zhong-Yuan Zhao, Gunjan Verma, Chirag R. Rao, A. Swami, Santiago Segarra · IEEE Transactions on Wireless Communications 2021 · https://doi.org/10.1109/twc.2022.3222781 · relevance: low · basis: abstract

**Problem:** Efficient scheduling of transmissions is a key problem in wireless networks.
**Approach:** The main challenge stems from the fact that optimal link scheduling involves solving a maximum weighted independent set (MWIS) problem, which is known to be NP-hard. In practical schedulers, centralized and distributed greedy heuristics are commonly used to approximately solve the MWIS problem.
**Tools and scale:** Not stated in the abstract.
**Results:** Test results on medium-sized wireless networks show that our centralized heuristic can reach a near-optimal solution quickly, and our distributed heuristic based on a shallow GCN can reduce by nearly half the suboptimality gap of the distributed greedy solver with minimal increase in complexity.
**Limitations and future work (as stated by the authors):**
- To overcome this limitation, we propose fast heuristics based on graph convolutional networks (GCNs) that can be implemented in centralized and distributed manners.
**Overlap with our engine:** partial — touches learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on learned network models.
