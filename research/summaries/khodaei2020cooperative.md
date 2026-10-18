# Cooperative Location Privacy in Vehicular Networks: Why Simple Mix-zones are not Enough
Mohammad Khodaei, Panos Papadimitratos · IEEE Internet of Things Journal 2020 · https://doi.org/10.1109/jiot.2020.3043640 · relevance: high · basis: full text

**Problem:** Cryptographic mix-zones are still linkable.
**Approach:** Tracking algorithm linking pseudonyms across mix-zones; cooperative mix-zone with relaying vehicles emitting decoy traffic.
**Tools and scale:** IEEE IoT-J; realistic mobility traces; standardised CAMs.
**Results:** Eavesdropper links about 73% of pseudonyms off-peak and 62% at rush hour; cooperative scheme improves privacy.
**Limitations and future work (as stated by the authors):**
- Expand adversary to malicious vehicles or compromised RSUs leaking keys.
- Effect of mix-zone range on overhead.
- Kalman-filter and radio-signal-based tracking.
- Compare privacy metrics.
- Evaluate decoy-traffic impact on safety applications.
**Overlap with our engine:** partial — linkability; authors list radio-signal tracking as future work, matching our cross-layer linkability runner-up.
**Novelty implications:** Application-layer linkability is quantified; radio-layer (PHY/MAC identifier) linkability across pseudonym changes is flagged as future work, i.e. open.
