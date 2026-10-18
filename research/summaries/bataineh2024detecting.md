# Detecting Poisoning Attacks in Collaborative IDSs of Vehicular Networks Using XAI and Shapley Value
A. Bataineh, Mohammad Zulkernine, Adel Abusitta, Talal Halabi · ACM Journal on Autonomous Transportation Systems 2024 · https://doi.org/10.1145/3696462 · relevance: medium · basis: abstract

**Problem:** Machine learning-based Intrusion Detection Systems (IDSs) for vehicle networks can collaborate to enhance their performance by sharing crucial decisions when individual datasets lack diversity, which hinders effective model training.
**Approach:** However, such collaborative coalitions are vulnerable to poisoning attacks, where certain members tamper with training data, leading to wrong predictions by the IDSs. The current solutions to this problem have the following issues: (1) They require accessing the training dataset of IDSs, which raises critical privacy concerns; (2) their heavy reliance on voting mechanisms may exclude clients and fail to detect malicious coalition members when attackers form the majority coalition; and (3) the verification process can potentially expose sensitive information, posing privacy risks.
**Tools and scale:** Not stated in the abstract.
**Results:** To address these issues and improve the resilience of collaborative IDSs against poisoning attacks, we propose a novel approach that combines XAI (Explainable AI) technology with Shapley value from cooperative game theory. Our results prove highly promising in detecting poisoning attacks and overcoming the flaws in existing solutions.
**Limitations and future work (as stated by the authors):**
- Machine learning-based Intrusion Detection Systems (IDSs) for vehicle networks can collaborate to enhance their performance by sharing crucial decisions when individual datasets lack diversity, which hinders effective model training.
- However, such collaborative coalitions are vulnerable to poisoning attacks, where certain members tamper with training data, leading to wrong predictions by the IDSs.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
