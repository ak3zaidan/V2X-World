# On the Impact of Building Attenuation Models in VANET Simulations of Urban Scenarios
Luis Urquiza-Aguiar, Carolina Tripp-Barba, José Estrada-Jiménez, Mónica Igartua · Electronics 2015 · https://doi.org/10.3390/electronics4010037 · relevance: high · basis: full text

**Problem:** Building attenuation models add cost to urban VANET simulation; is detail worth it?
**Approach:** Compares total blockage, offline-computed attenuation and a realistic propagation scheme for buildings in two urban scenarios.
**Tools and scale:** Network simulator with 802.11p, multi-hop traffic-efficiency application; up to 250 nodes, 300 s runs.
**Results:** Realistic model differs from others by at most 3% packet loss and 0.5 s end-to-end delay; total blockage is a conservative bound; no statistical relation with density except average neighbours.
**Limitations and future work (as stated by the authors):**
- Test other 802.11p channel capacities
- empirically obtain model-difference curves across scenario types
- test time discretisation of offline attenuation files and its step size (110 MB file for 300 s, 250 nodes).
**Overlap with our engine:** partial — building obstruction modelling, which our PHY includes.
**Novelty implications:** No impact on security claims.
