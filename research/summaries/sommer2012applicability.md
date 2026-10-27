# On the applicability of Two-Ray path loss models for vehicular network simulation
Christoph Sommer, Stefan Joerer, Falko Dressler · IEEE Vehicular Networking Conference 2012, pp. 64-69 · https://doi.org/10.1109/vnc.2012.6407446 · relevance: high · basis: full text (authors' copy, nsm.inf.tu-dresden.de/bib/sommer2012applicability/, read 2026-10-26; cached as `papers/sommer2012applicability.pdf`)

**Problem:** Network simulators offer a "Two-Ray Ground" model (Friis below a crossover distance, d⁻⁴ above it). For 802.11p antennas on cars the crossover is far beyond any realistic link, so such simulations are in fact free-space simulations, and they miss the distance-dependent constructive and destructive interference of the ground reflection.
**Approach:** §II-A derives the Two-Ray Interference model: d_los = √(d² + (h_t − h_r)²), d_ref = √(d² + (h_t + h_r)²), φ = 2π(d_los − d_ref)/λ (Eq. 4); sinθ = (h_t + h_r)/d_ref, cosθ = d/d_ref; Γ⊥ = (sinθ − √(εr − cos²θ))/(sinθ + √(εr − cos²θ)) (Eq. 5); L_tri = 20·log10(4πd/λ · |1 + Γ⊥e^{iφ}|⁻¹) (Eq. 6). Two-Ray Ground (Eq. 7-8) and the crossover d_c = 4π·h_t·h_r/λ (Eq. 9; 886.6 m for 1.895 m antennas at 5.89 GHz).
**Tools and scale:** Field measurements with off-the-shelf 802.11p DSRC radios on two Audi A4 cars (antennas at 1.895 m, 5.890 GHz CCH); Veins (SUMO + OMNeT++/MiXiM) implementation; highway and suburban scenarios.
**Results:** Figure 4 overlays the measured RSS quantiles with the analytical model at **εr = 1.02** and the simulation: the interference model fits the measured fades. Two-Ray Ground adds nothing over free space at realistic V2V ranges; free space with a fitted exponent α misses the fade structure.
**Limitations and future work (as stated by the authors):**
- Further reflections (their ref. [18]) are not modelled; obstacles are separate models.
**Overlap with our engine:** strong: `propagation/two-ray-interference` (crates/v2xw-radio/src/prop.rs, WP-09) is re-implemented from Eq. 4-6 with εr 1.02 from Figure 4.
**Novelty implications:** None; this is a model source.
