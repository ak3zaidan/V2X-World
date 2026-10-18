# A Field-Scale AI Surrogate: Graph Network Simulator for Soil Moisture Prediction
Tamima Rashid, Balasubramaniam Natarajan · unknown venue 2026 · https://doi.org/10.2139/ssrn.7075247 · relevance: low · basis: abstract

**Problem:** Accurate field-scale volumetric water content (VWC) prediction is essential for precision irrigation management.
**Approach:** However, Richards-equation solvers such as HYDRUS-1D provide vertical 1D flow within a single soil column and do not represent spatial distribution across a field. In this paper, we demonstrate the feasibility of an AI surrogate simulator to obtain field-scale VWC estimates.
**Tools and scale:** Non-vehicular or unspecified setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** However, Richards-equation solvers such as HYDRUS-1D provide vertical 1D flow within a single soil column and do not represent spatial distribution across a field. Specifically, we use a process-based hydrology model as a physics engine to generate synthetic training data (here HYDRUS-1D, though the framework is agnostic to the choice of simulator), and we propose and train a Graph Network Simulator (GNS) on these simulations, evaluating it ...
**Limitations and future work (as stated by the authors):**
- Among adaptations on observed data, multi-step unrolled fine-tuning on in-situ sensors is the only strategy that improves rollouts, whereas coarse-resolution satellite soil moisture measurement does not; the remaining accuracy ceiling is HYDRUS-1D&amp;apos;s own structural bias at depth, ...
**Overlap with our engine:** none: simulation/digital twins outside V2X. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
