# Tackling Ambiguity in User Intent for LLM-based Network Configuration Synthesis
Rajdeep Mondal, Nikolaj Bjorner, Todd Millstein, Alan Tang et al. · Proceedings of the 24th ACM Workshop on Hot Topics in Networks 2025 · https://doi.org/10.1145/3772356.3772402 · relevance: medium · basis: abstract

**Problem:** Beyond hallucinations, another problem in program synthesis using LLMs is ambiguity in user intent. We illustrate the ambiguity problem in a networking context for LLM-based incremental configuration synthesis of route maps and ACLs.
**Approach:** We propose a prototype system, Clarify, augmenting an LLM with a new module called a Disambiguator that helps elicit user intent.
**Tools and scale:** Mentioned in abstract: LLM. (From the abstract only.)
**Results:** Measurements in a large cloud identify complex ACLs with 100s of overlaps, showing ambiguity is a real problem.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
