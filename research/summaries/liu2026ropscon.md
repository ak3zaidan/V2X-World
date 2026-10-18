# ROPSCON: Linear BFT From Schnorr Threshold Signatures
Xuyang Liu, Zijian Zhang, Haibin Zhang, Jiamou Liu et al. · IEEE Transactions on Computers 2026 · https://doi.org/10.1109/tc.2026.3724280 · relevance: low · basis: abstract

**Problem:** Existing linear Byzantine fault-tolerant (BFT) protocols heavily rely on Threshold Signatures (TS) to reduce communication complexity. The threshold signatures used and implemented in these BFT protocols should be non-interactive and all such schemes rely on heavy pairing operations.
**Approach:** In this paper, we develop a novel approach that allows using threshold signatures like Schnorr TS in linear BFT protocols in an effective manner. In this approach, we can significantly reduce the interaction overhead caused by Schnorr TS while maintaining its computational efficiency.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** We instantiate a HotStuff-style BFT protocol called ROPSCON and show that ROPSCON achieves up to 2.2$\boldsymbol{\times}$× higher throughput compared to both HotStuff protocols using pairings as well as ones with directly aggregated signatures.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
