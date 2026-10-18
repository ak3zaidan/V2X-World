# State-of-the-Art Survey on In-Vehicle Network Communication (CAN-Bus) Security and Vulnerabilities
Omid Avatefipour, Hafiz Malik · arXiv.org 2018 · https://arxiv.org/abs/1802.01725 · relevance: low · basis: abstract

**Problem:** Nowadays with the help of advanced technology, modern vehicles are not only made up of mechanical devices but also consist of highly complex electronic devices and connections to the outside world. There are around 70 Electronic Control Units (ECUs) in modern vehicle which are communicating with each other over the standard communication protocol known as Controller Area ...
**Approach:** Even though CAN-Bus is considered as de-facto standard for in-vehicle network communication, it inherently lacks the fundamental security features by design like message authentication.
**Tools and scale:** Vehicular setting (VANET/IoV/V2X). Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- "This security limitation has paved the way for adversaries to penetrate into the vehicle network and do malicious activities which can pose a dangerous situation for both driver and passengers."
- "The objective of this research is to survey the current limitations of CAN-Bus protocol in terms of secure communication and different solutions that researchers in the society of automotive have provided to overcome the CAN-Bus limitation on different layers."
**Overlap with our engine:** partial: a V2X authentication/privacy scheme; our engine implements the standard SCMS/IEEE 1609.2 path rather than this protocol, and the paper reports no closed-loop traffic, radio or revocation simulation.
**Novelty implications:** Rules out nothing specific in our candidate directions. It shows that lightweight V2X authentication protocols are widely proposed; we should not claim a new authentication scheme. Open: evaluation of such schemes in a closed-loop city-scale simulation with radio and safety ground truth.
