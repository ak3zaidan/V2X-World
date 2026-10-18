# Central misbehavior evaluation for VANETs based on mobility data plausibility
Norbert Bißmeyer, Joël Njeukam, Jonathan Petit, Kpatcha M. Bayarou · Proceedings of the ninth ACM international workshop on Vehicular inter-networking, systems, and applications 2012 · https://doi.org/10.1145/2307888.2307902 · relevance: high · basis: abstract

**Problem:** Ghost-vehicle Sybil attackers must be identified and excluded permanently.
**Approach:** Central misbehaviour evaluation using trust and reputation from misbehaviour reports from vehicles and RSUs. (summarised from the abstract only)
**Tools and scale:** Simulation study.
**Results:** At least three benign witnesses suffice to significantly reduce the fake node's reputation; load scales with incidents not node count; colluding attacker case studied.
**Limitations and future work (as stated by the authors):**
- Assumes a majority of benign reporters.
**Overlap with our engine:** strong — central MA-style evaluation of misbehaviour reports leading to exclusion, the MA decision component of direction 1.
**Novelty implications:** Central report evaluation and witness thresholds are established; open: MA decisions feeding a real SCMS revocation pipeline with timing and safety outcomes.
