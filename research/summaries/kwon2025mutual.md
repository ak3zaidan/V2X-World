# Mutual Compact Hybrid Signature for Quantum-Secure Embedded Web Server Communication in Smart Grid Devices
Hee-Yong Kwon, Bohyun Ahn, Mun-Kyu Lee, Taesic Kim · 2025 IEEE Design Methodologies Conference (DMC) 2025 · https://doi.org/10.1109/dmc65958.2025.11475303 · relevance: low · basis: abstract

**Problem:** Smart grid devices, such as solar inverters and smart meters, are increasingly equipped with embedded web servers for easy and efficient device configuration, monitoring, and maintenance with encrypted communication. However, most embedded web servers use self-signed digital certificates, which is vulnerable to man-in-the-middle (MITM) attacks due to the absence of a ...
**Approach:** However, most embedded web servers use self-signed digital certificates, which is vulnerable to man-in-the-middle (MITM) attacks due to the absence of a centralized trust chain. In addition to classical threats, the advancement of quantum computing introduces new threats from quantum adversaries capable of breaking classical public-key cryptography algorithms which are currently used in smart grid devices.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The experimental results show that the proposed compact hybrid signature can enhance both the scalability and security of the embedded web server communications for smart grid devices against both MITM and quantum adversaries, providing a practical and quantum-resistant solution for resource-constrained environments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
