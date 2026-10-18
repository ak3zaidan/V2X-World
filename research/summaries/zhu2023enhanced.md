# Enhanced Federated Learning for Edge Data Security in Intelligent Transportation Systems
Rongbo Zhu, Meng-Yao Li, Jiangjin Yin, Lubing Sun et al. · IEEE transactions on intelligent transportation systems (Print) 2023 · https://doi.org/10.1109/tits.2023.3243088 · relevance: medium · basis: abstract

**Problem:** Federated learning (FL) provides a promising solution to meet the requirements of data privacy and security in intelligent transportation systems (ITS), which enables edge devices and road side units (RSUs) to collaboratively train learning models without exposing the raw data. However, the deep leakage from gradients (DLG) still leads to the risk of ...
**Approach:** Focusing on improving edge data security in ITS, this paper proposes an enhanced federated learning (FL) model (SemBroc-RF) with reinforcement learning, which considers the advantages of both end-to-end homomorphic encryption (HE) and SMC.
**Tools and scale:** Theoretical analysis and detailed simulation results verify that SemBroc-RF can effectively prevent gradient leakage.
**Results:** To improve the model accuracy by gradients aggregation, a FL algorithm (GreFLa) with reinforcement learning is proposed based on the adaptive assigned weight of the local gradients. On the MNIST and CIFAR-10 datasets, compared with the benchmark, the accuracy of SemBroc-RF is increased by 3.63% and 1.35%, and the training round of SemBroc-RF is reduced by 70.8% and 45.6%, respectively.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Bespoke VANET/IoV authentication or privacy scheme (CPPA/certificateless/batch/blockchain style), evaluated by analysis or small simulation; not IEEE 1609.2/SCMS and no closed-loop safety evaluation.
**Novelty implications:** Adds to the large body of standalone authentication schemes; does not touch end-to-end SCMS simulation or safety impact of security.
