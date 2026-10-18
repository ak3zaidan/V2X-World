# Digital Twin-Assisted Cooperative Driving at Non-Signalized Intersections
Ziran Wang, Kyungtae Han, Prashant Tiwari · arXiv 2021 · https://doi.org/10.1109/tiv.2021.3100465 · relevance: medium · basis: abstract

**Problem:** Digital Twin, as an emerging technology related to Cyber-Physical Systems (CPS) and Internet of Things (IoT), has attracted increasing attentions during the past decade.
**Approach:** Conceptually, a Digital Twin is a digital replica of a physical entity in the real world, and this technology is leveraged in this study to design a cooperative driving system at non-signalized intersections, allowing connected vehicles to cooperate with each other to cross intersections without any full stops. Within the proposed Digital Twin framework, we developed an enhanced first-in-first-out (FIFO) slot reservation algorithm to schedule the sequence of crossing vehicles, a consensus motion control algorithm to calculate vehicles' referenced longitudinal motion, and a model-based motion estimation algorithm to tackle communication delay and packet loss.
**Tools and scale:** Mentions: real world, real-world; scale not stated in the abstract.
**Results:** Agent-based modeling and simulation of the proposed system is conducted in Unity game engine based on a real-world map in San Francisco, and the human-in-the-loop (HITL) simulation results prove the benefits of the proposed algorithms with 20% reduction in travel time and 23.7% reduction in energy consumption, respectively, when compared with traditional signalized intersections.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
