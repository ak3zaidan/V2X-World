# Network-Efficient World Model Token Streaming
Shatadal Mishra, Ahmadreza Moradipari, Nejib Ammar · 2026 IEEE Vehicular Networking Conference (VNC) 2026 · https://doi.org/10.1109/vnc69225.2026.11629104 · relevance: low · basis: abstract

**Problem:** Generative driving world models rely on compact latent state representations that must be efficiently transmitted and synchronized across distributed compute and connected vehicles.
**Approach:** We study network-efficient streaming of a discrete world model state, where a stride-16 VQ-U-Net tokenizer (codebook size 8,192) maps each 288x512 frame to an 18x32 grid of token IDs (576 tokens/frame), equivalent to 936 bytes/frame under fixed-length coding. To connect state fidelity to world model usefulness, we train a lightweight next-token predictor and evaluate perplexity conditioned on streamed receiver states: at 0.024 Mb/s, dynamic-position perplexity improves from 206.0 to 193.1 (6.3\%), and at 0.036 Mb/s from 158.9 to 155.6 (2.1\%).
**Tools and scale:** Not stated in the abstract.
**Results:** The adaptive algorithm consistently improves the rate distortion frontier over periodic keyframes at matched bitrates: at 0.024 Mb/s (200-byte budget) dynamic-only embedding distortion drops from 0.0712 to 0.0661 (7.2\%), and at 0.036 Mb/s (400-byte budget) from 0.0427 to 0.0407 (4.8\%). These results support discrete token-state streaming as a practical systems layer for bandwidth-aware synchronization and improved downstream token-dynamics utility under vehicular networking constraints.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
