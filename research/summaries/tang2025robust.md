# Robust Threshold ECDSA with Online-Friendly Design in Three Rounds
Guofeng Tang, Haiyang Xue · 2025 IEEE Symposium on Security and Privacy (SP) 2025 · https://doi.org/10.1109/sp61157.2025.00115 · relevance: low · basis: abstract

**Problem:** Threshold signatures, especially ECDSA, enhance key protection by addressing the single-point-of-failure issue.
**Approach:** The state-of-the-art online-friendly threshold ECDSA with-out robustness was developed by Doerner et al. in S&P'24, requiring only three rounds. This paper presents the first three-round threshold ECDSA scheme with both robustness and an online-friendly design.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Recent work by Wong et al. in NDSS'23 (WMY+23) and NDSS'24 (WMC24) achieves robustness but demands additional communication rounds (7 and 4, respectively) or incurs costly operations in the online phase, such as computations over a homomorphic encryption scheme. The online phase of our scheme relies solely on several elliptic-curve group operations, which are 2 to 3 orders of magnitude less computationally intensive than those based on linearly homomorphic encryption schemes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
