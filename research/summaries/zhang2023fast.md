# Fast tracing method for Sybil attack in VANETs
Zhaoyi Zhang, Yingxu Lai, Ye Chen, Jingwen Wei et al. · IEEE Vehicular Technology Conference 2023 · https://doi.org/10.1109/vtc2023-fall60731.2023.10333449 · relevance: high · basis: abstract

**Problem:** Sybil detection mitigates but cannot trace attackers, especially with pseudonym changes.
**Approach:** Cascading operations to flag suspicious BSMs, then source estimation to trace the attacking node.
**Tools and scale:** IEEE VTC; simulation (details not in abstract).
**Results:** Precision >=97%, recall >=96%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — tracing attackers across pseudonym changes relates to our linkage-based revocation and cross-layer linkability.
**Novelty implications:** Physical tracing of Sybil sources exists; tying tracing to LA-based revocation with ground truth remains open.
