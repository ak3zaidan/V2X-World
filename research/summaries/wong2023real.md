# Real Threshold ECDSA
Harry W. H. Wong, Jack P. K. Ma, Hoover H. F. Yin, Sherman S. M. Chow · Proceedings 2023 Network and Distributed System Security Symposium 2023 · https://doi.org/10.14722/ndss.2023.24817 · relevance: low · basis: abstract

**Problem:** To mitigate such denial-of-service vulnerabilities prevalent in state-of-the-art, we propose a robust threshold ECDSA scheme that achieves the t -out-of- n threshold flexibility “for real” throughout the whole pre-signing and signing phases without assuming an honest majority.
**Approach:** Our design features 4 - round pre-signing, O ( n ) cheating identification, and self-healing machinery over distributive shares.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Prior arts mandate abort after an O ( n 2 ) -cost identification, albeit with 3 -round pre-signing (Canetti et al., CCS ’20), or O ( n ) using 6 rounds (Castagnos et al., TCS ’23). Empirically, our scheme saves up to ∼ 30% of the communication cost, depending on at which stage the fault occurred.
**Limitations and future work (as stated by the authors):**
- "Latest (communication-optimizing) schemes often assume all n or at least n ′ ≥ t participating users remain honest throughout the pre-signing phase, essentially degenerating to n ′ -out-of- n ′ multiparty signing instead of t -out-of- n threshold signing."
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
