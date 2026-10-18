# RF-GPT: Teaching AI to See the Wireless World
Han Zou, Yu Tian, Bo-Hao Wang, Lina Bariah et al. · IEEE Transactions on Wireless Communications 2026 · https://arxiv.org/abs/2602.14833 · relevance: medium · basis: abstract

**Problem:** Large language models (LLMs) and multimodal models have become powerful general-purpose reasoning systems. However, radio-frequency (RF) signals, which underpin wireless systems, are still not natively supported by these models.
**Approach:** To bridge this gap, we introduce RF-GPT, a radio-frequency language model (RFLM) that utilizes the visual encoders of multimodal LLMs to process and understand RF spectrograms. To train RF-GPT, we perform supervised instruction fine-tuning of a pretrained multimodal LLM using a fully synthetic RF corpus.
**Tools and scale:** Mentioned in abstract: 5G, LLM, GPT. (From the abstract only.)
**Results:** A text-only LLM then converts these captions into RF-grounded instruction-answer pairs, yielding roughly 12,000 RF scenes and 0.625 million instruction examples without any manual labeling. Across benchmarks for wideband modulation classification, overlap analysis, wireless-technology recognition, WLAN user counting, and 5G NR information extraction, RF-GPT achieves strong multi-task performance, whereas general-purpose VLMs with no RF grounding largely fail.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
