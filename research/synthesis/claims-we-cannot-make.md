# Claims we cannot make

Each line names a claim the thesis or a paper must **not** make and the prior work that rules it out. Each also gives the narrower claim that survives, where one does. Ids resolve to `summaries/<id>.md`. "(t)" marks a ruling paper summarised from its title only, and "(a)" one summarised from its abstract only. Read those in full before citing them as the reason.

## The engine as a simulator

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| First single-process (integrated, not co-simulated) traffic + V2X radio simulator | kerner2008testbed, kerner2009effect (a) | A single-process engine that also contains a validated SCMS, with deterministic exact reruns |
| First simulator integrating an SCMS with traffic and network simulation | abdo2024vehicular (VESNOS) (a); mushrall2018emulab (SCMS emulation lab) (a) | SCMS fidelity validated against official test vectors and SCMS Manager trust lists, inside a city-scale closed loop (pending a full read of VESNOS) |
| First integrated traffic/V2X security testbed or co-simulation | ahmad2025end, ahmad2025opencams (OpenCAMS); oruganti2019hardware (a); ching2020enabling (t); ming2018general (a); malik2022comfase (a) | Same as above |
| First V2X simulator with standard-compliant signed messages | riebl2016improved (Artery/Vanetza) (a) | Full PKI lifecycle, not just message signing |
| First 3D game-engine V2X simulator with IDM/MOBIL and ray-traced radio | egealopez2019vehicular (Veneris) | n/a (relevant to the V2 world-simulator vision) |
| First single-engine deterministic multi-modal simulator | gao2025great (Great X) (a) | Determinism across traffic, radio **and PKI** with per-message joint ground truth |
| First multi-RAT full-stack V2X simulator validated against measurements | pegurri2026van3twin (VaN3Twin) | Three V2X stacks plus SCMS in one process |
| "Veins-matched PHY/MAC" implies real-world fidelity | solida2026demystifying (Veins overestimates RSSI, about 18% fewer messages than a living lab); mavromatis2017agile | Matched to Veins/OpenCV2X; field calibration is separate work (ylmaz2025simulated has USDOT LTE-V2X field data) |
| First joint mobility + network V2X dataset with synchronised ground truth | anjum2026mint (MINT-V2X); stenhammar2026advancing | Joint traffic + radio + **PKI** per-message ground truth with counterfactual labels |
| First to show simulation settings change security conclusions | baharlouei2022exploring (a); bello2026ns3learn (radio realism doubles hard braking) | A systematic, many-factor version (N3) |
| Deterministic replay to compare security variants is new | cirne2018loop (trace-based, open loop) (a) | Closed-loop same-seed counterfactual reruns |
| First systematic configuration / statistical-power method for VANET co-simulation | xu2026cost | Paired same-seed designs that reduce replications |

## SCMS, PKI and credentials

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| The SCMS design (RA, PCA, LA, MA, butterfly keys, linkage-value revocation) as a contribution | whyte2013security (t); brecht2018security; anon2022ieee, anon2023ieee (IEEE 1609.2.1 / 1609.2) | Executable, validated implementation used for experiments |
| First standards-conformant SCMS + end-entity implementation with timing measurements | chen2023performance | Timing inside a traffic/radio closed loop |
| First implemented comparison of US SCMS and EU CCMS | chen2024performance; chen2026comparison (adds C-SCMS) | n/a |
| Only academic implementation interoperating with the real SCMS Manager trust lists | chen2026comparison (RCA certificate in the SCMS Manager test CTL and EU ECTL) | Validation of a full simulated backend against those lists |
| First implemented, scalable VPKI backend | khodaei2017secmace, khodaei2023secmace, noroozi2018vpkiaas, anon2013vespa | US butterfly-key SCMS rather than EU-style VPKI |
| First evaluation of pseudonym reload over cellular or while driving | haidar2021performance (a), haidar2019c (a), haidar2021experimentation (a) | Simulated top-ups at city scale (their field data can validate the model) |
| First butterfly-key provisioning performance study | hammi2017butterfly (t) | n/a |
| First P2PCD / certificate-chain distribution evaluation, or first DoS on it | yoshizawa2022handling (a), bittl2016efficient (a), bittl2015efficient (t) | P2PCD over the air with PQ chain sizes |
| Unified / improved butterfly keys, ACPC activation codes or LA-free revocation as new mechanisms | simplicio2018unified, simplicio2019acpc, cunha2020uacpc, wantoro2022efficient, junior2020privacy, simplicio2018privacy, wan2023escort | Executing and comparing them in closed loop |
| Linkage-based revocation is attack-free | junior2020privacy, simplicio2018privacy (birthday attacks on SCMS revocation) | n/a |
| Butterfly-key provisioning is secure as standardised without qualification | barreto2021schnorr (existential forgery under some conditions); boldyreva2024provable (secure with small modifications) | Cite both |
| First SCMS threat model | furtado2018threat (a) | A quantified one |
| First SCMS cross-certification / multi-root proposal | jha2018pseudonym (a) | Simulated multi-provider operation |
| First multi-PCA issuer-linkability analysis | huang2025privacy (a), huang2025interleaved (a) | n/a |
| First threshold-signing VPKI; first V2X use of FROST (corrected after critique) | ajibuwa2023resilient (full text via arXiv 2308.02711: FROST t-of-n signing across clustered VPKI CAs); stinson2001provably (threshold Schnorr implicit certificates, 2001) | Threshold authorities wire-compatible with IEEE 1609.2/1609.2.1 (threshold ECQV, CGGMP21) inside a running SCMS. Do not say "no V2X paper uses FROST" |
| SCMS provisioning load as an unrecognised problem | yu2026v (V-PASS) (a) | Measured provisioning load and failure modes |

## Revocation and misbehaviour

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| First detection → containment → eviction concept | raya2007eviction | n/a |
| First MDS-triggered revocation framework with an MA and CRLs | hasrouny2019misbehavior (a) | US SCMS linkage-seed path, measured |
| First simulation of detection, reporting **and reaction** | willecke2023a4md (A4MD, ETSI) (a); kamel2020simulation (F2MD) | US SCMS, J3287 reporting, linkage CRLs, safety attribution |
| First quantification of the revocation vulnerability window or revocation time | tesei2023transparent, scopelliti2024efficient, scopelliti2024demo, wasef2011managing, didouh2022blockchain, lopes2025novel (a), imran2026secure (a) | End-to-end, stage-decomposed time to neutralise in the SCMS, tied to safety |
| First city-scale CRL dissemination-latency study | khodaei2018efficient, khodaei2020scalable, haas2011efficient (a), eckhoff2013smartrevoc (a), robertndsegmented (a) | n/a |
| First study linking revocation to a safety application | haas2012global (t), must be read | Pending that read |
| First to model the risk of accepting revoked certificates vs CRL age | ganan2013certificate | n/a |
| First closed-loop detection → reaction study showing collision prevention depends on timing | boeira2018mitigating (a), wolf2020securing (a), franceschini2024neural, ghiro2026physics (a), zhang2024simulation, zhang2024enhancing, trkulja2025vehicle (a), hasan2026vehicle | Infrastructure (MA/PKI) reaction rather than local reaction |
| First MA dataset, ML at the MA or central report evaluation | haidar2020dare (a), kamel2019misbehavior (a), bimeyer2012central (a) | MA decisions that drive real revocation, evaluated for safety |
| First misbehaviour report format | kamel2018misbehavior (a); anon2024v2x (SAE J3287) | n/a |
| First LLM / game-theoretic conviction gate for insiders in NR C-V2X | carie2026hierarchical | n/a |
| First credentialed-insider attack + MBD in co-simulation | kalogiannis2022attack (a) | Insider attacks with valid SCMS credentials at city scale |
| First discreditation / framing attack on V2X trust | twardokus2021targeted (a); kamel2020detection (F2MD "MA Stress" fake-report attack with rotating pseudonyms, thesis p56); 5GAA MBD white paper 2022 p12 (slander combined with Sybil); Intel US12489779B2 and Qualcomm US12260747B2 (MA-side slander defences) (corrected after critique) | Which standard detector evidence can frame an honest vehicle under a proof-based MA, and the corroboration rule that closes it. twardokus2021targeted attacks a naive reputation counter, so "jamming alone gets an honest vehicle revoked" is not claimable against a proof-based MA |
| First spectrum-misbehaviour reporting via the SCMS | noori2020reporting, noori2021reference, noori2025detection (a) | n/a |
| First simulation-generated MBD dataset / benchmark | heijden2018veremi, kamel2020veremi (a), hermann2026veremi, yuce2025state (a), novo2026spade, guven2025creating, jerbi2026poster (a) | Counterfactual-paired, cross-layer labelled benchmark |
| Existing MBD benchmarks are sound | muzammil2026code (structural artefacts in VeReMi) | Must cite and avoid those artefacts |
| First impact-based (not precision/recall) MBD evaluation | wehmer2021practical (a) | Exact counterfactual impact (N1) |
| First causal framing of MBD effectiveness | wehmer2023fast (a) | Executed, exact counterfactuals in simulation |
| First repeatable simulation security benchmark for cooperative driving with safety outcomes | malik2025security | Beyond CACC jamming: PKI attacks, city scale, counterfactual pairs |
| First PQ V2X security dataset | abdelhakeem2025pq (a) | n/a |

## Attacks and their impact

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| First closed-loop attack-in-simulation with traffic or safety impact | bimeyer2011simulation (t), ming2018general (a), amoozadeh2015security (a), heijden2017analyzing, malik2022comfase (a), boeira2017impact (a), dunn2017regular (a), iqbal2022simulating, wellensmiles2026assessing (a), chen2018exposing, huang2021impact | Attacks that pass cryptographic checks, with PKI response in the loop |
| First matched attack-vs-baseline safety comparison in NR-V2X/traffic co-simulation | bello2026human | Exact same-seed counterfactuals, insiders, revocation |
| First LTE-V2X / NR-V2X scheduling or resource-selection DoS | trkulja2020denial, twardokus2022vehicle, twardokus2023protecting, yoon2021evasive (a), djaidja2022adaptive (a), talukder2024symjam (a), sun2024interleaved, ashik2026analyzing | Combined with verification exhaustion and PQ sizes |
| First NR sidelink security specification analysis | bitsikas2025security | n/a |
| First NR sync-spoofing attack | hossain2026sync (a) | n/a |
| First protocol-compliant DoS degrading a real FCW | tine2025real | Simulated city-scale version with verification queues |
| First verification-flooding (clogging) DoS on signature checks | he2012mitigating (a), jin2019dos (a), jin2024future (a), jin2025accountable, zhou2026efficient (a) | NR + PQ + closed-loop safety |
| First cross-technology (C-V2X on DSRC) jamming measurement | sousa2025jamming (a) | n/a |
| First spoof-to-collision risk quantification for platoons | bhardwaj2026quantifying (a), yang2021risk | City-scale closed loop with MBD |

## Cost of security and post-quantum

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| First to link V2X security overhead to safety | papadimitratos2008impact (a), petit2009analysis (a), petit2010analysis (a), baee2019broadcast, petho2022considering (a), petho2024investigating | The whole-system regime map |
| PKI overhead materially degrades safety in general | petho2024investigating (small effect, negligible near 100% PDR) | Only where the thesis shows it, with regime boundaries |
| First verify-on-demand, selective verification or certificate omission policy | krishnan2011verify (t), ko2021avod (a), feiri2012evaluation (a), feiri2012congestion (a), pollicino2021hardware, jha2018pseudonym (a), bittl2015efficient (t) | Joint policy × threshold × PQ evaluation with safety outcome |
| First to show OBU hardware cannot verify every message | pollicino2021hardware | n/a |
| First PQ or hybrid V2V authentication, or first spectrum-impact evaluation | twardokus2024cryptography, bindel2022drive (t) | n/a |
| First PQ in IEEE 1609.2 hardware-in-the-loop platform | twardokus2024demo | n/a |
| First hybrid PQ BSM authentication over C-V2X with certificate fragmentation | twardokus2025quantum (a) | n/a |
| First full-stack PQ C-V2X PDR/latency study; "only Falcon fits" | abrar2026deployment | NR-V2X, corridors, safety outcomes (their future work) |
| First identification that PQ certificates break 1609.2 packet budgets | yoshizawa2023post (a), kim2023signature, anonndproposing (a) | n/a |
| First PQ SCMS design / PQ butterfly-key provisioning | barreto2018qscms (t), oliveira2019qscms | System-level simulation of it |
| First PQ-aware SCMS or selective per-certificate-type PQ migration | fikry2026post (a) | Full-stack, city-scale, safety-outcome evaluation (theirs is analytical at 50 vehicles) |
| First hybrid PQ/ECC SCMS certificates or a PQ replacement for butterfly expansion | chen2024hybrid, chen2026pseudonym, chen2024pqcmc, lonc2025hybrid | n/a |
| Lattice implicit certificates for butterfly keys are straightforward | bindel2022need (failed constructions) (a) | n/a |
| First linking of PQ signature cost to required vehicle spacing | wang2026reproducibility (a); companion paper to be found | Simulation-based (not analytical) evidence |
| First linking of PQ packet size to NR sidelink capacity | dongre2025adaptive (a) | n/a |
| First adaptive PQ selection for vehicular networks | sengupta2026adaptive | End to end with SCMS issuance |
| First PQ vs pseudonym-PKI comparison on PDR / CBR / revocation overhead | samara2025post (a) | SCMS-faithful version with safety outcomes |
| First study of the safety cost of privacy (pseudonym change) | jemaa2017study (a), emara2017safety (a), emara2015evaluation (t), xin2019balance (a), eckhoff2016marrying (a) | n/a |
| First MBD-response safety/efficiency trade-off | wolf2020securing (a) | n/a |

## Privacy and linkability

| Claim we cannot make | Ruled out by |
|---|---|
| First cross-layer (MAC + pseudonym) linkability attack | liu2018cross (a), liu2020marp (a), liu2018mac (a) |
| First physical-layer pseudonym linking | bloessl2015scrambler, baldini2017analysis (a), vaas2018poster (a) |
| First multi-radio pseudonym association | yieh2019multi (a), zoccoli2025radar (a), yieh2018vehicle (a) |
| First issuer (PCA-id) linking | huang2025privacy (a) |
| First showing static fields defeat the C-ITS pseudonym scheme | escher2021well |
| First analysis of certificate-distribution vs pseudonym-change interaction | bittl2017mutual (a) |
| First location-privacy evaluation framework in a VANET simulator | eckhoff2014open (a), emara2016poster (a) |

## Counterfactuals, world models and surrogates

> **(corrected after critique)** "Exact" paired counterfactuals are overstated. The engine keys each random stream by entity, not by event (`crates/v2xw-core/src/rng.rs:1-58`). Actor and node ids follow realised insertion order (`v2xw-mobility/src/engine.rs:3147-3150`, `v2xw-engine/src/run.rs:3168`), and several draw counts depend on simulated state. Pairs are exact per unit only for open-loop replay, backend-only changes (once report streams are re-keyed) and the near field of a closed-loop discard. Everywhere else they are coupled, distribution-level comparisons over seeds. FCW/EEBL/IMA warnings also do not change driving yet (`run.rs:4324-4333`). See `critique-methods.md` and `final-three-approaches.md` §5.

Every "exact" in the right-hand column of this file is subject to that note.

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| First simulation benchmark with matched factual and counterfactual ground truth for world models | zhang2026driving | First for networked V2X with traffic, radio and PKI and long-horizon propagation |
| Simulating counterfactuals with a digital twin is a new idea | laudy2026digital (a) | Exact same-seed V2X instantiation |
| First factual/counterfactual branches sharing exogenous noise | ma2026twin (a) | Done with an exact simulator rather than a learned model |
| First causal or counterfactual network simulation | alomar2022causalsim, bothra2022veritas | Exact counterfactual truth that needs no RCT |
| First counterfactual KPI estimation in wireless | hou2024if, qchohi2026confounding, agudelo2026trustworthy, ak2024if | Ground truth to validate them |
| First world model for (mobile) wireless networks | chai2025mobiworld (a), bouton2026mobile (a), alagharu2026network | One that includes security state |
| First neural surrogate of V2X communication simulation | matsumoto2026neural (a) | Counterfactual-accurate, security-aware surrogate |
| First traffic-simulator surrogate world model | ranjan2026enactor (a) | Joint traffic + radio + PKI |
| First GNN network digital twin | rusek2019routenet, ferriolgalmes2022routenet and the RouteNet family | n/a |
| First synthetic counterfactual benchmark generator | thumm2026dotime (generic) (a) | Physically grounded V2X one |

## LLM agents

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| First LLM agent that configures, runs and interprets network simulations | rezazadeh2024genonet, rezazadeh2025generative, han2026ns3 | Over a joint traffic + radio + PKI engine |
| First LLM agent that configures SUMO | li2024chatsumo (a), jeong2025agentsumo, liu2026simskill (a) | n/a |
| First LLM-driven V2X attack/defence simulation | gao2025exploring (a) | With radio stack and PKI and ground truth |
| First LLM-based V2X misbehaviour detection | hu2025llm (a), hamhoum2024mistralbsm (a) | n/a |
| First LLM V2X fault / telemetry analysis | hazarika2025llm (a), wu2025v2x | Cross-layer security root cause scored by counterfactuals |
| First LLM troubleshooting / root-cause benchmark | wang2025network, sana2025reasoning (a), tseng2026sade (a) | V2X cross-layer RCA benchmark |
| First LLM network-configuration benchmark | wang2024netconfeval, aykurt2024netllmbench, liu2026netconfarena, twabi2026netagentbench | n/a |

## Risk assessment

| Claim we cannot make | Ruled out by | What can still be claimed |
|---|---|---|
| First simulation-based automotive cybersecurity risk assessment | jayaratne2024simulation (a), kamtam2025wolves (a), petho2024quantifying (a) | SCMS-specific ratings from counterfactual harm and executed attacks |
| First automated TARA-to-test generation | marksteiner2023tara (a) | n/a |
