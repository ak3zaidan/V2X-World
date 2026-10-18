# A Lightweight Post-Quantum Authentication Framework for 5G Base Station Bootstrapping
Saleh Darzi, Mirza Masfiqur Rahman, Imtiaz Karim et al. · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2606.30542 · relevance: low · basis: abstract

**Problem:** The absence of authenticated bootstrapping between User Equipments (UEs) and Base Stations (BSs) in 5G leaves System Information Block (SIB) broadcasts unprotected, enabling fake BS attacks, man-in-the-middle interception, and spoofed emergency alerts.
**Approach:** We propose EMULSION, a symmetric chained publicly verifiable authentication framework for 5G/6G BS broadcast authentication.
**Tools and scale:** Evaluated on a real over-the-air 5G testbed, EMULSION achieves 33x lower end-to-end delay and 31x less communication overhead than ML-DSA, and 12x lower delay and 5.4x less overhead than FN-DSA.
**Results:** EMULSION is the first framework to exploit native 5G architectural features: fixed SIB transmission windows, millisecond-level time synchronization, and eSIM/USIM credential management to achieve genuine PQ security at symmetric-key efficiency. Evaluated on a real over-the-air 5G testbed, EMULSION achieves 33x lower end-to-end delay and 31x less communication overhead than ML-DSA, and 12x lower delay and 5.4x less overhead than FN-DSA.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
