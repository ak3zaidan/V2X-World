# An Edge Computing Framework for Deep Learning-Based Anomaly Detection in Satellite-Linked Autonomous Vehicles
Mehedi Hasan, Maumita Bhattacharya, Md. Rafiqul Islam · The Web Conference 2026 · https://doi.org/10.1145/3774905.3794678 · relevance: medium · basis: abstract

**Problem:** Modern satellite-linked autonomous vehicles require real-time anomaly detection to address multi-layer cyber threats across CAN Bus, V2X communication, and sensor systems.
**Approach:** We present MARIS-ADS, a constraint-aware multi-modal anomaly detection framework for edge deployment.
**Tools and scale:** Not stated in the abstract.
**Results:** The system employs attention-based fusion of 41 CAN, 37 V2X, and 28 sensor features, applying entropy-based selection (ARS) to reduce dimensionality 83% while achieving 99.3% accuracy. Constraint validation enforcing SAE J2735, ISO 11898, and vehicle dynamics constraints filters 82% of adversarial attacks as protocol-invalid or physically impossible.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
