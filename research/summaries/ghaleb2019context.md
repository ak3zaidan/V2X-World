# Context-aware data-centric misbehaviour detection scheme for vehicular ad hoc networks using sequential analysis of the temporal and spatial correlation of the consistency between the cooperative awareness messages
Fuad A. Ghaleb, Mohd Aizaini Maarof, Anazida Zainal, Murad A. Rassam, Faisal Saeed, Mohammed Alsaedi · Vehicular Communications 2019 · https://doi.org/10.1016/j.vehcom.2019.100186 · relevance: high · basis: full text

**Problem:** MBD solutions use static thresholds that fail in dynamic vehicular contexts.
**Approach:** Context-aware data-centric MDS: Kalman filter tracking of neighbours' CAMs, innovation errors for temporal consistency (box plot), Hampel filter for spatial context reference and online plausibility reference; messages flagged when deviating from context.
**Tools and scale:** Simulation (NGSIM-type traces / network simulator; details not in extracted text); positioning noise modelled.
**Results:** 73% reduction in false alarm rate and 37% improvement in detection rate vs static-threshold solutions.
**Limitations and future work (as stated by the authors):**
- Reviews prior work assuming stationary Gaussian noise and ideal communication
- their own future work section not extracted.
**Overlap with our engine:** partial — adaptive-threshold local MBD; relevant to direction 2 (detection thresholds) but no safety or revocation coupling.
**Novelty implications:** Dynamic thresholds are known; their safety cost under a real loop is open.
