# Driver distraction: eyes-off-road glances (WP-39)

Status: 2026-10-28, P39.1 sources read and fit chosen. Plan:
`docs/design/frameworks/wp25-plus-build-plans.md` §WP-39. The process is built in P39.2
(`v2xw-mobility/src/drivers.rs`) and wired in P39.3.

## The process

Each driver alternates between two states:
- **on path**, eyes on the forward road;
- **off path**, eyes anywhere else (mirrors, instruments, a phone, a passenger, the
  roadside).

Each dwell is drawn independently when it starts. While the eyes are off path, the driver's
picture of the vehicle ahead is frozen at the last value seen, and the alarm rule (brake at
once when the scene asks for more than the comfortable deceleration) cannot fire, because
the driver does not see the scene. When the glance ends, the driver sees the scene again.
The response then goes through the driver's own reaction time and alarm rule, as in
`drivers.rs` §2.

| Dwell | Distribution | Parameters | Mean | Source |
|---|---|---|---|---|
| on path | inverse Gaussian, `f(x; μ, λ)` | μ 3.56 s, λ 1.50 s | 3.56 s | Morando, Victor & Dozza 2019, Fig. 6a (p. 6) |
| off path | lognormal, `ln x ~ N(μ, σ²)` | μ −0.21, σ 0.50 | 0.918 s (median 0.81 s) | the same, Fig. 6b |

These give the following, by calculation:
- **Glance onset rate:** 1/(3.56 + 0.918) = 0.223 per second, which is **804 glances per
  hour** of driving.
- **Eyes-off share:** 0.918/4.478 = **20.5 %** of driving time.
- **Long glances:** off-path glances over 2.0 s are 1 − Φ((ln 2 + 0.21)/0.50) = **3.5 %**
  of all glances, which is 28 per hour.

## Sources (read 2026-10-28)

**Morando, Victor & Dozza 2019**, "A Reference Model for Driver Attention in Automation:
Glance Behavior Changes During Lateral and Longitudinal Assistance", *IEEE T-ITS* 20(8),
2999-3009, doi 10.1109/TITS.2018.2870909. The accepted version was read from
research.chalmers.se, publication 505290.
- **Data:** naturalistic eye-tracking from a field operational test, manual driving. The
  aggregate distribution weights each driver equally (§II-D1, p. 3).
- **Filtering:** off-path glances shorter than 0.3 s are treated as blinks and removed (ISO
  15007-1:2014), and glances shorter than 0.15 s are interpolated (§II-C, p. 3).
- **The fit:** maximum likelihood, an inverse normal on path (eq. 1) and a lognormal off
  path (eq. 2), with χ²(497) p = 0.999 for both (§III-A2, pp. 5-6). The parameters are
  printed in Fig. 6 (p. 6).
- **Agreement:** the manual-driving distributions agree with the baselines of the 100-Car
  and SHRP 2 studies (Fig. 2, p. 4).
- **Independent draws:** the correlation between an on-path glance and the off-path glance
  after it is only ρ = −0.12 (p. 5).

**Klauer, Dingus, Neale, Sudweeks & Ramsey 2006**, *The Impact of Driver Inattention on
Near-Crash/Crash Risk*, DOT HS 810 594. It was read from the VTechWorks copy, because nhtsa.gov
refuses scripted access. Its baseline is 20,000 random 6 s epochs, with eye glances reduced
on 5,000 of them (executive summary, PDF p. 6).
- **Table 6.2 (PDF p. 132, report p. 102):** odds ratios by total eyes-off-road time in the
  5 s before and 1 s after the precipitating event:

  | Eyes off road | Odds ratio (95 % CI) |
  |---|---|
  | ≤ 0.5 s | 1.31 |
  | 0.5-1.0 s | 0.82 |
  | 1.0-1.5 s | 0.92 |
  | 1.5-2.0 s | 1.26 |
  | > 2.0 s | **2.19 (1.72-2.78)** |
  | any | 1.32 |

  Only the > 2 s bin, and any, differ from 1.
- **Figs. 6.1-6.3 (report pp. 105-107), read off the bar charts:**
  - baseline epochs have about **0.9 s** of eyes-off time per 6 s (15 %);
  - about **1.1** glances away per 6 s;
  - a longest glance of about **0.75 s**.

  So the 100-Car baseline sits close to the fit above, which has 20.5 % eyes-off time and
  1.34 glance onsets per 6 s.

**Dingus et al. 2016**, "Driver crash risk factors and prevalence evaluation using
naturalistic driving data", *PNAS* 113(10), 2636-2641 (PMC4790996). SHRP 2, 905
injurious and property-damage crashes, 19,732 baselines.
- **Prevalence:** a potentially distracting activity in **51.93 %** of baselines.
- **Overall odds ratio:** 2.0 for distraction.
- **Eyes-off-road activities:**

  | Activity | Odds ratio |
  |---|---|
  | handheld dialling | 12.2 |
  | reading or writing | 9.9 |
  | reaching for an object | 9.1 |
  | extended glance at an external object | 7.1 |
  | texting | 6.1 |
  | handheld phone overall (prevalence 6.40 %) | 3.6 |

  These are activity-level figures. They are validation targets for a later
  activity-level model, not inputs to the glance process.

**NHTSA Visual-Manual Driver Distraction Guidelines**, 78 FR 24818 (26 April 2013), read
from govinfo.gov.
- **Recommendation:** glances away of 2 s or less, and 12 s or less in total (p. 24821).
- **Acceptance criteria (p. 24888):** for 21 of 24 participants, no more than 15 % of
  glances over 2.0 s, a mean glance of at most 2.0 s, and at most 12.0 s of total
  eyes-off-road time.
- **Use here:** these are limits for a device task, not observed driving. The fit's 3.5 %
  of glances over 2 s lies inside them, as normal driving should.

## The warning hook (V2X)

The plan asks that a received, verified FCW or EEBL warning ends a glance "after a warning
reaction time". **No gaze-return latency was read.** The candidates could not be read:
- Lee, McGehee, Brown & Reyes 2002, *Human Factors* 44(2) (paywalled);
- NHTSA DOT HS 811 501 (nhtsa.gov refuses scripted access);
- a Volvo truck FOT study behind a bot wall.

So the warning ends the glance at the step it is received. The driver's response then goes
through their reaction time (`reaction_time_s`, HDM T′, 0.8 s, Green 2000) and the alarm
rule. **No new constant is added.** A cited gaze-return latency can be inserted at that
point later.

## Choices (recorded)

- **The glance renewal process, not "episodes" with an hourly rate.** The plan proposed an
  onset rate and a duration distribution. The naturalistic fit is a pair of dwell
  distributions, so the onset rate (804 per hour) follows from them rather than being a
  setting.
- **Every off-path glance freezes the forward picture,** mirror checks included. Klauer's
  eyes-off-road measure counts them too. Peripheral vision of a looming leader is not
  modelled, which is a known simplification that errs towards later responses.
- **Settings path:** `actors.vehicles.drivers.distraction`. The plan wrote
  `traffic.drivers`, but the scenario's drivers live under `actors.vehicles.drivers`.
- **Keyed draws:** each dwell is drawn at the step it starts, keyed by (driver key, step):
  `std_normal(key, step, 2)` and a SplitMix64 uniform on slot 3. Perception uses slots 0
  and 1. A driver starts on path with a fresh on-path dwell; the stationary residual is
  not drawn (a transient of about 4 s).
