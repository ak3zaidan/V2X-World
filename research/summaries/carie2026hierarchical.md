# Hierarchical Trust Management for C-V2X Networks: A Game-Theoretic Edge-Cloud Architecture with LLM-Driven Calibration
Anil Carie, Sanjeev Kumar Mekala, Satish Anamalamudi, Teena Shaik et al. · Intelligent Automation & Soft Computing 2026 · https://doi.org/10.32604/iasc.2026.088003 · relevance: high · basis: full text

**Problem:** Authenticated insiders sending false BSMs; MDSs give no incentive guarantee.
**Approach:** Four-layer NR C-V2X trust architecture: edge AI detection, cloud LLM (llama3 8B) weight calibration, Nash-equilibrium conviction gate, ledger-anchored clawback payoffs.
**Tools and scale:** NS-3 4.1 with 5G-LENA NR, 3GPP Rel-18 channels; 6-100 vehicles per zone; 30-70% attackers; 90 s runs; nine attack types; five seeds with seed-sensitivity appendix.
**Results:** Zero false positives over 1,584 honest instances (95% upper bound 0.19%); attacking has negative expected payoff; detection rate about 86% in the composite-attack arm.
**Limitations and future work (as stated by the authors):**
- Seeds with pipeline failures were excluded and replaced (documented); short 90 s simulations.
**Overlap with our engine:** strong — insider MBD to conviction (revocation-like) decision with an LLM in the loop, in NR C-V2X simulation; overlaps direction 1 (MA decision design) and the LLM runner-up.
**Novelty implications:** Game-theoretic conviction plus LLM calibration is done; SCMS-faithful revocation (LA/CRL) and safety outcome remain open.
