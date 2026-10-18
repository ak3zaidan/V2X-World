# Graph-Tensor Neural Networks for Network Traffic Data Imputation
Lei Deng, Xiao-Yang Liu, Haifeng Zheng, Xinxin Feng, Zhizhang Chen · IEEE/ACM Transactions on Networking 2023 · https://doi.org/10.1109/tnet.2023.3268982 · relevance: low · basis: abstract

**Problem:** It is important to estimate the global network traffic data from partial traffic measurements for many network management tasks, including status monitoring and fault detection.
**Approach:** However, existing estimation approaches cannot well handle the topological correlations hidden in network traffic and suffer from limited imputation performance. This paper proposes a deep learning approach for network traffic imputation, which well exploits the topological structure of network traffic.
**Tools and scale:** Not stated in the abstract.
**Results:** The proposed graph neural network well captures the topological correlations of network traffic and achieves accurate imputation. Extensive experiments on real-world datasets show that the proposed graph neural network achieves about one-half lower relative square error while at least ten times faster imputation speed than the existing methods.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on learned network models.
