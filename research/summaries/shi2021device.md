# On-device IoT Certificate Revocation Checking with Small Memory and Low Latency
Xiaofeng Shi, Shouqian Shi, Minmei Wang et al. · ACM SIGSAC Conference on Computer and Communications Security (CCS) 2021 · https://doi.org/10.1145/3460120.3484580 · relevance: low · basis: abstract

**Problem:** Allowing a device to verify the digital certificate of another device is an essential requirement and key building block of many security protocols for emerging and future IoT systems that involve device-to-device communication.
**Approach:** This paper presents an on-device CR checking system called TinyCR, which achieves 100% accuracy, memory and computation efficiency, low synchronization latency, and low network bandwidth, while being compatible with the current certificate standard.
**Tools and scale:** Not stated in the abstract.
**Results:** This paper presents an on-device CR checking system called TinyCR, which achieves 100% accuracy, memory and computation efficiency, low synchronization latency, and low network bandwidth, while being compatible with the current certificate standard. Our implementation shows that TinyCR only costs each device 1.7 MB of memory to track 100 million IoT certificates with 1% revocation rate.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
