# Impact Evaluation of Falsified Data Attacks on Connected Vehicle Based Traffic Signal Control
Shihong Ed Huang, Wai Wong, Yiheng Feng, Qi Alfred Chen et al. · arXiv 2020 · https://arxiv.org/abs/2010.04753 · relevance: high · basis: full text

**Problem:** Impact of falsified BSMs on CV-based traffic signal control when control logic is unknown to the attacker.
**Approach:** Attacker learns signal logic with a surrogate model, identifies critical features, and optimises falsified BSMs to maximise plan dissimilarity.
**Tools and scale:** Case study on I-SIG in traffic simulation.
**Results:** Surrogate mimics I-SIG well; I-SIG sensitive to ETA and number of approaching vehicles; attacks degrade performance.
**Limitations and future work (as stated by the authors):**
- None explicit in extracted sections.
**Overlap with our engine:** partial — partial: falsified BSMs and traffic impact; also uses a learned surrogate (of the controller, not the network).
**Novelty implications:** Surrogate-assisted attacks on CV-TSC exist; closed-loop with SCMS detection/revocation remains open.
