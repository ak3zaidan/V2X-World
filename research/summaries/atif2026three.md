# A Three-Axis Stress Test of LLM vs Classical ML for Network Intrusion Detection under Distribution Shift and Adversarial Evasion
Muhammad Ebad Atif, Muhammad Haider Ali · arXiv 2026 · https://arxiv.org/abs/2609.13511 · relevance: medium · basis: abstract

**Problem:** Same-dataset NIDS evaluation is incomplete.
**Approach:** Stress test XGBoost vs RoBERTa-LoRA on same-dataset, cross-dataset and adversarial axes.
**Tools and scale:** Two NetFlow v2 networks.
**Results:** Tied same-dataset; XGBoost +15 F1 cross-dataset; RoBERTa +17 F1 under evasion; RoBERTa FPR 0.78 on target network.
**Limitations and future work (as stated by the authors):**
- Recommendation depends on axis tested.
**Overlap with our engine:** none.
**Novelty implications:** Methodological lesson for misbehaviour detector evaluation.
