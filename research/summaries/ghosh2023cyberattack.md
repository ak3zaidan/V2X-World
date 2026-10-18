# A Cyberattack Detection-Isolation Algorithm for CAV Under Changing Driving Environment
Sanchita Ghosh, Nutan Saha, Tanushree Roy · IEEE transactions on intelligent transportation systems (Print) 2023 · https://doi.org/10.1109/tits.2024.3475952 · relevance: medium · basis: abstract

**Problem:** Under a changing driving environment, a Connected Autonomous Vehicle (CAV) platoon relies strongly on the acquisition of accurate traffic information from neighboring vehicles as well as reliable commands from a centralized supervisory controller through the communication network. Even though such modalities are imperative to ensure the safe and efficient driving performance of CAVs, they led to multiple security challenges.
**Approach:** Hence, in this paper, we propose a cyberattack detection-isolation algorithm comprised of a unified V2V and V2I cyberattack detection scheme along with a V2I isolation scheme for CAVs under changing driving conditions. The proposed algorithm is constructed using a bank of residual generators with Lyapunov function-based performance guarantees, such as disturbance-to-state stability, robustness, and sensitivity.
**Tools and scale:** Car-Hacking/CAN dataset (mentioned in abstract)
**Results:** Finally, we showcase the efficacy of our proposed algorithm through extensive Monte-Carlo simulations using real-world highway and urban driving data. The results show that the proposed algorithm can enhance the cybersecurity of CAVs by detecting cyberattacks on CAV platoons and isolating infrastructure-level traffic manipulation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Addresses V2X attacks or misbehaviour/trust evaluation that our engine can execute and detect in closed loop; it does not model the SCMS, revocation or joint traffic/radio/PKI ground truth.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
