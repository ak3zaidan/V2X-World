# Lightweight LLMs for Network Attack Detection in IoT Networks
Piyumi Bhagya Sudasinghe, Kushan Sudheera Kalupahana Liyanage, Harsha S. Gardiyawasam Pussewalage · arXiv 2026 · https://arxiv.org/abs/2601.15269 · relevance: medium · basis: abstract

**Problem:** The rapid growth of Internet of Things (IoT) devices has increased the scale and diversity of cyberattacks, exposing limitations in traditional intrusion detection systems. Classical machine learning (ML) models such as Random Forest and Support Vector Machine perform well on known attacks but require retraining to detect unseen or zero-day threats.
**Approach:** This study investigates lightweight decoder-only Large Language Models (LLMs) for IoT attack detection by integrating structured-to-text conversion, Quantized Low-Rank Adaptation (QLoRA) fine-tuning, and Retrieval-Augmented Generation (RAG).
**Tools and scale:** Experiments on the CICIoT2023 dataset show that a QLoRA-tuned LLaMA-1B model achieves an F1-score of 0.7124, comparable to the Random Forest (RF) baseline (0.7159) for known attacks.
**Results:** Experiments on the CICIoT2023 dataset show that a QLoRA-tuned LLaMA-1B model achieves an F1-score of 0.7124, comparable to the Random Forest (RF) baseline (0.7159) for known attacks. With RAG, the system attains 42.63% accuracy on unseen attack types without additional training, demonstrating practical zero-shot capability.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
