# Detection of Multiple Small Biased GPS Spoofing Attacks on Autonomous Vehicles
Ahmad Mohammadi, Vahid Hemmati, Reza Ahmari, Frederick Owusu-Ambrose et al. · 2025 IEEE 4th International Conference on AI in Cybersecurity (ICAIC) 2025 · https://doi.org/10.1109/icaic63015.2025.10849307 · relevance: medium · basis: abstract

**Problem:** This paper introduces an algorithm designed to detect GPS spoofing attacks in Autonomous Driving Systems (ADS). The algorithm combines data from in-vehicle sensors, including the speedometer and gyroscope.
**Approach:** Additionally, the proposed algorithm is able to detect a sequence of multiple biased attacks that fall below the mentioned threshold but can create a large deviation between the position/displacement and speed provided by GPS and Inertial Measurement Unit (IMU). To validate the effectiveness of the work, datasets simulating four distinct spoofing scenarios such as turn-by-turn, overshoot, stop and multiple biased attacks were generated using real-world data from [1].
**Tools and scale:** Car-Hacking/CAN dataset, deep learning (mentioned in abstract)
**Results:** The analyses shows that the Data-driven model successfully identifies the turn-by-turn, stop, overshoot and multiple biased spoofing attacks with the accuracies of $100 \%$, $\mathbf{9 9. 9 5 \%}, \mathbf{9 9.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Addresses V2X attacks or misbehaviour/trust evaluation that our engine can execute and detect in closed loop; it does not model the SCMS, revocation or joint traffic/radio/PKI ground truth.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
