# A Key Recovery Protocol for Multiparty Threshold ECDSA Schemes
Myungsun Kim, Sangrae Cho, Seongbong Choi, Young-Seob Cho et al. · IEEE Access 2022 · https://doi.org/10.1109/access.2022.3230683 · relevance: low · basis: abstract

**Problem:** Recently, threshold ECDSA schemes have received much attention from the security community, due to the need of efficient key management in the blockchain system.
**Approach:** In this paper, we provide a new key recovery protocol for threshold ECDSA schemes that is secure against static corruptions by malicious adversaries, as in the common adversary model of the state-of-the-art threshold ECDSA schemes. Our proposed protocol reduces both the computational and communication costs to $O(t^{2})$ from $O(t^{3})$ where $t$ is the threshold of the schemes, that is, the minimum number of users required for generating a valid signature.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** According to our experimental results, when $t=2$ with 128-bit security, while the previous result takes 10.46 ms in total for all computations (excluding the transmission time on the network), our protocol takes 4.21 ms, which improves by a factor of about 2.48 times. For example, when $t=9$ with 128-bit security, while the previous result requires 333.42 ms in total for all computations, our protocol requires 56.61 ms, which outperforms the previous result by a factor of about 5.89 times.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
