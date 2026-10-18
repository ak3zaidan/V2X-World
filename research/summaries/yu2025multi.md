# A Multi-Scenario Authenticated Key Exchange Scheme With Forward Secrecy for Fog-Enabled VANETs
Guosong Yu, Qiong Li, Haokun Mao, A. A. Abd El-Latif et al. · IEEE Transactions on Vehicular Technology 2025 · https://doi.org/10.1109/tvt.2024.3454472 · relevance: low · basis: abstract

**Problem:** However, their exclusive dependence on long-term secrets compromises forward secrecy, a critical security feature.
**Approach:** Recent research has introduced lightweight V2I AKE schemes that operate without relying on complex asymmetric cryptography. In response, a novel multi-party V2I AKE protocol based on the ASCON cryptographic algorithm is proposed.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** These schemes offer advantages in terms of reduced computational overhead and minimal communication latency. Pre-shared keys between entities are dynamically updated at the end of each communication session, achieving forward secrecy while maintaining computational efficiency compared to existing solutions.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
