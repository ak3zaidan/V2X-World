# TLS → Post-Quantum TLS: Inspecting the TLS landscape for PQC adoption on Android
Dimitri Mankowski, Thom Wiggers, Veelasha Moonsamy · 2023 IEEE European Symposium on Security and Privacy Workshops (EuroS&amp;PW) 2023 · https://doi.org/10.1109/eurospw59978.2023.00065 · relevance: low · basis: abstract

**Problem:** The ubiquitous use of smartphones has contributed to more and more users conducting their online browsing activities through apps, rather than web browsers.
**Approach:** In this paper, we performed an analysis of TLS usage by highest-ranked apps from Google Play Store to assess the resulting overhead for adoption of post-quantum algorithms. This will greatly magnify the impact of the transition to post-quantum cryptography, and we make recommendations for developers, server operators and the mobile operating systems to invest in making more use of these mitigating features or improving their accessibility.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our results show that apps set up large numbers of TLS connections with a median of 94, often to the same hosts. At the same time, many apps make little use of resumption to reduce the overhead of the TLS handshake.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
