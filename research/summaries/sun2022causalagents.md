# CausalAgents: A Robustness Benchmark for Motion Forecasting
Li-Ting Sun, R. Roelofs, Ben Caine, Khaled S. Refaat, Benjamin Sapp, S. Ettinger, Wei Chai · IEEE International Conference on Robotics and Automation 2022 · https://doi.org/10.1109/icra57147.2024.10610186 · relevance: low · basis: abstract

**Problem:** As machine learning models become increasingly prevalent in motion forecasting for autonomous vehicles (AVs), it is critical to ensure that model predictions are safe and reliable.
**Approach:** In this paper, we examine the robustness of motion forecasting to non-causal perturbations. We construct a new benchmark for evaluating and improving model robustness by applying perturbations to existing data.
**Tools and scale:** Mentioned in abstract: dataset.
**Results:** We evaluate a diverse set of state-of-the-art deep-learning models on our proposed benchmark and find that all evaluated models exhibit large shifts under non-causal perturbation: we observe a surprising 25-38% relative change in minADE as compared to the original. In addition, we investigate techniques to improve model robustness, including increasing the training dataset size and using targeted data augmentations that randomly drop non-causal agents throughout training.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches vehicular networking but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on vehicular networking.
