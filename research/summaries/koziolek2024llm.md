# LLM-based and Retrieval-Augmented Control Code Generation
Heiko Koziolek, Sten Grüner, Rhaban Hark, Virendra Ashiwal, Sofia Linsbauer, Nafise Eskandani · Proceedings of the 1st International Workshop on Large Language Models for Code 2024 · https://doi.org/10.1145/3643795.3648384 · relevance: low · basis: abstract

**Problem:** Control code is designed and implemented for industrial automation applications that manage power plants, petrochemical processes, or steel production.
**Approach:** Popular large language models (LLM) can synthesize low-level control code in the Structured Text programming notation according to the standard IEC 61131-3, but are not aware of proprietary control code function block libraries, which are often used in practice. To automate control logic implementation tasks, we proposed a retrieval-augmented control code generation method that can integrate such function blocks into the generated code.
**Tools and scale:** Mentioned in abstract: simulations.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation, LLM agents but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
