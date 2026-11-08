//! Model selection by id: the one place a new model is added (see [`crate::traits`]'s
//! "how to add another model").
//!
//! A scenario names a model by its card id (`energy/emission/hbefa3`) or by the id's last
//! segment (`hbefa3`). An unknown name is an error that lists every name this build knows.

use std::collections::BTreeMap;

use crate::ev::{KURCZVEIL_ID, Kurczveil, VT_CPEM_ID, VtCpem};
use crate::traits::{EmissionModel, EvEnergyModel};

/// A boxed emission model, as the provider holds one.
pub type BoxedEmission = Box<dyn EmissionModel + Send + Sync>;
/// A boxed EV energy model, as the provider holds one.
pub type BoxedEv = Box<dyn EvEnergyModel + Send + Sync>;

/// Every emission model id, in id order.
pub const EMISSION_MODELS: [&str; 3] = [
    crate::hbefa3::MODEL_ID,
    crate::moves::MODEL_ID,
    crate::phemlight::MODEL_ID,
];

/// Every EV energy model id, in id order.
pub const EV_MODELS: [&str; 2] = [KURCZVEIL_ID, VT_CPEM_ID];

/// The full id a name refers to among `ids`: itself, or the id whose last segment it is.
fn canonical<'a>(name: &str, ids: &[&'a str]) -> Option<&'a str> {
    ids.iter()
        .copied()
        .find(|id| *id == name || id.rsplit('/').next() == Some(name))
}

/// The emission model `name` selects.
///
/// # Errors
/// An unknown name, with the list of known ones.
pub fn emission_model(name: &str) -> Result<BoxedEmission, String> {
    match canonical(name, &EMISSION_MODELS) {
        Some(crate::hbefa3::MODEL_ID) => Ok(Box::new(crate::hbefa3::Hbefa3::new())),
        Some(crate::phemlight::MODEL_ID) => Ok(Box::new(crate::phemlight::PhemLight::new())),
        Some(crate::moves::MODEL_ID) => Ok(Box::new(crate::moves::MovesOpMode::new())),
        _ => Err(format!(
            "no emission model {name:?}; this build has {EMISSION_MODELS:?}"
        )),
    }
}

/// The EV energy model `name` selects, with `params` overriding its card's defaults.
///
/// # Errors
/// An unknown name, or an override the model's card does not declare.
pub fn ev_model(name: &str, params: &BTreeMap<String, f64>) -> Result<BoxedEv, String> {
    match canonical(name, &EV_MODELS) {
        Some(KURCZVEIL_ID) => Ok(Box::new(Kurczveil::new(params)?)),
        Some(VT_CPEM_ID) => Ok(Box::new(VtCpem::new(params)?)),
        _ => Err(format!(
            "no EV energy model {name:?}; this build has {EV_MODELS:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_id_constructs_under_its_full_and_short_name() {
        for id in EMISSION_MODELS {
            assert_eq!(emission_model(id).unwrap().card().id, id);
            let short = id.rsplit('/').next().unwrap();
            assert_eq!(emission_model(short).unwrap().card().id, id);
        }
        for id in EV_MODELS {
            assert_eq!(ev_model(id, &BTreeMap::new()).unwrap().card().id, id);
        }
        assert!(emission_model("copert").is_err());
    }
}
