# Unstoppable Wallets: Chain-assisted Threshold ECDSA and its Applications
Guy Zyskind, Avishay Yanai, Alex Pentland · Proceedings of the 19th ACM Asia Conference on Computer and Communications Security 2024 · https://doi.org/10.1145/3634737.3637657 · relevance: low · basis: abstract

**Problem:** However, current approaches for managing these keys often rely on third parties, trusted to be available at a minimum, and even serve as custodians in some solutions, creating single points of failure and limiting the ability of users to fully control their own assets.
**Approach:** In this work we first revisit the problem of threshold ECDSA by considering the commonly admissible 'server-aided' model, namely, the presence of a semi-honest and non-colluding service provider. Then, we leverage that model and consider cases where that 'server' is distributed, introducing the novel concept of unstoppable wallets; hence eliminating any single point of failure.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** We construct highly efficient threshold ECDSA protocols that form the basis of unstoppable wallets and prove their security in the server-aided model, achieving the standard notion of fairness and robustness even in case of a dishonest majority among the signers. Our protocols minimize the write-complexity for threshold ECDSA key-generation and signing, while reducing communication and computation overhead.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
