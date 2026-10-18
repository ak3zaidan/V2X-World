# Evaluation of the Architecture Alternatives for Real-time Intrusion Detection Systems for Connected Vehicles
Mubark B Jedh, Jian Kai Lee, Lotfi ben Othmane · arXiv 2022 · https://arxiv.org/abs/2201.06680 · relevance: medium · basis: abstract

**Problem:** Attackers demonstrated the use of remote access to the in-vehicle network of connected vehicles to launch cyber-attacks and remotely take control of these vehicles.
**Approach:** Machine-learning-based Intrusion Detection Systems (IDSs) techniques have been proposed for the detection of such attacks. This paper evaluates four architecture designs for real-time IDS for connected vehicles using Controller Area Network (CAN) datasets collected from a moving vehicle under malicious speed reading message injections.
**Tools and scale:** Not stated in the abstract.
**Results:** Attackers demonstrated the use of remote access to the in-vehicle network of connected vehicles to launch cyber-attacks and remotely take control of these vehicles. The evaluation of some of these IDS demonstrated their efficacy in terms of accuracy in detecting message injections but was performed offline, which limits the confidence in their use for real-time protection scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
