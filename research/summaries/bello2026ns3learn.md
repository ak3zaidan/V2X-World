# NS3Learn: Transferring 5G NR Mode-2 Reception Realism from ns-3 to the Veins/SUMO Stack for Connected-Vehicle Safety Assessment
Rasheed Bello, Arthur Mukwaya, Gurcan Comert, Varghese Vaidyan, Vijay Bendigeri, Anthony Dontoh et al. · arXiv 2026 · https://arxiv.org/abs/2609.20578 · relevance: high · basis: full text (extracted sections)

**Problem:** Veins/SUMO safety evaluations ignore NR Mode-2 resource competition, overstating delivery in dense traffic.
**Approach:** NS3Learn: closed-form five-stage logistic cascade distilled from 10.5 million ns-3 5G-LENA reception outcomes (half-duplex, scheduling collisions, capture, decoding) inserted into Veins/SUMO.
**Tools and scale:** ns-3 5G-LENA calibrated on 3GPP scenarios, SUMO trajectories; two signalised urban networks, six penetration levels (1-100%), five seeds each.
**Results:** Mean absolute deviation 0.06 vs ns-3 (others 0.44 and 0.55); 5G-LENA delivery falls 0.952 to 0.188 with density; realistic models reversed simulated speed trends and more than doubled predicted hard-braking events; 20% extra error on a new intersection.
**Limitations and future work (as stated by the authors):**
- Offline mobility traces cannot capture feedback from communication to driving (noted for prior work).
**Overlap with our engine:** partial - shows radio-model realism changes traffic-safety outcomes in coupled simulation, which our joint traffic/NR-V2X engine targets natively; no security.
**Novelty implications:** Cannot claim to be first to show communication realism alters safety outcomes; the security-cost-on-safety (direction 2) remains open.
