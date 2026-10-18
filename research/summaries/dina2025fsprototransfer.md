# FSProtoTransfer: Synergizing Few-Shot, Prototypical Networks, and Transfer Learning for Intrusion Detection in VANETs
Ayesha Siddiqua Dina, Colby Edell, Karim O. Elish, Arijet Sarker · Proceedings of the ... International Florida Artificial Intelligence Research Society Conference 2025 · https://doi.org/10.32473/flairs.38.1.138952 · relevance: medium · basis: abstract

**Problem:** The Internet of Vehicles (IoV) integrates Vehicular Ad Hoc Networks (VANETs) to enhance the safety and efficiency of vehicular transportation.
**Approach:** However, the distinct characteristics of IoV networks make them vulnerable to a range of attacks that compromise their confidentiality and functionality. To address these threats, Intrusion Detection Systems (IDSs) have been widely proposed as countermeasures.
**Tools and scale:** Not stated in the abstract.
**Results:** In the second phase, few-shot learning with a prototypical network is applied to train the pre-trained model, enabling it to learn effectively from a small number of labeled examples and reducing the reliance on large labeled datasets. Our experimental results demonstrate that FSProtoTransfer, utilizing only 20% of labeled data, outperforms fully supervised state-of-the-art models by 41%, 78% and 70% in terms of precision, recall, and F1 score, respectively.
**Limitations and future work (as stated by the authors):**
- However, the distinct characteristics of IoV networks make them vulnerable to a range of attacks that compromise their confidentiality and functionality.
- However, traditional ML/DL models rely heavily on large, labeled datasets for training, which are both costly and time-consuming to obtain.
- The challenge is further compounded by class imbalance within datasets, which skews the model's learning toward majority classes, leading to poor detection of attacks from minority classes.
- This challenge is particularly significant in VANETs environments, where the unique topology and diverse communication methods increase susceptibility to various cyberattacks.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
