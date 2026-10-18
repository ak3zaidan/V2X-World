# Internet Traffic Forecasting using Temporal-Topological Graph Convolutional Networks
Zhenjie Yao, Qian Xu, Yongrui Chen, Yanhui Tu, He Zhang, Yixin Chen · 2021 International Joint Conference on Neural Networks (IJCNN) 2021 · https://doi.org/10.1109/ijcnn52387.2021.9534096 · relevance: low · basis: abstract

**Problem:** Accurate and timely prediction of the Internet traffic flow is important for network performance improvement.
**Approach:** Few prediction methods incorporate the topology information of networks. In this paper, we present a novel Internet traffic forecasting algorithm named TTGCN, which applies the graph neural networks for traffic flow prediction on each link of a backbone network.
**Tools and scale:** Mentioned in abstract: dataset.
**Results:** The experimental results show that the average error of the proposed TTGCN over 90 minutes is 28.249 (Mbit/s), which is a significant improvement of conventional models, while that of ARIMA and GRU and STGCN are 44.955, 40.935, and 36.152, respectively. The proposed TTGCN, by encoding both temporal and topological information in a neural representation, can achieve better prediction performance on network traffic.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on learned network models.
