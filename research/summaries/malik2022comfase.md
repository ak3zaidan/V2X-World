# ComFASE: A Tool for Evaluating the Effects of V2V Communication Faults and Attacks on Automated Vehicles
Mateen Malik, Mehdi Maleki, Peter Folkesson, Behrooz Sangchoolie et al. · 2022 52nd Annual IEEE/IFIP International Conference on Dependable Systems and Networks (DSN) 2022 · https://doi.org/10.1109/dsn53405.2022.00029 · relevance: medium · basis: abstract

**Problem:** This paper presents ComFASE, a communication fault and attack simulation engine. ComFASE is used to identify and evaluate potentially dangerous behaviours of interconnected automated vehicles in the presence of faults and attacks in wireless vehicular networks.
**Approach:** To demonstrate the tool, we present results from a series of simulation experiments, where we injected delay and denial-of-service attacks on wireless messages exchanged between vehicles in a platooning application.
**Tools and scale:** SUMO, Veins, Car-Hacking/CAN dataset (mentioned in abstract)
**Results:** This paper presents ComFASE, a communication fault and attack simulation engine. To demonstrate the tool, we present results from a series of simulation experiments, where we injected delay and denial-of-service attacks on wireless messages exchanged between vehicles in a platooning application.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** strong — Fault/attack injection engine on OMNeT++/Veins/SUMO that injects delay and DoS attacks on V2V and measures safety implications for automated vehicles (platooning); this is the attack-to-safety closed loop of direction 1/2, without PKI, revocation or MA decisions.
**Novelty implications:** We cannot claim to be the first to inject V2X attacks in a coupled network/traffic simulator and measure safety effects. Left open: PKI/SCMS-in-the-loop revocation, detection-to-neutralisation latency, and exact counterfactual reruns.
