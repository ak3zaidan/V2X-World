# You Can't See Me: Physical Removal Attacks on LiDAR-based Autonomous Vehicles Driving Frameworks
Yu-Long Cao, S. H. Bhupathiraju, Pirouz Naghavi, Takeshi Sugawara et al. · USENIX Security Symposium 2022 · https://arxiv.org/abs/2210.09482 · relevance: medium · basis: abstract

**Problem:** Autonomous Vehicles (AVs) increasingly use LiDAR-based object detection systems to perceive other vehicles and pedestrians on the road. While existing attacks on LiDAR-based autonomous driving architectures focus on lowering the confidence score of AV object detection models to induce obstacle misdetection, our research discovers how to leverage laser-based spoofing techniques to selectively remove the LiDAR point ...
**Approach:** In this paper, we present a method invisible to the human eye that hides objects and deceives autonomous vehicles' obstacle detectors by exploiting inherent automatic transformation and filtering processes of LiDAR sensor data integrated with autonomous driving frameworks. We evaluate the attack impact on three fusion models (Frustum-ConvNet, AVOD, and Integrated-Semantic Level Fusion) and the consequences on the ...
**Tools and scale:** We evaluate the attack impact on three fusion models (Frustum-ConvNet, AVOD, and Integrated-Semantic Level Fusion) and the consequences on the driving decision using LGSVL, an industry-grade simulator.
**Results:** We call such attacks Physical Removal Attacks (PRA), and we demonstrate their effectiveness against three popular AV obstacle detectors (Apollo, Autoware, PointPillars), and we achieve 45{\deg} attack capability. In our moving vehicle scenarios, we achieve a 92.7% success rate removing 90\% of a target obstacle's cloud points.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
