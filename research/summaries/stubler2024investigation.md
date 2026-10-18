# An Investigation of Denial of Service Attacks on Autonomous Driving Software and Hardware in Operation
Tillmann Stübler, A. Amodei, Domenico Capriglione, G. Tomasso, Nicolas Bonnotte, Shawan Mohammed · 2022 IEEE 18th International Conference on Automation Science and Engineering (CASE) 2024 · https://doi.org/10.1109/case59546.2024.10711339 · relevance: medium · basis: abstract

**Problem:** This research investigates the impact of Denial of Service (DoS) attacks, specifically Internet Control Message Protocol (ICMP) flood attacks, on Autonomous Driving (AD) systems, focusing on their control modules.
**Approach:** Two experimental setups were created: the first involved an ICMP flood attack on a Raspberry Pi running an AD software stack, and the second examined the effects of single and double ICMP flood attacks on a Global Navigation Satellite System Real-Time Kinematic (GNSS-RTK) device for high-accuracy localization of an autonomous vehicle that is available on the market. The results indicate a moderate impact of DoS attacks on the AD stack, where the increase in median computation time was marginal, suggesting a degree of resilience to these types of attacks.
**Tools and scale:** Not stated in the abstract.
**Results:** The results indicate a moderate impact of DoS attacks on the AD stack, where the increase in median computation time was marginal, suggesting a degree of resilience to these types of attacks. In contrast, the GNSS device demonstrated significant vulnerability: during DoS attacks, the sample rate dropped drastically to approximately 50% and 5% of the nominal rate for single and double attacker configurations, respectively.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
