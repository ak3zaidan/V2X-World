# Batch 08 notes

378 papers summarised (62 from full text, the rest from abstracts or metadata only; many catalogue entries tagged high were reassessed as medium/low, and a few are off-topic keyword matches).

## 1. Stated limitations and future work

- hussein2024da: Calls for more research on platoon communication security (generic).
- jedh2024improvement: Future work: impact of vehicle speed, spoofed value, IDS success rate and response time on mitigation efficacy
- jedh2024improvement: integrate real-time IDS into resilience mechanisms rather than estimation.
- bhupathiraju2024vulnerability: Limited to indoor scenarios and low speed
- bhupathiraju2024vulnerability: higher power may be needed outdoors
- bhupathiraju2024vulnerability: outdoor evaluation left as future work
- bhupathiraju2024vulnerability: aiming IR lasers at distance is hard.
- kosari2022modelling: Informal safety analysis used to identify vulnerability modes
- kosari2022modelling: quantitative analysis suggested to guide designers.
- ruan2020securing: None stated explicitly.
- sun2021communication: Cannot ensure string stability in an adversarial environment
- sun2021communication: open problem whether a switching signal can achieve (probabilistic) string stability under communication attacks.
- bai2022securing: None explicit in extracted sections.
- yuan2024real: Authors note the performance cost of real-time tracking.
- marchiori2024work: Future work: refine threat models, innovate defences, assess AI adaptability to emerging adversarial strategies.
- braiteh2025secure: None explicit in extracted sections.
- zhou2025runtime: Highlights importance of driver intervention and basic safety mechanisms
- zhou2025runtime: no explicit limitation list in extracted sections.
- miller2019implementation: Enables future work on implementing attacks and mitigations.
- xiang2022blockchain: None explicit in extracted sections.
- placzek2017detection: Integrate more sophisticated driver behaviour models
- placzek2017detection: automatic parameter calibration from collected data
- placzek2017detection: test in more realistic scenarios with various signal control algorithms.
- memon2017dynamic: None explicit beyond speed analysis.
- memon2017optimal: None stated.
- marojevic2018c: Security mechanisms must be incorporated and enforceable
- marojevic2018c: overhead on resource-constrained service needs study.
- huang2020impact: None explicit in extracted sections.
- liu2020joint: None explicit.
- ma2020sequential: None explicit in extracted sections.
- qu2021attacking: More diverse action spaces (volume, speed, position)
- qu2021attacking: test on other DRL ATCS policies
- qu2021attacking: real-life considerations.
- irfan2022reinforcement: Test on different intersection architectures
- irfan2022reinforcement: deploy against DRL-based ATSCs
- irfan2022reinforcement: next step is attack detection and mitigation.
- li2023among: None explicit
- li2023among: generalisability claimed.
- zolotavkin2023improving: None explicit in extracted sections.
- zhao2023malicious: None explicit.
- malik2025security: Consensus reduces traffic density and relies on faultable sensors
- malik2025security: benchmark covers jamming against CACC only.
- tao2025gcp: None explicit.
- bitsikas2025security: Calls for security re-evaluation of NR V2X and future research building on it.
- liu2026adversarial: Dual-use
- liu2026adversarial: evaluated mainly in digital simulation.
- yu2026all: None explicit in extracted sections.
- breier2026automotive: None explicit in extracted sections.
- tao2026learning: Removal attacks have constrained optimisation space
- tao2026learning: effectiveness bounded by benign CAVs' joint coverage
- tao2026learning: future work: camera-based perception and effective defences.
- abbas2015simulation: Not in extracted sections.
- eckermann2019performance: None explicit in extracted sections.
- weber2021vanet: Lists challenges to improve VANET simulation quality (in paper).
- olaverrimonreal2018connection: Future lines of work stated in paper (not in extracted text).
- ameixieira2014harbornet: Lessons learned and recommendations (in paper).
- petrov2021performance: None explicit in extracted sections.
- boeglen2011survey: Few V2V measurements for statistically significant statements
- boeglen2011survey: regional variation needs more measurements
- boeglen2011survey: multi-antenna systems must be considered.
- kerner2008testbed: Accuracy/time trade-off via reception threshold.
- petrov2022benchmarking: None explicit in extracted sections.
- egealopez2019vehicular: Future: evaluate influence of physically accurate models on traffic results
- egealopez2019vehicular: implement diffraction and validate with measurements
- egealopez2019vehicular: performance optimisation and interactive simulation.
- shah2019real: None explicit in extracted sections.
- kueppers2024v2aix: Some drivers may have deactivated V2X
- kueppers2024v2aix: only VW Group vehicles equipped at the time.
- fernandezcarames2010fpga: None stated.
- hegde2020artery: Not in extracted sections.
- mccarthy2021opencv2x: Need to investigate networks with mixed application traffic to schedule resources while maintaining reliability.
- boban2011modeling: Areas not optimally represented in state-of-the-art simulators (as above).
- yang2021multi: Future work stated (truncated in extraction).
- wang2019vehicular: Has limitations (unspecified in extract)
- wang2019vehicular: invites community improvement.
- hoque2018parallel: Only a few solutions implemented
- hoque2018parallel: ongoing implementation and scalability evaluation for metro-wide networks.
- gelbal2020hardware: Large-scale public road deployment data must be collected.

## 2. Strong overlap

- malik2025security: strong: a simulation-based, repeatable security benchmark of cooperative driving applications with recorded safety outcomes — the same framing as our benchmark direction (direction 3) and safety-under-attack measurement. Implication: We cannot claim the first simulation-based V2X security benchmark for cooperative driving; open: PKI-layer attacks, per-message ground truth, counterfactual pairs, city scale.
- kerner2008testbed: strong: a single integrated (not co-simulated) traffic + V2X radio simulator already existed in 2008; our single-process design is not new per se. Implication: We cannot claim the first single-process integrated traffic/network V2X simulator; our distinct claims must rest on PKI/SCMS integration, determinism, joint ground truth and counterfactual reruns.
- egealopez2019vehicular: strong: a game-engine-based 3D traffic + V2X simulator with IDM/MOBIL and ray-traced radio — overlaps our V2 2D/3D world-simulator vision and our IDM/MOBIL traffic. Implication: Cannot claim first 3D game-engine V2X simulator with IDM/MOBIL; open: PKI, security and determinism in such a 3D world.
