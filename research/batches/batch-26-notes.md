# Batch 26 notes

All 378 papers are low relevance (mostly VANET/IoV authentication schemes and general threshold-signature background); all summaries are from abstracts.

## 1. Stated limitations and future work

- liang2024physically: Notably, this process guarantees the EV’s real identity remains undisclosed, even to the Control Center (CC).
- xue2021efficient: At the same time, the cost of the offline phase should remain similar to that of a normal signature generation.
- wong2023real: Latest (communication-optimizing) schemes often assume all n or at least n ′ ≥ t participating users remain honest throughout the pre-signing phase, essentially degenerating to n ′ -out-of- n ′ multiparty signing instead of t -out-of- n threshold sig
- huang2020secure: However, how to efficiently secure vehicular networks with privacy-preserving remains a big challenge.
- qureshi2022authentication: However, existing schemes have some limitations in terms of computational costs and efficiency and do not provide security at all levels.
- turnip20266g: Existing 5G-AKA protocols remain vulnerable to user tracking, replay attacks, and false base station impersonation, while Elliptic Curve Cryptography (ECC) and RSA-based key exchanges face imminent obsolescence due to quantum attacks.
- turnip20266g: Experimental benchmarks highlight potential performance constraints, including latency in PQC-based key exchange and resource limitations in edge computing.
- tan2023efficient: With assistance from the neighboring vehicles, the homomorphic signature involving all requesting vehicles is generated and forwarded to the remaining functional RSUs.
- sowa2024post: The grand question of how existing cyberinfrastructure will support post-quantum cryptography remains unanswered.
- shen2024blockchain: By leveraging a non-pairing certificateless mechanism, the BBASV scheme effectively addresses these limitations, reducing the computational complexity and network traffic in the authentication process, thereby enabling efficient batch authentication.
- choi2025puf: In this article, we propose a secure and lightweight authentication scheme for UAV-assisted IoV, considering the computational limitations of UAVs.
- benyamina2019anel: They considered as the best schemes in terms of computation and communication overheads when compared with other schemes that use other cryptography techniques, but they have some limitations.
- benyamina2019anel: The ANEL is a MAC-based authentication scheme that offers all the advantages of the MAC-based authentication schemes and overcomes all their limitations at the same time.
- wang2024lightweightb: Moreover, some existing schemes encounter limitations, including high computational complexity and the introduction of additional communication overhead and computational costs.
- alshareeda2025efficient: An interactive benchmarking toolkit and open-source codebase provide further reproducibility and potential for future work.
- battagliola2020threshold: Our goal is to produce digital signatures that are compatible with an existing centralized signature scheme: the key-generation and signature algorithms are replaced by a communication protocol between the players, but the verification algorithm rema
- zafar2025integrating: Code-based KEMs, despite their proven security measure they remain underexplored in hybrid frameworks due to some limitations like large key sizes and integration complexity.
- giron2023migrating: This position paper motivates ongoing and future research on this topic.
- aljanabi2024d: However, guaranteeing safe authentication in VFC presents notable difficulties as a result of characteristics such as dynamic network topology, extensive mobility, and limitations on resources.
- zhan2024dynamic: Secure group communication in Vehicle Ad hoc Networks (VANETs) over open channels remains a challenging task.
- katz2025honest: Yet in so-called key-management networks, where a fixed set of servers share a large number of keys on behalf of multiple users, it may be reasonable to assume that a majority of the servers remain uncompromised, and in that case there may be several
- wei2025lattice: Certificateless schemes effectively resolve both certificate management and key escrow problems but remain vulnerable to quantum computing threats.
- wei2025lattice: To address these limitations, this paper constructs an efficient post-quantum certificateless proxy re-signature scheme based on algebraic lattices.
- gao2025efficient: However, there remains significant room for improvement in ensuring privacy protection as well as the security and efficiency of key distribution.
- gao2025efficient: Furthermore, compared to other schemes, as the number of vehicles increases, the computational overhead at the vehicle and TA remain almost unchanged, while the signaling overhead is reduced by nearly half.
- campbell2025hybrid: This methodological limitation—extrapolating from permissioned to permissionless systems—represents a critical infrastructure failure that introduces massive uncertainty into migration planning.
- setyowati2025performance: Analysis of the latency distribution revealed a realistic network jitter profile, with a small number of outliers, while the majority of transmissions remained well below 150 ms.
- setyowati2025performance: Furthermore, CPU utilization on all virtual machines remained minimal, confirming the computational efficiency of ML-KEM for resource-constrained edge devices.
- setyowati2025performance: This work establishes a practical blueprint for deploying quantum-resistant protocols, with future research directed toward hardware-based validation, the integration of PQC digital signatures, and AI-driven anomaly detection.
- amadi2026symmetricasymmetric: Secure and auditable data sharing in large-scale Internet of Things (IoT) environments remains a significant challenge due to weak trust coordination, limited scalability, and susceptibility to emerging quantum attacks.
- prasanth2025comprehensive: This study highlights the strengths and limitations of each approach, providing insights into their suitability for different VANET applications.
- buruaga2025benchmarking: A promising approach is the hybridization of these technologies - combining their respective strengths while mitigating their limitations - and integrating them with the cryptographic mechanisms used today to enhance crypto-agility and facilitate tra
- lim2025pqtls: Various solutions have been proposed to address this issue; however, they suffer from limitations, such as the necessity of pre-fetching certificates or dependency on prior connections.
- liu2026enhanced: To address these limitations, we propose an enhanced PUF-based anonymous authentication protocol, referred to as iXDZ.
- gupta2026hybrid: The result is a practical path for operators who must begin quantum-safe preparation now, even where endpoint-native post-quantum support remains uneven.
- zhou2024crt: By utilizing the Chinese Remainder Theorem (CRT)-optimized Paillier encryption algorithm, it allows for homomorphic addition operations on encrypted data, aiming to support secure data aggregation in ciphertext states while ensuring data transmission
- liu2025efficientb: However, due to the mobility and openness of VANETs, they face a range of question, including vehicle privacy protection, message authentication, limitations in computational and storage capacity, and constrained network bandwidth.
- vincenzi2025carburos: In this context, Multi-Factor Authentication (MFA) can serve as an important building block; however, integrating MFA into vehicular environments remains nontrivial.
- tuncel2026budacvab: However, these approaches suffer from several limitations, including high computational cost, storage constraints, and reduced performance when processing raw driving data.
- tuncel2026budacvab: This limitation remains a critical challenge that needs to be addressed to ensure real-world applicability.
- shaw2026quantum: FIPS 203, 204 and 205 closed the algorithmic gap in post-quantum cryptography (PQC); the production gap -- hybrid combiners, conformance evidence, migration tooling, stateful signing, protocol helpers -- remains open.
- grupenhoff2023certificate: Since then, hash-based signature schemes have been created to replace vulnerable schemes, although they have limitations such as requiring a large key and/or signature size or only working a limited number of times.
- gorbenko2024evaluation: Although the performance profiles and “black-box” security of these schemes are well understood, resistance to side-channel attacks remains a weak point for all of them.
- zhao2024mika: To mitigate future security risks, researchers have been working on a hybrid approach that uses both classical and post-quantum cryptographic techniques, with the aim of keeping the system secure as long as at least one of the cryptosystems remains s

## 2. Strong overlap

None. No paper in this batch describes a closed-loop V2X simulator, SCMS backend, or a candidate direction; threshold-signature papers (e.g. doerner2024threshold, das2023adaptively) and PQ schemes (e.g. kumari2022post) are partial overlap only as cryptographic background.
