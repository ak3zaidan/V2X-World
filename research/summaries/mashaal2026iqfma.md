# IQFM—A Wireless Foundation Model for I / Q Streams in AI-Native 6G
Omar Mashaal, Hatem Abou-Zeid · IEEE Open Journal of the Communications Society 2026 · https://doi.org/10.1109/ojcoms.2026.3661435 · relevance: medium · basis: abstract

**Problem:** Foundation models have shown remarkable potential in natural language processing and computer vision, yet remain in their infancy in wireless communications. While a few efforts have explored image-based modalities such as channel state information (CSI) and frequency spectrograms, foundation models operating directly on raw in-phase/quadrature (IQ) signals remain largely unexplored.
**Approach:** This paper presents IQFM, the first foundation model for wireless communications built on raw multi-antenna IQ data. We introduce a task-aware augmentation strategy that categorizes transformations into core augmentations, such as cyclic time shifting, and task-specific augmentations, enabling structured representation learning within a contrastive self-supervised framework.
**Tools and scale:** Mentioned in abstract: 6G. (From the abstract only.)
**Results:** Using this approach, a lightweight encoder pre-trained on over-the-air multi-antenna IQ data achieves 99.67% accuracy on modulation classification and 65.45% on AoA classification with only one labeled sample per class through linear probing, far outperforming supervised baselines. These results establish raw IQ-based foundation models as efficient, reusable encoders for multi-task learning in AI-native 6G systems.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
