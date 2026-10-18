# Simulator Coupled with Distributed Co-Simulation Protocol for Automated Driving Tests
Max-Arno Meyer, Lina Sauter, Christian Granrath, Hassen Hadj-Amor, Jakob Andert · Automotive Innovation 2021 · https://doi.org/10.1007/s42154-021-00161-1 · relevance: high · basis: full text

**Problem:** Closed-loop automated-driving simulation needs distributed co-simulation; FMI lacks distributed support.
**Approach:** DCP-compliant interface between CARLA (as DCP slave via Python wrapper) and xMOD co-simulation master.
**Tools and scale:** CARLA on Linux, xMOD on Windows, DCP; several virtual vehicles with CACC controllers.
**Results:** Several CARLA vehicles controlled by external CACC; standard compliance verified; DCP meets major interface requirements.
**Limitations and future work (as stated by the authors):**
- A detailed study of saved effort and pros/cons of DCP versus alternative/proprietary couplings should be conducted.
**Overlap with our engine:** none — co-simulation standards for AD, no V2X security.
**Novelty implications:** Supports our argument that multi-tool coupling is costly; no claims ruled out.
