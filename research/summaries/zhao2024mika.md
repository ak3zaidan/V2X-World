# MIKA: A Minimalist Approach to Hybrid Key Exchange
Raymond K. Zhao, N. Sultan, Phillip Yialeloglou, Dongxi Liu et al. · Conference on Privacy, Security and Trust 2024 · https://doi.org/10.1109/pst62714.2024.10788054 · relevance: low · basis: abstract

**Problem:** However, most existing hybrid cryptosystems require protocol revisions to accommodate post-quantum cryptographic algorithms, leading to extensive modifications of existing code-bases and increased complexity in the state machines.
**Approach:** In this paper, we explore a novel generic hybrid model that requires only minimal changes to the codebase of a classical cryptosystem while maintaining the simplicity of the state machines. To illustrate the working principle and provide a benchmark for our generic hybrid model, we conduct a case study on the IKEv2 protocol using the strongS wan library.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- "To mitigate future security risks, researchers have been working on a hybrid approach that uses both classical and post-quantum cryptographic techniques, with the aim of keeping the system secure as long as at least one of the cryptosystems remains secure."
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
