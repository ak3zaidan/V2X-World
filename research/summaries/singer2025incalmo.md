# Incalmo: An Autonomous LLM-assisted System for Red Teaming Multi-Host Networks
Brian Singer, Keane Lucas, Lakshmi Adiga, Meghna Jain et al. · arXiv 2025 · https://doi.org/10.1109/sp63933.2026.00132 · relevance: medium · basis: abstract

**Problem:** Security operators use red teams to simulate real attackers and proactively find defense gaps. In realistic enterprise settings, this involves executing multi-host network attacks spanning many “stepping stone” hosts.
**Approach:** Given the promise of LLMs in CTF challenges, we first analyze whether LLMs can autonomously execute multi-host red-team exercises. We find that state-of-the-art LLM-assisted offense systems (e.g., PentestGPT, CyberSecEval3) with leading LLMs (e.g., Sonnet 4, Gemini 2.5 Pro) are unable to do so.
**Tools and scale:** Mentioned in abstract: LLM, GPT. (From the abstract only.)
**Results:** Based on this insight, we design and implement Incalmo11All code is publicly available: Incalmo GitHub an LLM-assisted system for autonomously red teaming multihost networks. For our evaluation, we develop MHBench, a novel multihost attack benchmark with 40 realistic emulated networks (from 22 to 50 hosts). We find that Incalmo successfully acquires critical assets (i.e., key hosts or data) in 37 of 40 MHBench environments.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM/agents for security operations or network security); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
