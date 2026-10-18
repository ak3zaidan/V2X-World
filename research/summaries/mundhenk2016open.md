# Open source model and simulator for real-time performance analysis of automotive network security
Philipp Mundhenk, Artur Mrowca, Sebastian Steinhorst, Martin Lukasiewycz, Suhaib A. Fahmy, Samarjit Chakraborty · ACM SIGBED Review 2016 · https://doi.org/10.1145/2983185.2983186 · relevance: high · basis: full text

**Problem:** Quantifying real-time impact of security (ECU authentication, message authorisation) in in-vehicle networks is hard at scale.
**Approach:** Open-source Python discrete-event model and simulator of secure automotive networks parameterised with hardware measurements.
**Tools and scale:** Python simulator (IVNS); in-vehicle buses (CAN etc.); up to 100 nodes; case study of a secure distributed battery management system.
**Results:** Networks of up to 100 nodes analysed in under 10 minutes with under 300 MB memory; architecture tweak increased case-study performance by close to an order of magnitude.
**Limitations and future work (as stated by the authors):**
- Integration into an OMNeT++ library left to future work.
**Overlap with our engine:** partial — simulates the timing cost of security, but in-vehicle not V2X/PKI.
**Novelty implications:** Timing cost of security in in-vehicle networks is done; V2X-level safety cost of security remains ours to check against V2X papers.
