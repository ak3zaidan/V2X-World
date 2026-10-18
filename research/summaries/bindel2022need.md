# The Need for Being Explicit: Failed Attempts to Construct Implicit Certificates from Lattices
Nina Bindel, Sarah McCarthy · The Computer Journal 2022 · https://doi.org/10.1093/comjnl/bxac132 · relevance: high · basis: abstract

**Problem:** IEEE 1609.2 relies on implicit certificates, only available over elliptic curves.
**Approach:** Attempts to build lattice implicit certificates from Dilithium/Falcon; explains failures.
**Tools and scale:** Not stated in the abstract.
**Results:** Each design fails due to key/signature sizes, sampler requirements and parameter rigidity.
**Limitations and future work (as stated by the authors):**
- No successful lattice implicit certificate construction
- hopes to spark more successful constructions
**Overlap with our engine:** partial — PQ implicit certificates for butterfly keys of SCMS.
**Novelty implications:** Confirms PQ SCMS must use explicit certificates; open problem remains.
