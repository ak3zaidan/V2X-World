# A Vehicle Transmission Scheduling Scheme for Supporting Vehicle Trust Management
Qin-Wen Hu, Wanqing Tu · arXiv.org 2020 · https://arxiv.org/abs/2006.03798 · relevance: medium · basis: abstract

**Problem:** Recent developments in advanced sensors, wireless communications and intelligent vehicle control technologies have enabled vehicles to detect traffic anomalies on the road and then notify surrounding vehicles to improve traffic safety.
**Approach:** In this paper, we aim to support the improvement of credibility evaluation in vehicle networks by enhancing the transmission experience of vehicles. The proposed solution utilizes the vehicle's trajectory information, detection range and a roadside unit (RSU) coverage to form a controlled number of detection zones, which guarantees that events can be detected and reported while limiting the number of transmission vehicles.
**Tools and scale:** Our ns2 evaluation shows that our scheme can greatly reduce delay and loss rates of vehicle messages to help existing studies on accurate vehicle credibility evaluation.
**Results:** Recent developments in advanced sensors, wireless communications and intelligent vehicle control technologies have enabled vehicles to detect traffic anomalies on the road and then notify surrounding vehicles to improve traffic safety. These studies mostly focus on how to enhance the accuracy of credibility evaluation, overlooking that the transmission performance may affect the quality of vehicle messages.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
