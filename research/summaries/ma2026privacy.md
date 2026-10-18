# Privacy-Preserving and Collusion-Resistant Data Query Scheme for Vehicular Platoons
Chengyuan Ma, Peng Hu, Tian-Jiao Ni, Ying Liu et al. · IEEE Transactions on Network and Service Management 2026 · https://doi.org/10.1109/tnsm.2026.3669013 · relevance: medium · basis: abstract

**Problem:** Data queries play a crucial role in the vehicular platoon, enabling vehicles to obtain traffic information about surrounding road conditions and personalized entertainment information services. However, data query requests from vehicles may expose the vehicle owner’s personal attributes and habits.
**Approach:** To solve this problem, in this paper we propose a privacy-preserving and collusion-resistant data query scheme, named PCDQ.
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** Detailed security proofs and analysis demonstrate that PCDQ satisfies multiple security properties, including resistance to collusion attacks and replay attacks, unlinkability, confidentiality, and authentication and data integrity. Experimental results show that, compared to existing solutions, PCDQ performs better in terms of computation overhead, communication overhead, and network performance.
**Limitations and future work (as stated by the authors):**
- Specifically, PCDQ uses the Paillier encryption and Chinese Remainder Theorem to protect the query privacy of vehicle owners, allowing the RSU to recover individual data query requests without associating them with the original vehicles.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
