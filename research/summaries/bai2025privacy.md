# Privacy-Preserving Driver Monitoring on the Edges: Transformer-Based Processing of Secret Shares from Video Streams
Tianyu Bai, Danyang Shao, Ying He, Qing Yang et al. · ACM Trans. Internet Things 2025 · https://doi.org/10.1145/3777384 · relevance: medium · basis: abstract

**Problem:** Modern vehicles increasingly rely on advanced driver monitoring systems (DMS) to ensure safety and enhance the driving experience. These systems assess driver status to prevent accidents caused by fatigue, inattentiveness, or intoxication.
**Approach:** To overcome these limitations, we present the Privacy-preserving Driver Monitoring System (PDMS), a novel framework based on the additive secret sharing theory and privacy-preserving Transformer-based deep learning models. Furthermore, we present a practical system that utilizes a vision transformer (ViT)-based gaze estimation model, demonstrating the effectiveness of PDMS through comprehensive experiments.
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** PDMS creates randomized secret shares from driver’s facial video data on vehicle, processes them independently through privacy-preserving Transformer models on edges, and securely aggregates partial results on vehicle, ensuring vehicles’ sensor data and final results remain protected. This approach reduces the computational load on the vehicle, enabling cost-effective and scalable DMS solutions that protect the ...
**Limitations and future work (as stated by the authors):**
- To overcome these limitations, we present the Privacy-preserving Driver Monitoring System (PDMS), a novel framework based on the additive secret sharing theory and privacy-preserving Transformer-based deep learning models.
- PDMS creates randomized secret shares from driver’s facial video data on vehicle, processes them independently through privacy-preserving Transformer models on edges, and securely aggregates partial results on vehicle, ensuring vehicles’ sensor data and final results remain protected.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
