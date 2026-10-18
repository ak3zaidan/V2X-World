# A fast co-simulation approach to vehicle/track interaction with finite element models of S&C
Demeng Fan, Michel Sebès, Emmanuel Bourgeois, Hugues Chollet, Cédric Pozzolini · arXiv 2021 · https://arxiv.org/abs/2110.04062 · relevance: low · basis: abstract

**Problem:** Simulations of vehicle/track interaction (VTI) in switches and crossings (S\&C) require taking into account the complexity of their geometry.
**Approach:** The VTI can be handled via a co-simulation process between a finite element (FE) model of the track and a multibody system (MBS) software. The objective of this paper is to reduce the computing effort in the co-simulation process.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** A good agreement is found between both approaches with a reduction of the computing time by a factor of 10.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
