# Critical Re-Engineering and Experimental Validation of Physical Layer Security Aware Geographic Routing in VANETs Using OMNeT++, INET, Veins, and SUMO
Driss El-Ghouchma, Mourad Benmenssour, O. Zytoune · IEEE International Conference on Circuits and Systems for Communications 2026 · https://doi.org/10.1109/iccsc71566.2026.11649962 · relevance: high · basis: abstract

**Problem:** A physical-layer-security routing method was only validated with Monte Carlo maths.
**Approach:** Re-implementation in SUMO + Veins + OMNeT++ + INET: GPSRCS baseline and UFRPLS utility-based secure forwarding with a waiting mechanism. (summarised from the abstract only)
**Tools and scale:** SUMO, Veins, OMNeT++, INET.
**Results:** UFRPLS secure-hop ratio 1.0 for delivered packets; 2-3x higher average minimum secrecy capacity at alpha=0.5; delay up and PDR down at higher alpha.
**Limitations and future work (as stated by the authors):**
- Secrecy-reliability tension is an open design challenge.
- Documents implementation corrections of the original work.
**Overlap with our engine:** none — PLS routing.
**Novelty implications:** Example that re-validation in full-stack simulators changes conclusions; supports our full-stack argument.
