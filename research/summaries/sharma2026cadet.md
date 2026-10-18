# CADET: A Modular Platform for Evaluating Distributed Cooperative Autonomy in Connected Autonomous Vehicles
Pragya Sharma, Brian Wang, Mani Srivastava · ICRA 2026 2026 · https://doi.org/10.1109/icra57385.2026.11696961 · relevance: medium · basis: abstract

**Problem:** Deep learning models are increasingly central to autonomous vehicle (AV) pipelines, yet their integration has traditionally followed a monolithic design where perception, planning, and control execute on a single onboard computer.
**Approach:** This design overlooks the emerging paradigm of cooperative autonomy, where vehicles interact with roadside units (RSUs), edge servers, and cloud-hosted intelligence through vehicle-to-everything (V2X) connectivity. Cooperative perception and control improve safety and efficiency, but also introduce systems-level challenges: network latency, compute heterogeneity, and multi-tenant contention, all critically affect real-time decision-making.
**Tools and scale:** Mentions: dataset; scale not stated in the abstract.
**Results:** Through V2V and V2I experiments, we show that distributed deployment choices fundamentally shape safety, with V2V intent packets outperforming cloud-based perception and RSU-assisted perception sustaining safety until overloaded by concurrent requests.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
