# Bandwidth-Efficient Robust Threshold ECDSA in Three Rounds
Yingjie Lyu, Zengpeng Li, Hong-Sheng Zhou, Haiyang Xue et al. · IEEE Transactions on Information Forensics and Security 2026 · https://doi.org/10.1109/tifs.2026.3682606 · relevance: low · basis: abstract

**Problem:** However, most prior protocols are not robust, wherein even one misbehaving or non-responsive party would mandate an abort.
**Approach:** Robust schemes have been proposed (Wong et al., NDSS ’23, ’24), but they do not match state-of-the-art number of rounds which is only three (Doerner et al., S&P ’24). In this work, we propose robust threshold ECDSA schemes RompSig-Q and RompSig-L that each take three rounds (where the first two are broadcasts, whereas the non-robust scheme of Doerner et al. uses no broadcasts).
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Building on the works of Wong et al. and further optimized towards saving bandwidth, they respectively take each signer $(1.0t+1.6)$ KiB and 3.0 KiB outbound broadcast communication, and thus exhibit bandwidth efficiency that is competitive in practical scenarios where broadcasts are natively handled.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
