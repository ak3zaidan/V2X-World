# Co-simulate no more: The CARLA V2X Sensor
Daniel Grimm, Marc Schindewolf, David Kraus, Eric Sax · 2024 IEEE Intelligent Vehicles Symposium (IV) 2024 · https://doi.org/10.1109/iv55156.2024.10588488 · relevance: high · basis: abstract

**Problem:** No easy-to-use tool combining V2X simulation with AD sensor data.
**Approach:** CARLA V2X sensor module: CAM generation/transmission/reception with LOS, NLOSv and NLOSb propagation; configurable parameters.
**Tools and scale:** CARLA; benchmark with varying number of vehicles.
**Results:** V2X simulation adds only small overhead to sensors such as cameras.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — in-process V2X inside a world simulator (avoiding co-simulation), similar to our single-process design; no PKI.
**Novelty implications:** Single-process V2X-in-simulator is not novel by itself.
