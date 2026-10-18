# Robust DoS Attack Detection for V2X Networks Using RNN Variants with Attention
Niranjan W. Meegammana, Harinda Fernando · 2025 7th International Conference on Advancements in Computing (ICAC) 2025 · https://doi.org/10.1109/icac69156.2025.11361446 · relevance: medium · basis: abstract

**Problem:** V2X communication is a foundational technology for connected and autonomous transportation, enabling vehicles, infrastructure, and pedestrians to exchange timely and reliable information essential for collision avoidance, traffic optimisation, and road safety.
**Approach:** Denial-of-Service (DoS) attacks pose critical threats to Vehicle-to-Everything (V2X) communication by disrupting real-time safety and mobility functions. This study presents a comparative evaluation of SimpleRNN, LSTM, and GRU models for DoS detection using a balanced, normalized dataset of $\mathbf{1 2 0, 0 0 0}$ samples derived from the VeReMi Extension.
**Tools and scale:** Mentioned in abstract: VEREMI, VeReMi
**Results:** Results show that GRU achieves the highest detection performance (F1-score up to 0.972, accuracy up to 97.2 %), RNN offers the smallest model size (0.09 MB) and fastest inference ($i 1 \mathrm{s}$), while LSTM provides high precision (up to 0.970) but requires tuning to improve recall.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
