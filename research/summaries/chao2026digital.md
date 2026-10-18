# Digital-Twin Losses for Lane-Compliant Trajectory Prediction at Urban Intersections
Kuo-Yi Chao, Erik Leo Haß, Melina Gegg, Jiajie Zhang, Ralph Raßhofer, Alois Christian Knoll · arXiv 2026 · https://arxiv.org/abs/2603.05546 · relevance: low · basis: abstract

**Problem:** Accurate and safety-conscious trajectory prediction is a key technology for intelligent transportation systems, especially in V2X-enabled urban environments with complex multi-agent interactions.
**Approach:** In this paper, we created a digital twin-driven V2X trajectory prediction pipeline that jointly leverages cooperative perception from vehicles and infrastructure to forecast multi-agent motion at signalized intersections. The proposed model combines a Bi-LSTM-based generator with a structured training objective consisting of a standard mean squared error (MSE) loss and a novel twin loss.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** In this paper, we created a digital twin-driven V2X trajectory prediction pipeline that jointly leverages cooperative perception from vehicles and infrastructure to forecast multi-agent motion at signalized intersections. We train and evaluate our approach on real-world V2X data sent from the intersection to the vehicle and collected in urban corridors.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
