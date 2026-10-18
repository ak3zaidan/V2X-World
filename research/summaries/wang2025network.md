# A Network Arena for Benchmarking AI Agents on Network Troubleshooting
Zhihao Wang, Alessandro Cornacchia, Alessio Sacco, Franco Galante, Marco Canini, Dingde Jiang · arXiv 2025 · https://arxiv.org/abs/2512.16381 · relevance: high · basis: full text (extracted sections)

**Problem:** No standardised benchmark for LLM agents in network troubleshooting.
**Approach:** NIKA: hundreds of curated incidents across five scenarios, 54 issue types, replay and agent-network interfaces.
**Tools and scale:** Emulated networks from data centres to ISPs.
**Results:** Larger models detect issues more often but struggle to localise faults and identify root causes.
**Limitations and future work (as stated by the authors):**
- Agents still struggle with fault localisation and root cause.
**Overlap with our engine:** partial - LLM root-cause analysis benchmark; runner-up direction (LLM cross-layer RCA) in a non-V2X domain.
**Novelty implications:** Cannot claim the first LLM troubleshooting benchmark; V2X cross-layer (traffic/radio/PKI) RCA remains open.
