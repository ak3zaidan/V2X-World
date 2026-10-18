# Mitigating Sybil Attacks in V2X Communication Through Cryptographic Trust Anchors
Maher Fayyad, Abdullah Awad, E. P. de Freitas, W. O. de Morais et al. · International Conference on Future Internet of Things and Cloud 2026 · https://doi.org/10.1109/ficloud70576.2026.00019 · relevance: high · basis: abstract

**Problem:** Pseudonyms enable Sybil.
**Approach:** SHA-256 PoW tokens plus time-windowed rate control on identity activation.
**Tools and scale:** Java parametric simulation, 8 vehicles.
**Results:** Difficulty 4 and 5 tokens per 10 s balance; 0.227 s mining; rate control is the main defence.
**Limitations and future work (as stated by the authors):**
- Assumes attackers are a minority.
**Overlap with our engine:** partial — pseudonym activation limits (SCMS enforces this by certificate validity windows).
**Novelty implications:** No impact.
