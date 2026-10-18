# TraceCodec: A Compiler-Backed Neural Codec for Network Traffic Traces
Jun-Hui Ding, Xin-Chen Zhang, Xiao-Hui Xie, Shi-Nan Liu ·  2026 · https://arxiv.org/abs/2605.29941 · relevance: medium · basis: abstract

**Problem:** Critical networking workflows require high-fidelity packet captures (PCAPs) for testing, security analysis, and protocol validation, not just statistical flow-level summaries. Recent packet generators have demonstrated protocol-constrained PCAP synthesis, but many operate on raw or near-raw packet representations.
**Approach:** We identify the decode interface as a key bottleneck and present TraceCodec, a compiler-backed neural codec for stateful multi-flow traces.
**Tools and scale:** Not stated in the abstract.
**Results:** In a matched flow-matching continuation setup, TraceCodec yields complete decoded suffixes in 100% of CICIDS2017 cases and 98.44% of MAWI cases, versus at most 10.94% for raw-field latent controls. Structural diagnostics further show that TraceCodec preserves TCP transitions and multi-flow interleaving.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Network digital twin / learned network or traffic simulation; no V2X PKI or misbehaviour loop.
**Novelty implications:** Digital twins and learned simulators exist for other networks; joint traffic-radio-PKI counterfactual twin for V2X is not addressed.
