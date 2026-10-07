//! The driving-model library: every car-following model by name, a per-class composite,
//! and the scenario block that selects them.
//!
//! # Selecting models in a scenario
//!
//! ```yaml
//! actors:
//!   vehicles:
//!     driving:
//!       car_following: { model: krauss }            # every class, unless by_class says otherwise
//!       by_class:
//!         truck:      { model: gipps }
//!         passenger:  { model: cacc-ploeg, share: 0.4 }   # 40 % of cars cooperative
//!       cooperative: { max_age_s: 0.3, association_gate_m: 5.0 }
//! ```
//!
//! Leaving `driving` out keeps the engine's own default (the IDM with the
//! [`super::idm::IdmPreset::UrbanHcm`] drivers) and changes nothing about an existing run.
//!
//! # The registry
//!
//! [`FACTORIES`] is the one list of selectable models. Each entry names the model's id, a
//! short name, a constructor from the scenario's `params` and a card. **Adding a model is
//! one module and one line here** — `docs/models/driving-model-library.md` walks through
//! it.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier};
use v2xw_core::ids::ActorId;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use super::acc::{self, Acc, AccParams};
use super::cacc::{self, Cacc, CaccLaw, CaccParams};
use super::common;
use super::eidm::{self, Eidm, EidmParams};
use super::gipps::{self, Gipps, GippsParams};
use super::idm::{self, Idm, IdmParams, IdmPreset};
use super::krauss::{self, Krauss, KraussParams};
use super::w99::{self, W99, W99Params};
use crate::classes::VehicleClass;
use crate::coop::{CoopLink, CoopParams};
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};

/// A car-following model as the engine holds it.
pub type SharedCf = Arc<dyn CarFollowing + Send + Sync>;

/// One model choice: which model, its own parameters, and the share of the class that
/// drives it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CfChoice {
    /// The model, by short name (`krauss`) or id (`mobility/car-following/krauss`).
    pub model: String,
    /// The model's own parameter struct, as JSON; `null` for the cited defaults.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
    /// The share of the class's vehicles that drive this model, `0..=1`; the rest drive the
    /// default model. Drawn once per vehicle from its own keyed stream.
    #[serde(default = "CfChoice::full", skip_serializing_if = "CfChoice::is_full")]
    pub share: f64,
}

impl CfChoice {
    fn full() -> f64 {
        1.0
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn is_full(s: &f64) -> bool {
        *s >= 1.0
    }

    /// A choice of `model` with its defaults, for every vehicle.
    pub fn of(model: &str) -> Self {
        Self {
            model: model.to_string(),
            params: serde_json::Value::Null,
            share: 1.0,
        }
    }
}

/// The scenario's `actors.vehicles.driving` block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DrivingSpec {
    /// The model every class drives unless [`DrivingSpec::by_class`] says otherwise;
    /// `None` keeps the engine's default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub car_following: Option<CfChoice>,
    /// Per-class choices.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub by_class: BTreeMap<VehicleClass, CfChoice>,
    /// How cooperative models associate and age messages.
    pub cooperative: CoopParams,
    /// The lateral (sub-lane) model, when one is selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lateral: Option<crate::lateral::LateralSpec>,
}

impl DrivingSpec {
    /// Whether this block changes nothing (the scenario's content hash leaves it out).
    pub fn is_default(&self) -> bool {
        *self == DrivingSpec::default()
    }
}

/// What a factory may need beyond the model's own parameters.
#[derive(Debug, Clone)]
pub struct BuildCtx {
    /// The run's master seed, for models with keyed per-step noise.
    pub seed: u64,
    /// The one cooperative link every cooperative model of the run shares.
    pub link: Arc<CoopLink>,
}

/// One selectable model.
#[derive(Clone, Copy)]
pub struct CfFactory {
    /// The model id.
    pub id: &'static str,
    /// The short name a scenario may use.
    pub short: &'static str,
    /// Whether it reads V2X (subscribes to the cooperative link).
    pub cooperative: bool,
    /// Builds the model from its parameters.
    pub build: fn(&serde_json::Value, &BuildCtx) -> Result<SharedCf, String>,
    /// The card at the cited defaults.
    pub card: fn() -> ModelCard,
}

impl core::fmt::Debug for CfFactory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CfFactory").field("id", &self.id).finish()
    }
}

fn parse<T: serde::de::DeserializeOwned + Default>(v: &serde_json::Value) -> Result<T, String> {
    if v.is_null() {
        Ok(T::default())
    } else {
        serde_json::from_value(v.clone()).map_err(|e| e.to_string())
    }
}

/// IDM parameters as a scenario gives them: a preset plus optional model-wide overrides.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct IdmChoice {
    preset: Option<IdmPreset>,
    params: Option<IdmParams>,
}

fn cacc_build(law: CaccLaw, v: &serde_json::Value, ctx: &BuildCtx) -> Result<SharedCf, String> {
    let params = if v.is_null() {
        CaccParams::for_law(law)
    } else {
        let mut base = serde_json::to_value(CaccParams::for_law(law)).map_err(|e| e.to_string())?;
        if let (Some(b), Some(o)) = (base.as_object_mut(), v.as_object()) {
            for (k, val) in o {
                b.insert(k.clone(), val.clone());
            }
        }
        serde_json::from_value(base).map_err(|e| e.to_string())?
    };
    Ok(Arc::new(Cacc::with_params(law, params, Arc::clone(&ctx.link))))
}

/// Every selectable car-following model, in id order. Add a model here.
pub const FACTORIES: &[CfFactory] = &[
    CfFactory {
        id: acc::MODEL_ID,
        short: "acc",
        cooperative: false,
        build: |v, _| Ok(Arc::new(Acc::new(parse::<AccParams>(v)?))),
        card: || acc::card(&AccParams::default()),
    },
    CfFactory {
        id: "mobility/car-following/cacc-milanes",
        short: "cacc-milanes",
        cooperative: true,
        build: |v, c| cacc_build(CaccLaw::Milanes, v, c),
        card: || {
            cacc::card(
                CaccLaw::Milanes,
                &CaccParams::for_law(CaccLaw::Milanes),
                &CoopParams::default(),
            )
        },
    },
    CfFactory {
        id: "mobility/car-following/cacc-path",
        short: "cacc-path",
        cooperative: true,
        build: |v, c| cacc_build(CaccLaw::Path, v, c),
        card: || {
            cacc::card(
                CaccLaw::Path,
                &CaccParams::for_law(CaccLaw::Path),
                &CoopParams::default(),
            )
        },
    },
    CfFactory {
        id: "mobility/car-following/cacc-ploeg",
        short: "cacc-ploeg",
        cooperative: true,
        build: |v, c| cacc_build(CaccLaw::Ploeg, v, c),
        card: || {
            cacc::card(
                CaccLaw::Ploeg,
                &CaccParams::for_law(CaccLaw::Ploeg),
                &CoopParams::default(),
            )
        },
    },
    CfFactory {
        id: eidm::MODEL_ID,
        short: "eidm",
        cooperative: false,
        build: |v, _| Ok(Arc::new(Eidm::new(parse::<EidmParams>(v)?))),
        card: || eidm::card(&EidmParams::default()),
    },
    CfFactory {
        id: gipps::MODEL_ID,
        short: "gipps",
        cooperative: false,
        build: |v, _| Ok(Arc::new(Gipps::new(parse::<GippsParams>(v)?))),
        card: || gipps::card(&GippsParams::default()),
    },
    CfFactory {
        id: idm::MODEL_ID,
        short: "idm",
        cooperative: false,
        build: |v, _| {
            let c: IdmChoice = parse(v)?;
            let preset = c.preset.unwrap_or(IdmPreset::UrbanHcm);
            Ok(Arc::new(match c.params {
                Some(p) => Idm::with_params(preset, p),
                None => Idm::new(preset),
            }))
        },
        card: || idm::card(IdmPreset::UrbanHcm, &IdmPreset::UrbanHcm.params()),
    },
    CfFactory {
        id: krauss::MODEL_ID,
        short: "krauss",
        cooperative: false,
        build: |v, c| Ok(Arc::new(Krauss::new(parse::<KraussParams>(v)?, c.seed))),
        card: || krauss::card(&KraussParams::default()),
    },
    CfFactory {
        id: w99::MODEL_ID,
        short: "w99",
        cooperative: false,
        build: |v, c| Ok(Arc::new(W99::new(parse::<W99Params>(v)?, c.seed))),
        card: || w99::card(&W99Params::default()),
    },
];

/// The factory a scenario names, by short name or id.
pub fn factory(name: &str) -> Option<&'static CfFactory> {
    FACTORIES.iter().find(|f| f.short == name || f.id == name)
}

/// The id of the per-class composite.
pub const BY_CLASS_ID: &str = "mobility/car-following/by-class";

/// Dispatches to a model per vehicle class (and, with a share below one, per vehicle).
pub struct ByClass {
    default: SharedCf,
    per_class: BTreeMap<VehicleClass, (f64, SharedCf)>,
    seed: u64,
    link: Option<Arc<CoopLink>>,
    card: ModelCard,
}

impl core::fmt::Debug for ByClass {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ByClass")
            .field("default", &self.default.card().id)
            .field(
                "per_class",
                &self
                    .per_class
                    .iter()
                    .map(|(c, (s, m))| (c.as_str(), *s, m.card().id.clone()))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ByClass {
    /// The model `actor` of `class` drives.
    pub fn model_for(&self, class: VehicleClass, actor: ActorId) -> &SharedCf {
        match self.per_class.get(&class) {
            Some((share, m)) if *share >= 1.0 => m,
            Some((share, m))
                if common::keyed_uniform(self.seed, BY_CLASS_ID, actor, 0, 0) < *share =>
            {
                m
            }
            _ => &self.default,
        }
    }
}

impl v2xw_core::model::Model for ByClass {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl CarFollowing for ByClass {
    fn accel(
        &self,
        ego: &VehicleView,
        leader: Option<&LeaderView>,
        lane: &LaneView,
        w: &WeatherState,
    ) -> f64 {
        self.model_for(ego.class, ego.actor).accel(ego, leader, lane, w)
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        // A class split between two models draws the default's driver: the profile is
        // drawn per class at spawn, before the vehicle's share draw is known to the
        // engine. ACC and CACC hold their own time gap, so a cooperative vehicle with the
        // default's driver still regulates to its controller's gap.
        match self.per_class.get(&class) {
            Some((share, m)) if *share >= 1.0 => m.profile(class),
            _ => self.default.profile(class),
        }
    }

    fn begin_step(&self, now: SimTime, dt: Duration) {
        self.default.begin_step(now, dt);
        for (_, m) in self.per_class.values() {
            m.begin_step(now, dt);
        }
    }

    fn cooperative(&self) -> Option<Arc<CoopLink>> {
        self.link.clone()
    }
}

/// Builds the car-following model a [`DrivingSpec`] selects, or `None` when it selects
/// none (the engine keeps its default).
///
/// # Errors
///
/// A model name no factory knows, parameters that do not fit, or a share outside `0..=1`,
/// each naming the scenario key.
pub fn build(spec: &DrivingSpec, seed: u64) -> Result<Option<SharedCf>, String> {
    if spec.car_following.is_none() && spec.by_class.is_empty() {
        return Ok(None);
    }
    let mut subscribers = Vec::new();
    let cooperative = |c: &CfChoice| factory(&c.model).is_some_and(|f| f.cooperative);
    let default_coop = spec.car_following.as_ref().is_some_and(cooperative);
    for class in VehicleClass::ALL {
        let coop = match spec.by_class.get(&class) {
            Some(c) => cooperative(c) || (c.share < 1.0 && default_coop),
            None => default_coop,
        };
        if coop {
            subscribers.push(class);
        }
    }
    let link = Arc::new(CoopLink::new(spec.cooperative, subscribers));
    let ctx = BuildCtx {
        seed,
        link: Arc::clone(&link),
    };
    let make = |key: &str, c: &CfChoice| -> Result<SharedCf, String> {
        let f = factory(&c.model).ok_or_else(|| {
            format!(
                "{key}.model: unknown car-following model `{}` (known: {})",
                c.model,
                FACTORIES.iter().map(|f| f.short).collect::<Vec<_>>().join(", ")
            )
        })?;
        if !(0.0..=1.0).contains(&c.share) {
            return Err(format!("{key}.share: {} is outside 0..=1", c.share));
        }
        (f.build)(&c.params, &ctx).map_err(|e| format!("{key}.params: {e}"))
    };
    let default: SharedCf = match &spec.car_following {
        Some(c) => make("actors.vehicles.driving.car_following", c)?,
        None => Arc::new(Idm::new(IdmPreset::UrbanHcm)),
    };
    let mut per_class = BTreeMap::new();
    for (class, c) in &spec.by_class {
        let key = format!("actors.vehicles.driving.by_class.{}", class.as_str());
        per_class.insert(*class, (c.share, make(&key, c)?));
    }
    let any_coop = !link.subscribers().is_empty();
    let card = by_class_card(&default, &per_class);
    Ok(Some(Arc::new(ByClass {
        default,
        per_class,
        seed,
        link: any_coop.then_some(link),
        card,
    })))
}

fn by_class_card(default: &SharedCf, per_class: &BTreeMap<VehicleClass, (f64, SharedCf)>) -> ModelCard {
    let mut card = ModelCard::new(
        BY_CLASS_ID,
        Family::Mobility,
        "1.0.0",
        "Dispatches each vehicle to the car-following model its class (and, with a share, its \
         own keyed draw) selects. Holds no equation of its own.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![Equation::new(
        "dispatch",
        "model(v) = by_class[class(v)] if U(seed, v) < share else default",
    )];
    let choice = Source::new(
        SourceKind::Code,
        "the scenario's actors.vehicles.driving block (crate::carfollowing::library)",
    );
    card.parameters = vec![
        Parameter::new(
            "default",
            "-",
            serde_json::json!(default.card().id),
            choice.clone(),
        ),
        Parameter::new(
            "by_class",
            "-",
            serde_json::json!(per_class
                .iter()
                .map(|(c, (s, m))| (c.as_str().to_string(), serde_json::json!({"model": m.card().id, "share": s})))
                .collect::<serde_json::Map<_, _>>()),
            choice,
        ),
    ];
    card.determinism = v2xw_core::card::Determinism {
        uses_rng: per_class.values().any(|(s, _)| *s < 1.0),
        rng_domains: if per_class.values().any(|(s, _)| *s < 1.0) {
            vec!["plugin".to_string()]
        } else {
            Vec::new()
        },
    };
    card
}

/// Every library card at its cited defaults (the IDM's is registered by the crate already
/// and is left out here).
pub fn cards() -> Vec<ModelCard> {
    FACTORIES
        .iter()
        .filter(|f| f.id != idm::MODEL_ID)
        .map(|f| (f.card)())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_factory_builds_from_null_and_its_card_validates() {
        let link = Arc::new(CoopLink::new(CoopParams::default(), vec![]));
        let ctx = BuildCtx { seed: 1, link };
        let mut ids: Vec<&str> = FACTORIES.iter().map(|f| f.id).collect();
        let sorted = {
            let mut s = ids.clone();
            s.sort_unstable();
            s
        };
        assert_eq!(ids, sorted, "FACTORIES is kept in id order");
        ids.dedup();
        assert_eq!(ids.len(), FACTORIES.len());
        for f in FACTORIES {
            let m = (f.build)(&serde_json::Value::Null, &ctx).unwrap();
            assert_eq!(m.card().id, f.id);
            (f.card)().validate().unwrap_or_else(|e| panic!("{}: {e}", f.id));
            assert_eq!(m.cooperative().is_some(), f.cooperative, "{}", f.id);
        }
    }

    #[test]
    fn an_empty_spec_selects_nothing_and_a_bad_name_is_refused() {
        assert!(build(&DrivingSpec::default(), 1).unwrap().is_none());
        let spec = DrivingSpec {
            car_following: Some(CfChoice::of("no-such-model")),
            ..DrivingSpec::default()
        };
        let Err(err) = build(&spec, 1) else {
            panic!("an unknown model was accepted")
        };
        assert!(err.contains("no-such-model") && err.contains("krauss"), "{err}");
    }

    #[test]
    fn by_class_dispatches_and_subscribes_only_cooperative_classes() {
        let yaml = r#"{"car_following": {"model": "krauss"},
                       "by_class": {"truck": {"model": "gipps"},
                                    "passenger": {"model": "cacc-ploeg", "share": 0.5}}}"#;
        let spec: DrivingSpec = serde_json::from_str(yaml).unwrap();
        let m = build(&spec, 9).unwrap().unwrap();
        let link = m.cooperative().expect("a cooperative class");
        assert!(link.subscribes(VehicleClass::Passenger));
        assert!(!link.subscribes(VehicleClass::Truck));
        assert_eq!(m.profile(VehicleClass::Truck), Gipps::class_profile(VehicleClass::Truck));
        // Half the cars: over 200 ids the split is near one half, and stable.
        let bc = by_class_of(&spec);
        let n = (0..200u32)
            .filter(|i| {
                bc.model_for(VehicleClass::Passenger, ActorId::new(*i)).card().id
                    == "mobility/car-following/cacc-ploeg"
            })
            .count();
        assert!((70..130).contains(&n), "{n}");
        let again = by_class_of(&spec);
        for i in 0..200u32 {
            assert_eq!(
                bc.model_for(VehicleClass::Passenger, ActorId::new(i)).card().id,
                again.model_for(VehicleClass::Passenger, ActorId::new(i)).card().id
            );
        }
    }

    fn by_class_of(spec: &DrivingSpec) -> ByClass {
        let link = Arc::new(CoopLink::new(spec.cooperative, vec![VehicleClass::Passenger]));
        let ctx = BuildCtx { seed: 9, link };
        let mk = |c: &CfChoice| (factory(&c.model).unwrap().build)(&c.params, &ctx).unwrap();
        let default = mk(spec.car_following.as_ref().unwrap());
        let per_class = spec
            .by_class
            .iter()
            .map(|(k, c)| (*k, (c.share, mk(c))))
            .collect::<BTreeMap<_, _>>();
        let card = by_class_card(&default, &per_class);
        ByClass {
            default,
            per_class,
            seed: 9,
            link: None,
            card,
        }
    }
}
