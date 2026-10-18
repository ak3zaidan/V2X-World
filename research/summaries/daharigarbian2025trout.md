# Trout: Two-Round Threshold ECDSA from Class Groups
Hila Dahari-Garbian, Ariel Nof, Luke Parker · Proceedings of the 2025 ACM SIGSAC Conference on Computer and Communications Security 2025 · https://doi.org/10.1145/3719027.3765192 · relevance: low · basis: abstract

**Problem:** We present Trout (Two-ROUnd Threshold), the first distributed two-round ECDSA signing protocol for arbitrary thresholds.
**Approach:** We achieve this without a trusted setup. To obtain our result, we leverage the recent construction of an exponent-VRF (Boneh et al., Eurocrypt 2025) and a novel protocol to multiply an encrypted value with a committed value and simultaneously decrypt it, which we call ''scaled decryption''.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our protocol has a very low communication cost of just 6.5 KB sent per party. Furthermore, we implemented our protocol in Rust and provide benchmarks for various configurations, showing its practicality even for 100 parties.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
