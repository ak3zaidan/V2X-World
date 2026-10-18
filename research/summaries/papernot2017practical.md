# Practical Black-Box Attacks against Machine Learning
Nicolas Papernot, Patrick McDaniel, Ian Goodfellow, Somesh Jha et al. · Proceedings of the 2017 ACM on Asia Conference on Computer and Communications Security 2017 · https://doi.org/10.1145/3052973.3053009 · relevance: medium · basis: abstract

**Problem:** Machine learning (ML) models, e.g., deep neural networks (DNNs), are vulnerable to adversarial examples: malicious inputs modified to yield erroneous model outputs, while appearing unmodified to human observers. Potential attacks include having malicious content like malware identified as legitimate or controlling vehicle behavior.
**Approach:** We introduce the first practical demonstration of an attacker controlling a remotely hosted DNN with no such knowledge.
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** We find that their DNN misclassifies 84.24% of the adversarial examples crafted with our substitute. We demonstrate the general applicability of our strategy to many ML techniques by conducting the same attack against models hosted by Amazon and Google, using logistic regression substitutes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
