# When LLM Agents See a Conflict but Still Proceed: Cross-Domain Dependencies in Network Slicing
Marcos Lima Romero, Ricardo Suyama · arXiv 2026 · https://arxiv.org/abs/2610.17118 · relevance: low · basis: abstract

**Problem:** Future AI-native networks are being designed to apply autonomous agents built on large language models (LLMs) in network management through intents.
**Approach:** When intents from different domains meet on the same resource, an agent must recognize the cross-domain dependency before taking an irreversible action.
**Tools and scale:** Not stated in the abstract.
**Results:** This slice appears idle by design, which is precisely the profile targeted by a cost-reduction intent. Most models acted against the dependency they had observed: seven of the ten models removed the protection slice at least once after receiving the cross-domain dependency, and in the high-harm tier the slice was removed in 76% of the exposed runs, while four models practically never did.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); does not touch the engine's core layers.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
