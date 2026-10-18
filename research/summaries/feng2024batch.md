# Batch-Aggregate: Efficient Aggregation for Private Federated Learning in VANETs
Xia Feng, Haiyang Liu, Haowei Yang, Qing-Qing Xie et al. · IEEE Transactions on Dependable and Secure Computing 2024 · https://doi.org/10.1109/tdsc.2024.3364371 · relevance: medium · basis: abstract

**Problem:** Federated learning (FL) in Vehicular Ad-hoc Networks (VANETs) enables vehicles to collaboratively train machine learning models by aggregating local gradients without revealing the training data. To ensure no gradient is revealed during aggregation, proposals are using a secret sharing-based strategy.
**Approach:** In the paper, we propose Batch-Aggregate, an efficient aggregation scheme for FL coping with high mobility and unstable connections of VANETs.
**Tools and scale:** The simulations show that the computation overhead of Batch-Aggregate grows linearly under the batch-enabled scheme, which reduces up to 50% over the existing schemes.
**Results:** Thus, the computation time of the aggregator will be reduced to $\mathcal {O}(n)$O(n) while the gradients can be aggregated in one communication round per training iteration. The simulations show that the computation overhead of Batch-Aggregate grows linearly under the batch-enabled scheme, which reduces up to 50% over the existing schemes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Bespoke VANET/IoV authentication or privacy scheme (CPPA/certificateless/batch/blockchain style), evaluated by analysis or small simulation; not IEEE 1609.2/SCMS and no closed-loop safety evaluation.
**Novelty implications:** Adds to the large body of standalone authentication schemes; does not touch end-to-end SCMS simulation or safety impact of security.
