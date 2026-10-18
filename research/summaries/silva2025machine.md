# Machine Learning-Powered IDS for Gray Hole Attack Detection in VANETs
M.C. Juan Antonio Arizaga Silva, Alejandro Medina Santiago, Mario Espinosa Tlaxcaltecatl, Carlos Muñiz-Montero · World Electric Vehicle Journal 2025 · https://doi.org/10.3390/wevj16090526 · relevance: low · basis: abstract

**Problem:** Vehicular Ad Hoc Networks (VANETs) enable critical communication for Intelligent Transportation Systems (ITS) but are vulnerable to cybersecurity threats, such as Gray Hole attacks, where malicious nodes selectively drop packets, compromising network integrity. Traditional detection methods struggle with the intermittent nature of these attacks, necessitating advanced ...
**Approach:** This study proposes a machine learning-based Intrusion Detection System (IDS) to detect Gray Hole attacks in VANETs. Methods: This study proposes a machine learning-based Intrusion Detection System (IDS) to detect Gray Hole attacks in VANETs.
**Tools and scale:** Vehicular setting (VANET/IoV/V2X). Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The Random Forest classifier outperformed others, achieving an F1-score of 0.9927 with 15 estimators and a depth of 15.
**Limitations and future work (as stated by the authors):**
- "In contrast, SVM variants exhibited limitations due to overfitting, with precision and recall below 0.76."
- "Future work could explore hybrid models or real-world deployment to further validate the system’s efficacy."
**Overlap with our engine:** partial: vehicular security topic, but no simulation of traffic, radio, SCMS backend or safety outcomes is described.
**Novelty implications:** Rules out nothing specific in our candidate directions. It shows that lightweight V2X authentication protocols are widely proposed; we should not claim a new authentication scheme. Open: evaluation of such schemes in a closed-loop city-scale simulation with radio and safety ground truth.
