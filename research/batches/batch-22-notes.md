# Batch 22 notes

All 378 papers in this batch are catalogued at medium relevance; every summary is abstract-based (109 had no abstract and are summarised from title/metadata only). Most are control-theoretic resilient platooning, ML intrusion detection, blockchain authentication, and automotive TARA/attack-tree methodology.

## 1. Stated limitations and future work

None of the abstracts in this batch states a limitation or future-work item explicitly; no full texts were read because no paper is high relevance. Gaps observed (not author-stated):
- Attack-to-traffic papers (wang2019extended, tan2025jamming, tan2025congestion, ali2025microscopic, zhao2021evaluating, khan2025evaluating, wang2025effects, jayaratne2024simulation) model attacks without a radio stack or PKI/revocation loop.
- Pseudonym/linkability papers (miao2018vehicle, haidar2019risk, chaurasia2008maximizing, chaurasia2009optimizing, kumar2019context) are not evaluated against SCMS butterfly-key provisioning.
- TARA/attack-tree papers (chlup2022threatget, kern2021model, ebrahimi2022identification, oberti2023psp, saulaiman2025integrated, cheah2021cybersecurity) rate risk qualitatively, not from simulated impact.
- Verification-load papers (li2011ramv, lee2011performance, surapaneni2024sebake) predate or omit PQ signatures.

## 2. Strong overlap

None. Closest partial overlaps:
- ghanavati2017pde: Attack-to-traffic impact, but macroscopic PDE modelling rather than closed-loop microscopic simulation with PKI.
- chlup2022threatget: Relevant to the ISO/SAE 21434 runner-up direction (tool-based threat analysis), but not simulation-quantified.
- chaurasia2009optimizing: Pseudonym change is relevant to cross-layer linkability; our engine already executes pseudonym change and could evaluate such strategies.
- cheah2018formalising: Relevant background for quantified ISO/SAE 21434 ratings.
- barnwal2019publish: Addresses false-data impact on rerouting; our engine models misbehaviour with traffic impact but at message/PKI level.
- kern2021model: Related to the 21434 runner-up; not simulation-based quantification.
- kashyap2022detection: Traffic-level attack detection; macroscopic, not V2X message level.
- maleki2022modeling: Attack-to-traffic impact; our engine also runs jamming in a closed loop.
- jayaratne2024simulation: Close to the quantified-21434 runner-up: uses traffic simulation to rate attack impact. Our differentiator would be radio+PKI ground truth.
- vogt2021comprehensive: Quantified risk method; relevant to 21434 runner-up, not simulation-based.
- tan2025congestion: Uses IDM-based traffic with attacks; we have IDM/MOBIL with in-loop attacks.
- ernst2017framework: Traffic-simulated attack impact; infrastructure rather than V2X/PKI.
- ebrahimi2022identification: Relates to 21434 runner-up.
- saulaiman2025integrated: Related to 21434 runner-up.
- aslam2019empirical: Radio-to-application coupling like ours, without security.
- hua2023rogue: Misbehaviour detection without PKI revocation loop.
- anonndonboard: Our LTE-V2X stack should conform to these requirements; standard rather than research.
- liu2025cooperative: Position-plausibility, akin to misbehaviour checks.
- bahamou2016vehicular: Relates to linkability and quantified risk.
- haidar2019risk: Directly relates to the cross-layer linkability runner-up; qualitative risk analysis, not simulated.
- li2020empirical: Attack-to-traffic impact with CV data spoofing; our engine runs such attacks with signed messages in the loop.
- tang2025effect: Traffic-flow impact of attacks, analytical.
- wang2025effects: Attack-to-traffic impact; no PKI.
- tan2025jamming: IDM-based attack impact.
- chaurasia2008maximizing: Pseudonym change; relevant to linkability.
- dubosarskii2021jamming: Couples mobility and jamming, analytical.
- oberti2023psp: Relates to 21434 runner-up.
- m2014efficient: Pseudonym scheme distinct from SCMS butterfly keys.
- miao2018vehicle: Directly relevant to cross-layer linkability: shows BSM-trajectory linking despite ID change.
- kumar2019context: Pseudonym change; linkability.
- vivek2022evaluating: Network-level attack impact.
- chen2024machine: Data-plausibility misbehaviour detection.
- lee2011performance: We implement GeoNetworking with 1609.2 security; overlaps the safety-cost-of-security direction at the beacon level.
- cheah2021cybersecurity: Relevant to quantified ratings runner-up.
- memary2025trcoin: Trust-based alternative to MA revocation.
- ghanavati2018pde: Macroscopic attack impact.
- wang2019extended: IDM-based attack modelling, like our traffic layer but without radio/PKI.
- ali2025microscopic: Microscopic traffic plus attacks; no radio/PKI.
- bernardeschi2025hardness: Post-quantum in vehicles, but OTA not V2X SCMS.
- silva2026spoofer: Cross-modal misbehaviour detection; PHY not in our engine.
- li2011ramv: Directly relevant to verification-policy part of the safety-cost-of-security direction.
- wang2021formulating: Attack-to-traffic modelling.
- abdelkreem2024received: Physical-plausibility misbehaviour detection; our radio layer produces RSSI ground truth.
- khan2025evaluating: Attack-to-traffic impact.
- tandon2025palct: Pseudonym scheme, not SCMS-compatible.
- qiao2026traffic: Attack-to-traffic modelling.
- calandriellondefficient: Pseudonym certificate authentication like 1609.2/SCMS.
- kerner2009effect: Early coupled traffic-communication simulation.
- guirguis2012stuck: Attack-to-traffic impact.
- laszka2018detection: Infrastructure attack-defence modelling.
- dong2019cyber: Impact assessment; relevant to quantified ratings.
- bagheri20205g: Background for our NR-V2X stack.
- thodi2020noticeability: Attack-to-traffic with detectability tradeoff.
- saifuddin2020performance: LTE-V2X radio performance, which we model.
- zhao2021evaluating: Attack-to-traffic-safety impact in simulation.
- boeira2021exploiting: Formal protocol analysis; we validate by test vectors instead.
- wang2021infrastructure: Position misbehaviour detection.
- xiang2022v2xp: Adversarial V2X scenario generation; our engine generates attack scenarios at the message level.
- dickey2022wiggle: Physical verification complements PKI; adjacent to misbehaviour detection.
- dutta2023location: Position misbehaviour detection.
- abouelnaga2023security: Background for 21434 runner-up.
- shui2024design: Communication-control coupling.
- chopinet2024susceptibility: We run 802.11p jamming in-loop.
- mohammed2025vehicular: Co-simulation of disturbances; partially like our engine.
