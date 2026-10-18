# Compact LLM Deployment and World Model Assisted Offloading in Mobile Edge Computing
Ruichen Zhang, Xiaofeng Luo, Jiayi He, Jiawen Kang, Zehui Xiong, Shiwen Mao · arXiv 2026 · https://arxiv.org/abs/2602.13628 · relevance: low · basis: abstract

**Problem:** This paper investigates compact large language model (LLM) deployment and world-model-assisted inference offloading in mobile edge computing (MEC) networks.
**Approach:** We first propose an edge compact LLM deployment (ECLD) framework that jointly applies structured pruning, low-bit quantization, and knowledge distillation to construct edge-deployable LLM variants, and we evaluate these models using four complementary metrics: accessibility, energy consumption, hallucination rate, and generalization accuracy. Building on the resulting compact models, we formulate an MEC offloading optimization problem that minimizes the long-term average inference latency subject to per-device energy budgets and LLM-specific quality-of-service constraints on effective accuracy and hallucination.
**Tools and scale:** Not stated in the abstract.
**Results:** To solve this problem under unknown and time-varying network dynamics, we develop a world model-proximal policy optimization (PPO) algorithm, which augments an on-policy PPO algorithm with a learned recurrent world model that provides improved value targets and short imagination rollouts. Extensive experiments on Llama-3.1-8B, Qwen3-8B, and Mistral-12B show that ECLD compresses base models by about 70-80% in storage (i.e., from 15.3 GB to 3.3 GB for Llama-3.1-8B) and reduces per-query energy consumption by up to 50%, while largely preserving accuracy and often lowering hallucination compared with quantization-only or pruning-only baselines.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches world models, LLM agents but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on world models.
