# A General Testing Framework Based on Veins for Securing VANET Applications
Liang Ming, Gang Zhao, Minhuan Huang, Xiaohui Kuang, Jingzhe Zhang, Huayang Cao et al. · 2018 IEEE SmartWorld, Ubiquitous Intelligence &amp; Computing, Advanced &amp; Trusted Computing, Scalable Computing &amp; Communications, Cloud &amp; Big Data Computing, Internet of People and Smart City Innovation (SmartWorld/SCALCOM/UIC/ATC/CBDCom/IOP/SCI) 2018 · https://doi.org/10.1109/smartworld.2018.00347 · relevance: high · basis: abstract

**Problem:** Cyber threats (MITM, spoofing, DoS) can compromise VANET applications; need a way to test their security.
**Approach:** General Veins-based security testing framework; proof of concept with malicious-message attacks on an expressway tolling application; countermeasures.
**Tools and scale:** Veins (OMNeT++ + SUMO); expressway tolling.
**Results:** Attacks cause 60.60% more total toll fees, 8.39% longer travel distance and 43.63% more travel time.
**Limitations and future work (as stated by the authors):**
- Challenges of combining packet simulators into one framework and other limitations reported.
**Overlap with our engine:** strong — attacks executed inside a coupled traffic/V2X simulation with measured traffic-level impact, as our closed-loop attack engine does.
**Novelty implications:** Closed-loop attack impact on traffic in Veins is prior art; open: PKI/MA response and revocation timing.
