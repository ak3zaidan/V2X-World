# Misbehavior detection and attacker identification in vehicular ad hoc networks
N. Bißmeyer · PhD thesis, Technische Universität Darmstadt 2014 · https://doi.org/10.26083/tuprints-00004257 · relevance: high · basis: full text

**Problem:** Detect location-based misbehaviour from authenticated insiders on VANET nodes, and identify and exclude attackers or faulty nodes centrally despite pseudonyms.
**Approach:**
- A module-based local framework: maximum communication range 1 km, maximum transmission delay 500 ms, maximum beacon frequency 15 Hz, suddenly appearing station 200 m, plausible movement 5 m (Kalman prediction), 111 m/s, 10° (Table 3.3, PDF p.73).
- Vehicle-overlap and particle-filter checks.
- A central Misbehavior Evaluation Authority (MEA) that acts only when enough independent reporters confirm an event, with conditional pseudonym resolution.
**Tools and scale:**
- **Real field data.** The simTD FOT, with 120 vehicles and 100 RSUs over 76 days and no attacks (PDF p.53), plus dedicated attack trials with test vehicles.
- Simulation (VSimRTI) for the attack studies.
**Results:**
- The FOT gave "a false-positive rate of ≈9.25%" of processed messages (PDF p.75).
- Of those false positives, 88.2% were transmission-delay violations caused by unsynchronised or overloaded units, and 6.2% were range violations from RSUs testing at high power. Plausible-movement violations were 1.8% (Fig. 3.8, PDF p.76).
- Movement-check false positives were ≈1.6‰ (PDF p.178).
- Suddenly appearing stations come from shadowing and "should probably not be used to exclude vehicles" (PDF p.77).
**Limitations and future work (as stated by the author):**
- "the message-based false-positive rate exceeded the expectations. The main reason for this was the unreliable transmission of mobility data and its inaccuracy" (PDF p.180).
- MEA report evaluation "only been evaluated in a proof-of-concept manner".
- Policies for central evaluation and pseudonym resolution are still needed (PDF p.180).
**Overlap with our engine:** strong. Benign trigger rates of plausibility checks on real vehicles, and a k-reporter central authority.
**Novelty implications:** Rules out "first real-data benign trigger rate of V2X plausibility checks" and the generic claim that transmission effects drive benign triggers. Leaves open the US SM-MBR 1 m rule set, the SCMS reporting chain and LA-linked revocation, and wrongful-exclusion counts at scale. See `synthesis/check-paper1-preemption.md`.
