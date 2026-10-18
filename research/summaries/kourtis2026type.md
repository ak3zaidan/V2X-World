# Type-Safe Decision Frameworks for Agentic 5G Control: A Theory-Driven Testbed Characterization of Where They Can Be Applied
M. Kourtis, G. Xilouris ·  2026 · https://arxiv.org/abs/2609.33689 · relevance: medium · basis: abstract

**Problem:** This paper presents a theory-driven characterization of type-safe decision frameworks for the agentic control of 5G networks, where every decision must be an element of a declared option set rather than free text. Three design points are evaluated on an Open5GS/UERANSIM testbed with a closed core-policy loop, namely a hosted typed model (Jev), an open ...
**Approach:** This paper presents a theory-driven characterization of type-safe decision frameworks for the agentic control of 5G networks, where every decision must be an element of a declared option set rather than free text.
**Tools and scale:** Three design points are evaluated on an Open5GS/UERANSIM testbed with a closed core-policy loop, namely a hosted typed model (Jev), an open fine-tunable typed encoder (Laya), and a zero-label retrofit of a general language model (AnyJev).
**Results:** Measuring every predicate yields an applicability map from framework to 5G decision class. Type safety removes format failures but not the question: the fine-tuned typed encoder returned its training answer for 98-99.5% of changed questions, and its calibrated gate then acted wrongly on up to 80% of them, whereas the question-reading frameworks acted wrongly on at ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
