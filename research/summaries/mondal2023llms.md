# What do LLMs need to Synthesize Correct Router Configurations?
Rajdeep Mondal, Alan Tang, Ryan Beckett, Todd Millstein, George Varghese · Proceedings of the 22nd ACM Workshop on Hot Topics in Networks 2023 · https://doi.org/10.1145/3626111.3628194 · relevance: high · basis: full text (extracted sections)

**Problem:** Can GPT-4 synthesise correct router configurations?
**Approach:** Verified Prompt Programming: GPT-4 combined with verifiers whose localised feedback automatically corrects errors.
**Tools and scale:** Cisco-to-Juniper translation on one router; no-transit policy across multiple routers.
**Results:** GPT-4 alone makes egregious errors; with verifier, leverage of 10X (Juniper translation) and 6X (no-transit).
**Limitations and future work (as stated by the authors):**
- Experiments 'very preliminary'; much further testing in more complex use cases needed.
- Human input still required.
- Open whether GPT-4 can add policy incrementally without interfering with verified policy.
**Overlap with our engine:** partial - LLM-generated configuration with verification; relevant to validating our LLM agent's configurations.
**Novelty implications:** Verifier-in-the-loop LLM config is established; applying it to simulator configuration is not covered.
