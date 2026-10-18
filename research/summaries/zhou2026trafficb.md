# Traffic Engineering in Large-Scale Networks With Generalizable Graph Neural Networks
Fangtong Zhou, Sihao Liu, Xiaorui Liu, Ruozhou Yu, Guoliang Xue · IEEE Transactions on Networking 2026 · https://doi.org/10.1109/ton.2026.3659135 · relevance: low · basis: abstract

**Problem:** Traffic Engineering (TE) in large-scale networks like cloud Wide Area Networks (WANs) and Low Earth Orbit (LEO) satellite constellations is a critical challenge.
**Approach:** Although learning-based approaches have been proposed to address the scalability of traditional TE algorithms, their practical application is often hindered by a lack of generalization, high training overhead, and a failure to respect link capacities. This paper proposes TELGEN, a novel TE algorithm that learns to solve TE problems efficiently in large-scale network scenarios, while achieving superior generalizability across diverse network conditions.
**Tools and scale:** Not stated in the abstract.
**Results:** TELGEN shows less than 3% optimality gap while ensuring feasibility in all testing scenarios, even when the test network has $2\text {-}20\times $ more nodes than the largest training network. It also saves up to 84% TE solving time than traditional interior-point method, and reduces up to 79.6% training time per epoch than the state-of-the-art learning-based algorithm.
**Limitations and future work (as stated by the authors):**
- Although learning-based approaches have been proposed to address the scalability of traditional TE algorithms, their practical application is often hindered by a lack of generalization, high training overhead, and a failure to respect link capacities.
**Overlap with our engine:** partial — touches learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on learned network models.
