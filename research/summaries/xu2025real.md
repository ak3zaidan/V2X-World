# Real-Time Self-Adaptive SMPC for Non-IID Vehicular Data in Intelligent Transportation Systems
Xiao-Jie Xu, Xiao-Feng Chen, Xiangfei Lou, Ziling Liu et al. · International Conference on Cyber Security and Cloud Computing 2025 · https://doi.org/10.1109/cscloud66326.2025.00049 · relevance: medium · basis: abstract

**Problem:** Real-time traffic analytics require joint processing of privacy-sensitive vehicular data that are inherently non-IID, yet current secure multi-party computation (SMPC) pipelines either ignore data skew or exceed the sub- 200 ms latency budget of V2X links. The entire workflow executes within a two-party additive SMPC circuit, protecting inputs from honest-but-curious roadside units and cloud servers.
**Approach:** We present RT-AW-SMPC, a self-adaptive framework that combines (i) an entropy-volume-based local weighting estimator to debias aggregation under skewed sample distributions, and (ii) an edge-optimized private set intersection protocol that aligns feature keys in only two network rounds.
**Tools and scale:** Experiments on the public PEMS-D7 freeway dataset show that RT-AW-SMPC reduces aggregation error by $6.3 \times$ compared with a uniform-weight SMPC baseline, completes each computation round in 25 ms-well below the V2X hard limit-and caps per-vehicle uplink traffic at 2.3 KB.
**Results:** Real-time traffic analytics require joint processing of privacy-sensitive vehicular data that are inherently non-IID, yet current secure multi-party computation (SMPC) pipelines either ignore data skew or exceed the sub- 200 ms latency budget of V2X links. Experiments on the public PEMS-D7 freeway dataset show that RT-AW-SMPC reduces aggregation error by $6.3 \times$ compared with a uniform-weight SMPC baseline, ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
