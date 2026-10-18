# Digital Twin-Based Remaining Driving Range Prediction for Connected Electric Vehicles
Shilong Zhuo, Heng Li, Muaaz Bin Kaleem, Hui Peng, Yue Wu · SAE International Journal of Electrified Vehicles 2023 · https://doi.org/10.4271/14-13-01-0004 · relevance: medium · basis: abstract

**Problem:** &lt;div&gt;Electric vehicles (EVs) suffer from long charging time and inconvenient charging due to limited charging stations, which are the main causes of drivers’ range anxiety.
**Approach:** Real-time and accurate driving range prediction can help drivers plan journeys, alleviate range anxiety, and promote EV development. However, predicting the EV driving range is challenging due to different weather, road conditions, driver habits, and limited available data.
**Tools and scale:** Mentions: dataset, real-world; scale not stated in the abstract.
**Results:** Results show that the sliding window algorithm can significantly improve the prediction model with only SOC as input, while it can deteriorate other models with more features. The most accurate model taking all six features as inputs provides 89.8% data that has an accuracy of over 80%, while data proportion of the prediction model without past energy consumption is only 31.8%.&lt;/div&gt;
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
