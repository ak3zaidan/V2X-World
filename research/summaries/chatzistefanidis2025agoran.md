# Agoran: An Agentic Open Marketplace for 6G RAN Automation
Ilias Chatzistefanidis, Navid Nikaein, Andrea Leone et al. · arXiv 2025 · https://arxiv.org/abs/2508.09159 · relevance: low · basis: abstract

**Problem:** Next-generation mobile networks must reconcile the often-conflicting goals of multiple service owners.
**Approach:** We introduce Agoran Service and Resource Broker (SRB), an agentic marketplace that brings stakeholders directly into the operational loop. An 1B-parameter Llama model, fine-tuned for five minutes on 100 GPT-4 dialogues, recovers approximately 80% of GPT-4.1's decision quality, while operating within 6 GiB of memory and converging in only 1.3 seconds.
**Tools and scale:** Mentions: 5G.
**Results:** Deployed on a private 5G testbed and evaluated with realistic traces of vehicle mobility, Agoran achieved significant gains: (i) a 37% increase in throughput of eMBB slices, (ii) a 73% reduction in latency of URLLC slices, and concurrently (iii) an end-to-end 8.3% saving in PRB usage compared to a static baseline. These results establish Agoran as a concrete, standards-aligned path toward ultra-flexible, stakeholder-centric 6G networks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); does not touch the engine's core layers.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
