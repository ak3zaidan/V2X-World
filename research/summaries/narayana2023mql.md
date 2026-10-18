# MQL: ML-Assisted Queuing Latency Analysis for Data Center Networks
Shruti Yadav Narayana, Jie Tong, Anish Krishnakumar, Nuriye Yildirim et al. · 2023 IEEE International Symposium on Performance Analysis of Systems and Software (ISPASS) 2023 · https://doi.org/10.1109/ispass57527.2023.00014 · relevance: medium · basis: abstract

**Problem:** Data center network (DCN) performance analysis is becoming increasingly critical due to the growing data center scale and proliferation of latency-critical applications. Packetlevel simulators, the de-facto performance evaluation tools, allow accurate modeling of the network and protocols, but they are extremely slow.
**Approach:** To address these challenges, we propose a novel and scalable performance analysis methodology that combines two powerful techniques.
**Tools and scale:** Mentioned in abstract: ns-3, NS-3. (From the abstract only.)
**Results:** The resulting ML-assisted technique achieves less than 3% modeling error on average compared to ns-3 simulations. Moreover, the speedup over ns-3 ranges from 100× to 9000× on DCNs with 128 to 1024 nodes.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — ML surrogate for slow packet-level simulation.
**Novelty implications:** Same.
