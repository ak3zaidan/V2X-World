# GROSS German Rail Open-Source SUMO Scenario
Juri Penell, Damian Dailisan · SUMO Conference Proceedings 2026 · https://doi.org/10.52825/scp.v7i.3324 · relevance: medium · basis: abstract

**Problem:** Microscopic simulation enables reproducible evaluation in intelligent transportation systems, yet most open SUMO scenarios and toolchains remain road-traffic centric, leaving rail underrepresented despite its importance for public transport and its sensitivity to network-wide disruptions.
**Approach:** We present the German Rail Open-Source Scenario (GROSS), an open pipeline that combines OpenStreetMap railway infrastructure with GTFS schedules to generate nation-scale rail scenarios for SUMO (Simulation of Urban MObility). Existing conversions often rely on geometry-only stop-to-track matching and inconsistent platform/track assignments, which can create routing anomalies and unstable simulations dominated by teleportation artefacts.
**Tools and scale:** Mentions: SUMO; scale not stated in the abstract.
**Results:** Across multiple German regions, GROSS reduces average teleportations per vehicle by a factor of 1.7--76.8$\times$, shortens delays compared to the vanilla SUMO pipeline, and it enables end-to-end generation of a Germany-wide scenario with 35\,925 trips for comparisons with operator-reported delay statistics.
**Limitations and future work (as stated by the authors):**
- While the remaining long delays highlight limitations in available timetable metadata and rail dispatch modeling, GROSS lowers the barrier to building scalable, fully open rail simulations and to studying delay propagation at country scale.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
