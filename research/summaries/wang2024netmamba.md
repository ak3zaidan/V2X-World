# Netmamba: Efficient Network Traffic Classification Via Pre-Training Unidirectional Mamba
Tong-Ze Wang, Xiao-Hui Xie, Wen-Duo Wang, Chu-Yi Wang et al. · IEEE International Conference on Network Protocols 2024 · https://doi.org/10.1109/icnp61940.2024.10858569 · relevance: medium · basis: abstract

**Problem:** Network traffic classification is a crucial research area aiming to enhance service quality, streamline network management, and bolster cybersecurity. To address the growing complexity of transmission encryption techniques, various machine learning and deep learning methods have been proposed.
**Approach:** To address these challenges, we propose NetMamba, an efficient linear-time state space model equipped with a comprehensive traffic representation scheme. We adopt a specially selected and improved unidirectional Mamba architecture for the networking field, instead of the Transformer, to address efficiency issues.
**Tools and scale:** Mentioned in abstract: field. (From the abstract only.)
**Results:** It achieves an accuracy rate of nearly $99 \%$ (some over $99 \%$) in all tasks. Additionally, NetMamba demonstrates excellent efficiency, improving inference speed by up to 60 times while maintaining comparably low memory usage. Furthermore, NetMamba exhibits superior few-shot learning abilities, achieving better classification performance with fewer labeled data.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for network traffic); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
