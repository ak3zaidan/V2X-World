# Evaluating the performance of post-quantum secure algorithms in the TLS protocol
Iraklis Tzinos, Konstantinos Limniotis, Nicholas Kolokotronis · Journal of Surveillance, Security and Safety 2022 · https://doi.org/10.20517/jsss.2022.15 · relevance: low · basis: abstract

**Problem:** However, the effect of the network parameters on the performance is more crucial than the effect of the underlying chosen ciphers.
**Approach:** This paper focuses on embedding post-quantum secure cryptographic algorithms into the TLS protocol to analyze its performance. Conclusion: According to the experiments, we conclude that there exist very promising algorithms that could be utilized in TLS in the near future, which may behave even better than the conventional elliptic curve algorithms for key exchange.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** It should also be pointed out that NIST announced on 5 July 2022 (i.e., after the completion of our research experiments) that, for general encryption used when we access secure websites, the CRYSTALS-Kyber algorithm has been selected, having as one of its advantages the speed of operation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
