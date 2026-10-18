# Investigation of Feature Engineering Methods for Identifying Attacks in the VANET
Irina Bolodurina, Denis Parfenov, Lyubov Grishina · 2021 International Russian Automation Conference (RusAutoCon) 2021 · https://doi.org/10.1109/rusautocon52004.2021.9537464 · relevance: medium · basis: abstract

**Problem:** This article discusses the problem of increasing the efficiency of machine learning methods in identifying attacks in the VANET network by expanding the feature space using Feature Engineering methods.
**Approach:** The main idea of this work is to generate new features of a dataset using pre-trained models such as support vector machines for classification and Kmeans for clustering. The analysis of the efficiency of the generated features was carried out when solving the problem of identifying attacks using such machine learning methods as KNN, Random Forest, XGB, CatBoost, LGBM.
**Tools and scale:** Not stated in the abstract.
**Results:** Computational experiments showed that when SVM-based features were included, most ensemble machine learning methods improved accuracy by an average of 0.137% while adding a cluster number based on Kmeans resulted in an average efficiency improvement of 0.493%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
