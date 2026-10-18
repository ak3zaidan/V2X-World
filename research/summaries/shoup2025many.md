# The many faces of Schnorr: a toolkit for the modular design of threshold Schnorr signatures
Victor Shoup · IACR Communications in Cryptology 2025 · https://doi.org/10.62056/ahsgvurzn · relevance: low · basis: abstract

**Problem:** Recently, a number of highly optimized threshold signing protocols for Schnorr signatures have been proposed.
**Approach:** While these proposals contain important new techniques, some of them present and analyze these techniques in very specific contexts, making it less than obvious how these techniques can be adapted to other contexts, or combined with one another. The main goal of this paper is to abstract out and extend in various ways some of these techniques, building a toolbox of techniques that can be easily combined in different ways and in different contexts.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** To this end, we present security results for various “enhanced” modes of attack on the Schnorr signature scheme in the non-distributed setting, and we demonstrate how to reduce the security in the distributed threshold setting to these enhanced modes of attack in the non-distributed setting.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
