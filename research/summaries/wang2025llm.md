# Towards LLM-Based Failure Localization in Production-Scale Networks
Chenxu Wang, Xumiao Zhang, Runwei Lu, Xianshang Lin et al. · Proceedings of the ACM SIGCOMM 2025 Conference 2025 · https://doi.org/10.1145/3718958.3750505 · relevance: medium · basis: abstract

**Problem:** Root causing and failure localization are critical to maintain reliability in cloud network operations. When an incident is reported, network operators must review massive volumes of monitoring data and identify the root cause (i.e., error device) as fast as possible, making it extremely challenging even for experienced operators.
**Approach:** In this paper, we present BiAn, an LLM-based framework designed to assist operators in efficient incident investigation.
**Tools and scale:** Mentioned in abstract: LLM. (From the abstract only.)
**Results:** To date, BiAn has been deployed in our network infrastructure for 10 months and it has successfully assisted operators in identifying error devices more quickly, reducing time to root causing by 20.5% (55.2% for high-risk incidents). Extensive performance evaluations based on 17 months of real cases further demonstrate that BiAn achieves accurate and fast failure localization. It improves accuracy by 9.2% compared to the baseline approach.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — LLM failure localization in production networks (BiAn).
**Novelty implications:** As above.
