# Fast 2-out-of-n ECDSA Threshold Signature
Lin Zhong, Yujue Wang, Jun Du, Daji Liang et al. · 2023 IEEE Intl Conf on Parallel &amp;amp; Distributed Processing with Applications, Big Data &amp;amp; Cloud Computing, Sustainable Computing &amp;amp; Communications, Social Computing &amp;amp; Networking (ISPA/BDCloud/SocialCom/SustainCom) 2023 · https://doi.org/10.1109/ispa-bdcloud-socialcom-sustaincom59178.2023.00092 · relevance: low · basis: abstract

**Problem:** However, schemes involving 2-outof$- n (2- n)$ participants have been comparatively underexplored.
**Approach:** To tackle this issue, we present an optimized extension of Li17’s two-party signature to facilitate fast $2- n$ ECDSA threshold signatures. During the Decentralized Key Generation (DKG) phase, we replace Li17’s inefficient range proof with a more streamlined version, which minimizes both proof size and computational demands.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Although one could theoretically employ $\mathrm{a}t-n$ threshold signature system and set $t =2$ to create a $2- n$ configuration, this workaround results in increased computational and communication overhead, thereby reducing signing speed. Unlike Li17’s two-party signature, which only precomputes part of r, our scheme can also precalculate a portion of s, thereby reducing computational requirements during the Sign-online phase.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
