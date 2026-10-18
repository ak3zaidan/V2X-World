# Leveraging Temporal and Statistical Features for Improved Falsification Attack Detection in Vehicular Ad Hoc Networks
Zawiyah Saharuna, Tohari Ahmad, Royyana Muslim Ijtihadie · Conference on Information Communications Technology and Society (ICTAS) 2025 · https://doi.org/10.1109/ictas64866.2025.11155825 · relevance: medium · basis: abstract

**Problem:** Falsification attacks in Vehicular Ad Hoc Networks (VANETs) pose a significant threat to vehicle-to-vehicle communication integrity by manipulating Basic Safety Messages (BSM) that convey critical information such as position and speed.
**Approach:** To address this limitation, this study introduces a rolling-window-based feature engineering approach to capture vehicle movement dynamics more comprehensively. The proposed method achieves 98.12% recall in multiclass classification, representing a 12.86 percentage point improvement over the previous work (85.26%), with substantial gains in detecting challenging attack types such as Type 8 and Type 16.
**Tools and scale:** Not stated in the abstract.
**Results:** Experimental results demonstrate that feature engineering yields a 16.50% improvement in CatBoost model recall compared to the baseline version without feature engineering.
**Limitations and future work (as stated by the authors):**
- "To address this limitation, this study introduces a rolling-window-based feature engineering approach to capture vehicle movement dynamics more comprehensively."
- "The proposed method achieves 98.12% recall in multiclass classification, representing a 12.86 percentage point improvement over the previous work (85.26%), with substantial gains in detecting challenging attack types such as Type 8 and Type 16."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
