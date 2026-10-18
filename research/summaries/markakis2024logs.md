# From Logs to Causal Inference: Diagnosing Large Systems
Markos Markakis, Brit Youngmann, Trinity Gao, Ziyu Zhang et al. · Proceedings of the VLDB Endowment 2024 · https://doi.org/10.14778/3705829.3705836 · relevance: medium · basis: abstract

**Problem:** Causal inference can quantify cause-effect relationships in domains as varied as medicine, economics and public policy. Production computer systems exhibit a similar level of complexity and a recurring need to diagnose problems quickly.
**Approach:** In this work, we want to accelerate large systems debugging by applying causal inference over logs, enabling engineers to diagnose problems and assess interventions in a principled manner. We also provide a data preparation pipeline, the
 Log Converter
 , which transforms raw, messy, real-world logs into an appropriate tabular input for causal inference, using methods drawn from data transformation, cleaning, and extraction.
**Tools and scale:** Mentioned in abstract: real-world. (From the abstract only.)
**Results:** Our framework achieves this through two human-in-the-loop modules: (1) The
 Candidate Cause Ranker
 , through which one can determine the causes of a variable without running a full causal discovery algorithm; and (2) the
 Interactive Causal Graph Refiner
 , which helps engineers compute an unbiased estimation of their effect of interest without extensive manual causal graph verification. We evaluate LOGos, a prototype implementation, on both real-world and synthetic logs and find that: (1) The
 Candidate Cause Ranker
 achieved an average precision 1.08×-18× higher than the baselines, in interactive time; (2) The
 Interactive Causal Graph Refiner
 required a number of causal judgments 1.61 × - 16.83× lower than the baselines; and (3) The latency of
 Log Converter
 scaled linearly with three measures of the complexity of a log: length, distinct templates, and fraction of tokens that are variables.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — Causal inference over system logs for diagnosis.
**Novelty implications:** Same.
