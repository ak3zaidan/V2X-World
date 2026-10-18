# AIR-CAV: AI-assisted Reliable Channel Selection for Connected Autonomous Vehicles
Md Tajul Islam, Sejun Song, Baek-Young Choi · 2025 IEEE Annual Congress on Artificial Intelligence of Things (AIoT) 2025 · https://doi.org/10.1109/aiot66900.2025.00037 · relevance: low · basis: abstract

**Problem:** Traditionally, vehicles relied on two separate networks—the Car Area Network (CAN) and the infotainment network.
**Approach:** This paper introduces AIR-CAV, a novel AI-assisted reliable channel selection framework for CAVs. In addition to using a real CAV dataset for analysis, we developed a simulation environment to model Vehicle-to-Vehicle (V2V) communication channels and evaluated several machine learning models—Exponentially Weighted Moving Average (EWMA), Feedforward Neural Network (FFNN), AlexNet, and Convolutional Neural Network (CNN)—for Signal to Noise Ratio (SNR) prediction of different communication networks.
**Tools and scale:** Mentions: CAN, CNN.
**Results:** Experimental results show that ML- based methods can effectively identify reliable channels in diverse settings. Among them, CNN achieved the best performance with a Mean Absolute Error (MAE) of 0.069, outperforming other models and baseline predictors.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
