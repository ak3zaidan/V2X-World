# Conformal Counterfactual Inference under Hidden Confounding
Zonghao Chen, Ruocheng Guo, Jean-François Ton, Yang Liu · Knowledge Discovery and Data Mining 2024 · https://doi.org/10.1145/3637528.3671976 · relevance: medium · basis: abstract

**Problem:** Personalized decision making requires the knowledge of potential outcomes under different treatments, and confidence intervals about the potential outcomes further enrich this decision-making process and improve its reliability in high-stakes scenarios. Predicting potential outcomes along with its uncertainty in a counterfactual world poses the foundamental challenge in causal inference.
**Approach:** In this paper, to overcome these limitations, we first propose a novel approach wTCP-DR based on transductive weighted conformal prediction, which provides confidence intervals for counterfactual outcomes with marginal converage guarantees, even under hidden confounding. Since transductive conformal prediction is notoriously costly, we propose wSCP-DR, a two-stage variant of wTCP-DR, based on split conformal prediction with same marginal coverage guarantees but at a significantly lower computational cost.
**Tools and scale:** Mentioned in abstract: real-world. (From the abstract only.)
**Results:** Theoretical results explicitly demonstrate the conditions under which our algorithm is strictly advantageous to the naive method that only uses interventional data. We demonstrate our method across synthetic and real-world data, including recommendation systems, to verify the superiority of our methods compared against state-of-the-art baselines in terms of both coverage and efficiency. Our code can be found at https://github.com/rguo12/KDD24-Conformal.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — Conformal intervals for counterfactual outcomes.
**Novelty implications:** Method relevant to direction 3.
