# Foundation CAN LM: A Pretrained Language Model For Automotive CAN Data
Akiharu Esashi, Pawissanutt Lertpongrujikorn, Justin Makino, Yuibi Fujimoto et al. · arXiv 2026 · https://doi.org/10.1109/iv66570.2026.11623984 · relevance: medium · basis: abstract

**Problem:** The Controller Area Network (CAN) bus provides a rich source of vehicular signals increasingly leveraged for applications in automotive and auto insurance domains, including collision detection, predictive maintenance, and driver risk modeling. Despite this potential, existing pipelines largely train isolated task-specific models on raw CAN data, with only ...
**Approach:** In this work, we introduce the foundation CAN model that demonstrates multi-objective downstream generalization using a single pretrained backbone.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results show that one pretrained CAN model can adapt effectively to diverse predictive tasks, validating that the foundation modeling paradigm, proven in NLP and CV, also holds for CAN data.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
