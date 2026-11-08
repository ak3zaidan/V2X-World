# Traffic, world and vehicle models

Code: `crates/v2xw-world`, `crates/v2xw-mobility`, `crates/v2xw-demand`, and the signal controllers in `crates/v2xw-engine`. Each model's
card carries its equations, parameter sources and validation; the citations below are the
ones the code and cards name. Validation against papers and field data:
`crates/v2xw-trafficval` ([../testing.md](../testing.md)).

## World

`v2xw_world::model::World` holds lanes (with centreline geometry, width, class masks and
speed limits), junctions, connections with their conflict matrix, signal plans,
crosswalks, buildings and terrain. It is built once, content-hashed, and read by every
other crate.

| Source | Module | Notes |
|---|---|---|
| OpenStreetMap XML | `osm` | streaming parser; `highway=*` speed defaults come from a named jurisdiction preset and the importer refuses to guess one; the bounding box fixes the world frame (origin at its south-west corner) |
| SUMO `net.xml` | `sumo` | keeps SUMO edge and lane ids so co-simulation maps lanes one to one; the module also contains an OpenDRIVE reader |
| procedural | `procedural` | grid, radial and random generators |
| DEM | `dem`, `los` | SRTM-30 and Copernicus GLO-30 rasters draped under the lanes; line of sight over terrain |

Drivable turn geometry is rounded into arcs (`curve`, AASHTO Green Book 2018 §9.5,
Table 2-2). `validate` reports geometry a vehicle or pedestrian cannot use and fidelity to
the source tags. The city library and jurisdictions are in `worlds/cities.yaml` and
`v2xw_world::jurisdiction`.

## Car-following

`crates/v2xw-mobility/src/carfollowing/`; selected per vehicle class through
`actors.vehicles.driving` (`carfollowing::library`).

| Model id | Model | Source |
|---|---|---|
| `mobility/car-following/idm` | Intelligent Driver Model (default) | Treiber, Hennecke & Helbing 2000, Phys. Rev. E 62, 1805 |
| `mobility/car-following/eidm` | Extended IDM | Salles, Kaufmann & Reuss, SUMO User Conference 2020 |
| `mobility/car-following/krauss` | Krauss stochastic, collision-free | Krauss, dissertation (SUMO's default model) |
| `mobility/car-following/gipps` | Gipps behavioural model | Gipps 1981, Transp. Res. B 15(2), eqs. (1)-(5) |
| `mobility/car-following/w99` | Wiedemann 99 psycho-physical (ten parameters) | Wiedemann 1999, as in PTV Vissim |
| `mobility/car-following/acc` | ACC, PATH multi-mode controller | Milanés & Shladover 2014, Transp. Res. C |
| `mobility/car-following/cacc-milanes`, `cacc-ploeg`, `cacc-path` | CACC on received V2X data | Milanés & Shladover 2014; Ploeg et al. 2011; Rajamani, *Vehicle Dynamics and Control* |
| `mobility/car-following/platoon-channel` | platoon string stability versus the V2X channel | composes the above |
| `mobility/car-following/by-class` | a model per vehicle class | |

The CACC laws use only what V2X delivered (`coop`): with no BSM or CAM from the radar
target younger than `max_age_s`, the follower falls back to the ACC law at the ACC time
gap. IDM presets include an urban set calibrated to HCM signalised discharge. Driver
heterogeneity, amber and red-running behaviour, and lamps are under
`mobility/native/driver/*`.

### Car-following equations

**IDM** (Treiber, Hennecke & Helbing 2000), the default. With net gap $s$, speed $v$ and
approach rate $\Delta v$:

$$
a = a_{\max}\Big[1 - \Big(\frac{v}{v_0}\Big)^{\delta} - \Big(\frac{s^*(v, \Delta v)}{s}\Big)^2\Big],\qquad
s^* = s_0 + s_1\sqrt{v/v_0} + \max\!\Big(0,\; T v + \frac{v\,\Delta v}{2\sqrt{a_{\max} b}}\Big).
$$

| Parameter | Car | Truck | Source |
|---|---|---|---|
| $v_0$ | 33.3 m/s | 23.6 m/s | Kesting, Treiber & Helbing 2010 (`preset: kesting-2010`) |
| $T$ | 1.5 s | 2.0 s | idem |
| $a_{\max}$ | 1.4 m/s² | 0.7 m/s² | idem |
| $b$ | 2.0 m/s² | 2.0 m/s² | idem |
| $s_0$ | 2.0 m | 4.0 m | idem |
| $\delta$ | 4 | 4 | idem |
| $s_1$ | 0 | 0 | base model |
| `a_min` | −6 m/s² | | `todo-calibrate` |

Under weather, $v_0$ is scaled by the desired-speed factor after the legal limit and the
driver's wish are reconciled, $T$ by the headway factor, and $b$ capped (FHWA Road Weather
Management bands). Optional Kesting 2010 enhancement (`enhanced`, coolness 0.99).

**Gipps** (1981): $v_a = v + 2.5\,a\tau(1 - v/V)\sqrt{0.025 + v/V}$,
$v_b = -B\tau + \sqrt{B^2\tau^2 + B[2(g - s_0) - v\tau + v_l^2/\hat B]}$ with
$\hat B = \max(3, (B+3)/2)$; the new speed is $\max(0, \min(v_a, v_b))$.

**Krauss** (SUMO default):
$v_{\text{safe}} = v_l + \dfrac{g - v_l\tau}{(v + v_l)/(2b) + \tau}$,
$v_{\text{des}} = \min(v + a\Delta t, v_0, v_{\text{safe}})$, then a random dawdle
$\sigma\,\Delta t\,\eta$, $\eta \sim U[0,1)$, drawn from the `mobility` RNG domain.

**ACC** (PATH, Milanés & Shladover 2014): speed control $a = -k_{sc}(v - v_0)$ beyond
120 m; gap control on $e = g - t_{hw} v - d_0$ by mode; a safety envelope on the emergency
deceleration.

**CACC**: Milanés & Shladover's PATH controller, $a = (k_p e + k_d \dot e)/0.1\,\text{s}$
with $e = g - h v$ and $\dot e = v_p^{\text{V2X}} - v - h a$ at $h$ = 0.6 s; Ploeg et al.
2011, $h\,\dot u = -u + k_p e + k_d \dot e + u_p^{\text{V2X}}$ at 0.7 s. With no BSM or CAM
from the radar target younger than `coop.max_age_s`, the follower degrades to the ACC law,
its time gap opening to 1.1 s over `degrade_ramp_s`.

## Lane change and lateral position

| Model id | Model | Source |
|---|---|---|
| `mobility/lane-change/mobil` | MOBIL incentive and safety criteria, keep-right rule | Kesting, Treiber & Helbing 2007 |
| (`lanechange::lc2013`) | SUMO LC2013 strategic, cooperative, speed-gain and keep-right motivations, with MOBIL's safety criterion | Erdmann 2015, *Modeling Mobility with Open Data*, pp. 105-123 |
| `mobility/lateral/sublane-sl2015` | sub-lane lateral model: two-wheelers filtering, lateral gaps | Semrau & Erdmann, SUMO 2016 |

**MOBIL** (Kesting, Treiber & Helbing 2007): a change is safe when the new follower's
acceleration after it is $\tilde a_n \ge -b_{\text{safe}}$, and wanted when

$$
(\tilde a_c - a_c) + p\,\big[(\tilde a_n - a_n) + (\tilde a_o - a_o)\big] + \Delta a_{\text{bias}} > \Delta a_{\text{th}},
$$

with $c$ the ego, $n$ the new and $o$ the old follower, $p$ the politeness and
$\Delta a_{\text{bias}}$ the keep-right term (zero unless enabled). Every acceleration comes
from the car-following model with each driver's own parameters. The lateral move is a
smoothstep $d(t) = d_0 + (d_1 - d_0)\,p^2(3 - 2p)$ over the change duration. Route-driven
(mandatory) changes are the engine's rule, not MOBIL's incentive.

## Intersections and signals

| Model | Where | Source |
|---|---|---|
| gap acceptance (`mobility/intersection/gap-acceptance-hcm`) | `intersection/gap_acceptance.rs` | HCM 6th ed. critical gaps and follow-up times, heavy-vehicle adjustment |
| pre-timed signals (`mobility/intersection/signal-fixed-time`) | `intersection/signal_fixed_time.rs` | the world's `SignalPlan` |
| conflict zones within a junction | `intersection/zones.rs` | path crossings from the conflict matrix |
| vehicle-actuated control, green-wave coordination | `crates/v2xw-engine/src/actuated.rs` | `traffic.signals` |
| NEMA dual-ring eight-phase controller | `crates/v2xw-engine/src/nema.rs` | FHWA Signal Timing Manual 2nd ed. (NCHRP Report 812), MUTCD 2009 |
| emergency-vehicle signal priority (SRM/SSM) | `crates/v2xw-engine/src/priority.rs` | NTCIP 1211 |
| abstract two-colouring (`mobility/intersection/two-coloring-legacy`) | `intersection/two_coloring.rs` | abstract tier |

Gap acceptance (HCM 6th ed.): a minor-stream vehicle enters when
$\min_{\text{conflicting}} t_{\text{to stop line}} \ge t_c + k\,t_f$, with $k$ the vehicles of its own
movement queued ahead (the composition is this card's reading of the two-value model; the
critical-gap table is secondary-sourced). NEMA gap-out, max-out and barrier rules follow
the FHWA Signal Timing Manual 2nd ed. §5.1 to §5.3, with yellow change
$y = t + v/(2a + 2Gg)$ (Equation 6-1).

## Vulnerable road users

`crates/v2xw-mobility/src/vru/`. Pedestrians walk by the social-force model (Helbing &
Molnár 1995), $\dot{\mathbf v} = (v_0\mathbf e - \mathbf v)/\tau + \sum \mathbf f_{\alpha\beta} + \sum \mathbf f_{\alpha B}$, with an elliptical
repulsion $V(b) = V_0 e^{-b/\sigma}$ from other pedestrians and $(U_0/R)e^{-d/R}$ from kerbs, on sidewalks and crosswalks; crossing on a signal, crossing into a gap, and
mid-block crossing (`midblock.rs`) are modelled with the gap each needs; groups,
cyclist speeds and population mixes are separate models (`mobility/vru/*`). Vehicles and
pedestrians read one crosswalk index, so "the vehicle can still stop" and "the vehicle
must yield" are the same computation.

## Routing and demand

| Model id | What |
|---|---|
| `mobility/routing/dijkstra` | shortest path on the lane graph (a node is a lane, an arc a permitted connection), honouring turn restrictions and class masks |
| `mobility/routing/dynamic-reroute` | re-planning at the next junction on a closure, travel-time update or timer |
| `mobility/demand/poisson-thinned` | Poisson arrivals thinned by a time-of-day multiplier; daily profiles from counted traffic (`demand/daily.rs`) |
| `mobility/demand/od-gravity` | uniform or gravity origin-destination choice |
| `mobility/demand/tr36885-drop` | 3GPP TR 36.885 evaluation drops for radio studies |
| `mobility/demand/{od-matrix, counts, gtfs, purposes, routes, composite, sumo-rou}` | `v2xw-demand`: VISUM/CSV OD matrices and gravity-Furness, routes from link counts with GEH, GTFS buses and street-level rail, trip purposes and tours, saved routes, `.rou.xml` |
| `demand/assign/iterative-due`, `demand/choice/{gawron, logit, msa}`, `demand/loader/{bpr, queue, micro}` | `v2xw-demand`: iterative dynamic traffic assignment to a stochastic user equilibrium with relative-gap convergence |

Kerbside stops (buses, taxis, double-parked deliveries) follow TCQSM 2nd ed. Part 4 and
HCM bus blockage (`kerbside.rs`). Network-level measures (MFD, stops, dwell) follow
Daganzo & Geroliminis 2008 and Edie 1963 (`network.rs`).

## Weather

`weather.rs`: speed and headway effects on driving per FHWA Road Weather Management.
The radio effect of rain (ITU-R P.838-3) is in [network.md](network.md).

## Node beliefs: GNSS and clock

| Model id | What | Source |
|---|---|---|
| `gnss/error/gauss-markov` (medium, high default) | first-order Gauss-Markov plus white noise per axis, sigma and tau fitted to measured quantiles at construction, environment scaling, confidence ellipse | Reid et al. 2019 |
| `gnss/error/ou-bias-legacy` | Ornstein-Uhlenbeck bias model, abstract tier | |
| `clock/drift/tcxo-ocxo` | node time is GNSS time (within the GPS SPS 30 ns at 95 % commitment) while the fix holds, oscillator drift in holdover; a library model the engine's node runtimes do not yet use ([Determinism and time](determinism-and-time.md#node-clocks-and-gnss-time)) | GPS SPS performance standard |
| `clock/drift/none` | node time equals GNSS time | abstract tier |

The Gauss-Markov GNSS error per axis is

$$
b_{k+1} = \alpha\,b_k + \sigma_b\sqrt{1 - \alpha^2}\,\mathcal N(0,1),\quad \alpha = e^{-\Delta t/\tau},\qquad
\hat{\mathbf p} = \mathbf p + \mathbf b + \mathcal N(0, \sigma_w),
$$

plus outliers at a uniform angle; the stated confidence is $2.4477\sqrt{\sigma_{\text{nom}}^2 + \sigma_b^2}$
from the receiver's own parameters, never from the bias it carries. Receiver quality,
outliers, bursts, outages, smoothing and the stated accuracy are set under
`actors.vehicles.gnss` ([reference](../configuration/reference/actors.md)).

## Tiers and scale

| Tier | Implementation |
|---|---|
| abstract | `mobility/kinematic/lane-follow` (`kinematic.rs`) |
| medium/high | `engine::NativeMobility`: Jacobi-updated micro simulation, decide pass in parallel, merged in actor-id order |
| mesoscopic | `mobility/meso/queue`: the Eissfeldt 2003 vehicle-based queue model, as SUMO MESO runs it |
| tiered | `mobility/tiered/meso-micro`: microscopic inside a focus region, mesoscopic outside (`scale`) |
| SUMO | `v2xw_interop::sumo::SumoMobility`: SUMO drives every vehicle over TraCI (`mobility.provider: sumo`) |

## Energy and emissions

Emission, fuel and EV energy models scored over the recorded trajectories
(`crates/v2xw-energy`, `v2xw energy`): [Energy and emissions](energy.md).

## Modelled versus simplified

| Modelled | Simplified or absent |
|---|---|
| microscopic car-following (IDM, EIDM, Krauss, Gipps, W99, ACC, CACC) on a Jacobi-updated step | no reaction-time action points or estimation errors in the IDM family (the EIDM's are left out); W99's original equations are unpublished, the open executable reading is used |
| CACC on what V2X delivered, degrading to ACC on stale messages | no platoon management protocol (join, leave, split) |
| MOBIL, LC2013 motivations, SL2015 sub-lane position | SL2015 reduced to its speed-gain core; no strategic sub-lane motivation |
| HCM gap acceptance, NEMA dual ring, actuated control, signal priority | gap acceptance critical gaps are secondary-sourced |
| social-force pedestrians with crossing behaviour | no attraction term, no jam state; a mid-block crosser walks straight at its desired speed |
| mesoscopic queues for city scale | no lane changes, overtaking or acceleration inside a meso segment |
| Gauss-Markov GNSS error fitted to measured percentiles | no satellite geometry or building-specific multipath; only environment classes |
| Dijkstra and dynamic rerouting; iterative DTA in `v2xw-demand` | the native router is one shortest path with no route-choice heterogeneity |

Every card's assumptions, limitations and parameters:
[Model parameters, mobility family](../configuration/reference/models.md#mobility).
