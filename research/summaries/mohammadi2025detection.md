# Detection of Multiple Small Biased GPS Spoofing Attacks on Autonomous Vehicles Using Time Series Analysis
Ahmad Mohammadi, Reza Ahmari, Vahid Hemmati, Frederick Owusu-Ambrose et al. · IEEE Open Journal of Vehicular Technology 2025 · https://doi.org/10.1109/ojvt.2025.3559461 · relevance: medium · basis: abstract

**Problem:** This research introduces an algorithm to identify GPS spoofing attacks in Autonomous Vehicles (AV). It uses data from onboard sensors such as speedometers and gyroscopes, which are integrated and analyzed using a Neural Network (NN).
**Approach:** To evaluate the algorithm's effectiveness, five different test datasets depicting four types of spoofing scenarios—turn-by-turn, overshoot, stop, and multiple small biased attacks—were created using data from the publicly accessible Honda Research Institute Driving Dataset (HDD).
**Tools and scale:** Car-Hacking/CAN dataset, deep learning (mentioned in abstract)
**Results:** The analysis shows that the model accurately detects these types of attacks with average accuracies of 98.62<inline-formula><tex-math notation="LaTeX">$\pm$</tex-math></inline-formula>1%, 99.96<inline-formula><tex-math notation="LaTeX">$\pm$</tex-math></inline-formula>0.1%, 99.88<inline-formula><tex-math notation="LaTeX">$\pm$</tex-math></inline-formula>0.1%, and 95.92<inline-formula><tex-math notation="LaTeX">$\pm$</tex-math></inline-formula>1.7
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Addresses V2X attacks or misbehaviour/trust evaluation that our engine can execute and detect in closed loop; it does not model the SCMS, revocation or joint traffic/radio/PKI ground truth.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
