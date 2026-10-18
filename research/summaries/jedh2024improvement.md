# Improvement and Evaluation of Resilience of Adaptive Cruise Control Against Spoofing Attacks Using Intrusion Detection System
Mubark Jedh, Lotfi ben Othmane, Arun K. Somani · Lecture Notes in Computer Science 2024 · https://doi.org/10.1007/978-3-031-61231-2_5 · relevance: medium · basis: full text

**Problem:** Speed spoofing on the CAN bus can make ACC crash into the lead vehicle.
**Approach:** ACC extended with a real-time ML intrusion detection system (ACC-IDS) that triggers braking.
**Tools and scale:** CARLA simulation; three speed/spoof cases (e.g., 40 km/h with 5 km/h spoofed speed).
**Results:** ACC-IDS mitigates speed spoofing attacks when the IDS fires before an imminent crash.
**Limitations and future work (as stated by the authors):**
- Future work: impact of vehicle speed, spoofed value, IDS success rate and response time on mitigation efficacy
- integrate real-time IDS into resilience mechanisms rather than estimation.
**Overlap with our engine:** none — none: in-vehicle CAN, not V2X.
**Novelty implications:** Nothing of our candidate directions is ruled out.
