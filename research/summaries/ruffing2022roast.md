# ROAST: Robust Asynchronous Schnorr Threshold Signatures
Tim Ruffing, Viktoria Ronge, Elliott Jin et al. · Proceedings of the 2022 ACM SIGSAC Conference on Computer and Communications Security 2022 · https://doi.org/10.1145/3548606.3560583 · relevance: low · basis: abstract

**Problem:** Bitcoin and other cryptocurrencies have recently introduced support for Schnorr signatures whose cleaner algebraic structure, as compared to ECDSA, allows for simpler and more practical constructions of highly demanded ''t-of-n'' threshold signatures.
**Approach:** In this work, we propose ROAST, a simple wrapper that turns a given threshold signature scheme into a scheme with a robust and asynchronous signing protocol, as long as the underlying signing protocol is semi-interactive (i.e., has one preprocessing round and one actual signing round), provides identifiable aborts, and is unforgeable under concurrent signing sessions. When applied to the state-of-the-art Schnorr threshold signature scheme FROST, which fulfills these requirements, we obtain a simple, efficient, and highly practical Schnorr threshold signature scheme.
**Tools and scale:** Not stated in the abstract.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
