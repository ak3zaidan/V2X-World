# Optimizing Threshold Secret Sharing with Hierarchical Access Structures in ECDSA
Aditya S. L. Sumule, Mareta Wahyu Ardyani, Nadia Paramita Retno Adiati, Santi Indarjani · 2024 IEEE International Conference on Internet of Things and Intelligence Systems (IoTaIS) 2024 · https://doi.org/10.1109/iotais64014.2024.10799352 · relevance: low · basis: abstract

**Problem:** The (t, n) threshold multi-secret sharing scheme based on Verifiable Secret Sharing (VSS) is a well-known method, but it typically uses a uniform access structure, which limits its application in organizations with hierarchical access needs.
**Approach:** While hierarchical schemes often rely on a tree structure, this research explores a model where share values at each level directly relate to the secret.In this study, we employed three (t, n) threshold secret sharing schemes on the Elliptic Curve Digital Signature Algorithm (ECDSA): one without a hierarchical access structure and two with hierarchical access structure.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Performance analysis based on computation time and time complexity shows that hierarchical schemes outperform non-hierarchical one, as they reduce the polynomial degree into smaller hierarchical levels.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
