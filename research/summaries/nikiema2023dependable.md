# Towards Dependable RISC-V Cores for Edge Computing Devices
Pegdwende Romaric Nikiema, A. Palumbo, Allan Aasma, Luca Cassano et al. · IEEE International Symposium on On-Line Testing and Robust System Design 2023 · https://doi.org/10.1109/iolts59296.2023.10224862 · relevance: medium · basis: abstract

**Problem:** The migration of the computation from the cloud into edge devices, i.e., Internet-of-Things (IoTs) devices, reduces the latency and the quantity of data flowing into the network. With the emerging open-source and customizable RISC-V Instruction Set Architecture (ISA), cores based on such ISA are promising candidates for several application domains within ...
**Approach:** We propose three mechanisms towards this goal, i.e., (i) a Root of Trust module for post-quantum secure boot, (ii) hardware checkers against hardware trojan horses and microarchitectural side-channel attacks, and (iii) a fine-grained dual core lockstep mechanism for real-time error detection and correction.
**Tools and scale:** Not stated in the abstract.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- The paper illustrates the proposed mechanisms with related motivations and implications, as well as a discussion on future research directions.
**Overlap with our engine:** partial — Post-quantum cryptography performance/schemes; engine has PQ and hybrid signatures but this work does not model SCMS migration or safety impact.
**Novelty implications:** PQ cost measurements exist; their system-level safety cost inside an SCMS-driven V2X simulation remains open.
