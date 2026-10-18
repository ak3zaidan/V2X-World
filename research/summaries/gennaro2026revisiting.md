# Revisiting the ETSI ITS Lifecycle with Certificateless Authorization Based on Group Signatures
Riccardo Gennaro, Stefano Berlato, Alessandro Tomasi, Silvio Ranise et al. · 2026 IEEE European Symposium on Security and Privacy Workshops (EuroS&amp;PW) 2026 · https://doi.org/10.1109/eurospw72509.2026.00052 · relevance: medium · basis: abstract

**Problem:** The current standards for Intelligent Transport Systems (ITSs) by the European Telecommunications Standards Institute (ETSI) rely heavily on Public Key Infrastructures (PKIs) and pseudonym-based digital certificates to provide message authenticity and user privacy in vehicular communications. Although effective, this approach introduces substantial complexity due to heavy certificate management and network overhead, ...
**Approach:** To simplify certificate management without sacrificing interoperability, this paper proposes a standard-compatible redesign of (part of) ETSI’s authentication and authorization lifecycle that replaces pseudonym-based certificates with Group Signatures (GSs). Finally, we discuss a further ETSI-compatible extension with Attribute-Based Encryption (ABE) to introduce fine-grained access control aligned with ETSI’s ...
**Tools and scale:** Also, we implement a proof-of-concept within the opensource C2C-Common platform using IBM’s libgroupsig library.
**Results:** Our benchmarks show that our redesign introduces substantial yet improvable signing and verification overheads — 3.7× and 10×, respectively — while maintaining comparable message sizes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — revisits the ETSI ITS certificate lifecycle (enrolment/authorization), which our engine models for the US SCMS; certificateless group-signature alternative is not in our engine.
**Novelty implications:** Does not remove any of our claims; it is a related point of comparison. We cannot claim to be first on the specific mechanism it covers; the joint closed-loop, multi-layer evaluation remains open as far as this abstract shows.
