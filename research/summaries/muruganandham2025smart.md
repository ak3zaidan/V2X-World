# SMART: Sim2Real Meta-Learning-Based Training for mmWave Beam Selection in V2X Networks
Divyadharshini Muruganandham, Suyash Pradhan, Jerry Gu, Torsten Braun et al. · IEEE Transactions on Mobile Computing 2025 · https://doi.org/10.1109/tmc.2025.3576203 · relevance: medium · basis: abstract

**Problem:** Digital twins (DT) offer a low-overhead evaluation platform and the ability to generate rich datasets for training machine learning (ML) models before actual deployment. Specifically, for the scenario of ML-aided millimeter wave (mmWave) links between moving vehicles to roadside units, we show how DT can create an accurate replica of the real world for model training and testing.
**Approach:** The contributions of this paper are twofold: First, we propose a framework to create a multimodal Digital Twin (DT), where synthetic images and LiDAR data for the deployment location are generated along with RF propagation measurements obtained via ray-tracing. Second, to ensure effective domain adaptation, we leverage meta-learning, specifically Model-Agnostic Meta-Learning (MAML), with transfer learning (TL) serving as a baseline validation approach.
**Tools and scale:** Mentioned in abstract: digital twin, real-world. (From the abstract only.)
**Results:** The proposed framework is validated using a comprehensive dataset containing both real and synthetic LiDAR and image data for mmWave V2X beam selection. Experimental results show that models trained on synthetic data using transfer learning and meta-learning, followed by minimal fine-tuning with real-world data, achieve up to 4.09× and 14.04× improvements in accuracy, respectively.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (learned prediction of V2X/vehicular link or QoS performance); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
