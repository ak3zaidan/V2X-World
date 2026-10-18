# Broadcast Authentication in Latency-Critical Applications: On the Efficiency of IEEE 1609.2
Mir Ali Rezazadeh Baee, L. Simpson, E. Foo, Josef Pieprzyk · IEEE Transactions on Vehicular Technology 2019 · https://doi.org/10.1109/tvt.2019.2945339 · relevance: high · basis: full text (extracted sections)

**Problem:** Does IEEE 1609.2 authentication overhead endanger latency-critical safety messages?
**Approach:** Implements 1609.2-recommended primitives, measures CPU cycles, time and message size; simulates a high-density highway for IEEE 1609, ETSI ITS-G5 and ARIB STD-T109, counting safety messages received within 100 ms.
**Tools and scale:** Own cryptographic implementation; highway simulation; 802.11p/EDCA.
**Results:** Shows how authentication overhead reduces timely-received safety messages and that, under an assumed traffic scenario, a crash is possible due to authentication delay (numbers not extracted).
**Limitations and future work (as stated by the authors):**
- Assumed traffic scenario rather than closed-loop traffic simulation (as described).
**Overlap with our engine:** strong - quantifies the safety cost of 1609.2 verification delay in dense traffic (direction 2), including a crash argument.
**Novelty implications:** We cannot claim to be first to link 1609.2 verification latency to crash risk. Open: closed-loop traffic with real MAC, PQ/hybrid certificates, verification policies (verify-on-demand), and SCMS certificate sizes at city scale.
