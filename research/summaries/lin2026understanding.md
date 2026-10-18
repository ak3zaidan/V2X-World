# Understanding Implicit Trust Errors in Core Carrier Networks through Multi-Agent Flaw Discovery and Analysis
Ziyu Lin, Ziting Wang, Xinfeng Li, Wei Dong et al. · arXiv 2026 · https://arxiv.org/abs/2607.10315 · relevance: medium · basis: abstract

**Problem:** Cellular core networks (CNs) are critical infrastructure, yet their internal security model has historically relied on physical isolation: interfaces between core components often operate within an assumed trust zone. As CNs transition to cloud-native deployments, this assumption weakens, expanding the attack surface and enabling external adversaries to ...
**Approach:** To detect iTrues and understand their security impacts, we designed iFinder, an LLM-driven multi-agent system that summarizes known flaws, distills them into detection patterns, and applies them to discover new iTrues in CN implementations.
**Tools and scale:** Importantly, a session-hijacking flaw has been confirmed on real-world commercial 5G core networks.
**Results:** Among them, 83 have already been confirmed and 81 have been assigned CVEs. Importantly, a session-hijacking flaw has been confirmed on real-world commercial 5G core networks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
