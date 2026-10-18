# An Enhanced and Lightweight Anonymous Authentication Protocol Based on PUF for VANETs
Yidan Liu, Xingyun Hu, Bing Gao, Yan-Bei Zhu et al. · IEEE transactions on intelligent transportation systems (Print) 2026 · https://doi.org/10.1109/tits.2026.3660278 · relevance: low · basis: abstract

**Problem:** Previous authentication protocols often face challenges such as key leakage risks and high computational overhead.
**Approach:** Recently, Xie et al. designed an anonymous authentication protocol for VANETs. We rigorously demonstrate the security of iXDZ under the random oracle model and supplement the proof utilizing the Scyther formal analysis tool.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Performance evaluations reveal that iXDZ significantly enhances security while reducing computational and communication overhead. Additionally, a case study highlights that iXDZ achieves a 33.3% reduction in execution time compared to the original protocol.
**Limitations and future work (as stated by the authors):**
- "To address these limitations, we propose an enhanced PUF-based anonymous authentication protocol, referred to as iXDZ."
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
