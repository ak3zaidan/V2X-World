# Road Network Partitioning with Distributed Threaded-Parallelism for Large-Scale SUMO Traffic Simulation
Vicente R. Tomás, Iván Monzón-Catalán, Maribel Castillo, Luis A. García · unknown venue 2026 · https://doi.org/10.21203/rs.3.rs-11161759/v1 · relevance: low · basis: abstract

**Problem:** Abstract Large-scale SUMO traffic simulation can require substantial computational resources when realistic road networks, high traffic demand, dynamic routing, and computationally intensive external processing are considered.
**Approach:** One approach to reduce this cost is to spatially decompose the road network and process the resulting simulation domains concurrently, while also exploiting parallelism within each domain. This work follows this strategy through a two-level parallel architecture that combines thread-level parallelism enabled by free-threaded Python with distributed execution across multiple computing nodes.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** At the single-node level, the six-thread configuration achieves speedups from 3.88× to 5.77× over the sequential baseline. Distributed execution further increases performance, with the best configurations reaching speedups between 9.08× and 13.32×, corresponding to an additional 1.57×–2.45× acceleration over six-thread single-node execution.
**Limitations and future work (as stated by the authors):**
- The results also show that spatial decomposition affects transfer activity and distributed performance, whereas the increasing cost of global synchronization becomes the main observed limitation to further scalability.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
