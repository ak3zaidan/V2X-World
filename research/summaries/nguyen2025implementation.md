# Implementation Efficiency of Falcon Digital Signature Scheme on Arty-7 XC7A35T Board
Tat-Thang Nguyen, Duc-Duy Nguyen, Toan-Thanh Dao, Nhu-Quynh Luc · Electronics 2025 · https://doi.org/10.3390/electronics14224504 · relevance: low · basis: abstract

**Problem:** However, the software component has not yet undergone thorough security testing.
**Approach:** The system is combined with a software component developed in C# using the NET Framework 4.7, capable of running on the Windows operating system. The system is designed to leverage the hardware acceleration capabilities of an FPGA to enhance performance compared to software-only signature methods.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The obtained results show that key generation execution time is approximately 1516.2 ms, while the signing and verification times are around 2–5 ms. Hardware resource usage has significantly improved, and the power consumption is 0.097 W (57%).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
