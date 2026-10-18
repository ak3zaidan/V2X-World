# Virtual Traffic Simulator for Connected and Automated Vehicles
Junfeng Zhao, Hai Wu, ChenFang Chang · SAE Technical Paper Series 2019 · https://doi.org/10.4271/2019-01-0676 · relevance: low · basis: abstract

**Problem:** &lt;div class="section abstract"&gt;&lt;div class="htmlview paragraph"&gt;Connected and automated vehicle (CAV) technologies promise a substantial decrease in traffic accidents and traffic jams, and bring new opportunities for improving vehicle’s fuel economy.
**Approach:** However, testing autonomous vehicles in a real world traffic environment is costly, and covering all corner cases is nearly impossible. Furthermore, it is very challenging to create a controlled real traffic environment that vehicle tests can be conducted repeatedly and compared fairly.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The simulator integrates a 3D traffic model with a high-fidelity vehicle model using a modular architecture, which supports hardware-in-the-loop (HIL) and vehicle-in-the-loop (VIL) testing.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
