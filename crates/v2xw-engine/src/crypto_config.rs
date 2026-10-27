//! `security.kem`, `security.threshold` and `actors.backend.failures`: from the scenario to
//! the credential system's parameters.
//!
//! Every value is looked up in a registry — `v2xw_sec::scheme` for schemes,
//! `v2xw_proto::threshold::THRESHOLD_PROTOCOLS` for protocols — so a scheme or protocol
//! registered there is accepted here without an edit. The loader runs [`check`] and refuses
//! a scenario whose values these functions would refuse at run time.

use v2xw_core::ids::NodeId;
use v2xw_core::time::Duration;
use v2xw_proto::fault::{Fault, FaultKind};
use v2xw_proto::scms::params::ThresholdDeployment;
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::threshold::{ProtocolParams, THRESHOLD_IDS};

use crate::error::EngineError;
use crate::scenario::schema::{Scenario, ThresholdSpec};

/// The roles a committee can play.
pub const THRESHOLD_ROLES: [&str; 3] = ["pca", "crl", "linkage"];
/// The fault modes `actors.backend.failures[].mode` accepts.
pub const FAILURE_MODES: [&str; 3] = ["offline", "slow", "malicious"];
/// The most members a committee may have (`v2xw_proto::scms::run::COMMITTEE_STRIDE`).
pub const MAX_MEMBERS: u16 = 64;

fn conflict(field: &str, why: impl Into<String>) -> EngineError {
    EngineError::Scenario(crate::ScenarioError::conflict(field, why))
}

/// The post-quantum KEM `security.kem` names; `None` for ECIES.
#[must_use]
pub fn kem_spec(s: &Scenario) -> Option<v2xw_proto::KemSpec> {
    v2xw_proto::KemSpec::from_kem(&s.security.kem)
}

fn registered(id: &str) -> Option<&'static str> {
    THRESHOLD_IDS.iter().copied().find(|p| *p == id)
}

fn ms(v: f64) -> Duration {
    Duration::from_nanos((v * 1e6).round().max(0.0) as u64)
}

/// The committees `security.threshold` asks for.
///
/// # Errors
/// A conflict naming the field: an unregistered protocol, a quorum larger than its
/// committee, an unknown role, a non-positive timeout.
pub fn threshold_deployment(s: &Scenario) -> Result<Option<ThresholdDeployment>, EngineError> {
    let Some(t) = s.security.threshold.as_ref() else {
        return Ok(None);
    };
    check_spec(t)?;
    let protocol = registered(&t.protocol).expect("checked");
    let inner = registered(&t.umbrella_inner).expect("checked");
    let has = |r: &str| t.roles.iter().any(|x| x == r);
    Ok(Some(ThresholdDeployment {
        protocol,
        protocol_params: ProtocolParams {
            umbrella_leaves: t.umbrella_leaves.max(1),
            umbrella_inner: inner,
            preprocessed: t.preprocessed,
        },
        members: t.members,
        quorum: t.quorum,
        pca: has("pca"),
        crl: has("crl"),
        linkage: has("linkage").then(|| {
            (
                t.linkage_members.unwrap_or(t.members),
                t.linkage_quorum.unwrap_or(t.quorum),
            )
        }),
        round_timeout: ms(t.round_timeout_ms),
        retry_after: ms(t.retry_after_s * 1000.0),
        max_attempts: ThresholdDeployment::default().max_attempts,
        fanout_all: t.ask_all,
        member_link_latency: ms(t.member_latency_ms),
    }))
}

fn check_spec(t: &ThresholdSpec) -> Result<(), EngineError> {
    let ids = THRESHOLD_IDS.join(", ");
    if registered(&t.protocol).is_none() {
        return Err(conflict(
            "security.threshold.protocol",
            format!(
                "'{}' is not a registered threshold protocol; allowed: {ids}",
                t.protocol
            ),
        ));
    }
    if registered(&t.umbrella_inner).is_none() || t.umbrella_inner == "umbrella" {
        return Err(conflict(
            "security.threshold.umbrella_inner",
            format!(
                "'{}' cannot sign umbrellas: name a registered signing protocol other than \
                 'umbrella' ({ids})",
                t.umbrella_inner
            ),
        ));
    }
    let committee = |field: &str, members: u16, quorum: u16| {
        if members == 0 || members > MAX_MEMBERS {
            return Err(conflict(
                field,
                format!("a committee has 1 to {MAX_MEMBERS} members, not {members}"),
            ));
        }
        if quorum == 0 || quorum > members {
            return Err(conflict(
                field,
                format!(
                    "a quorum of {quorum} out of {members} members cannot be met: it must be \
                     between 1 and the committee size"
                ),
            ));
        }
        Ok(())
    };
    committee("security.threshold.quorum", t.members, t.quorum)?;
    committee(
        "security.threshold.linkage_quorum",
        t.linkage_members.unwrap_or(t.members),
        t.linkage_quorum.unwrap_or(t.quorum),
    )?;
    for r in &t.roles {
        if !THRESHOLD_ROLES.contains(&r.as_str()) {
            return Err(conflict(
                "security.threshold.roles",
                format!(
                    "'{r}' is not a role a committee can play; allowed: {}",
                    THRESHOLD_ROLES.join(", ")
                ),
            ));
        }
    }
    if t.round_timeout_ms.is_nan() || t.round_timeout_ms <= 0.0 {
        return Err(conflict(
            "security.threshold.round_timeout_ms",
            "must be positive: a coordinator with no timeout waits forever for a member \
             that is down",
        ));
    }
    if t.retry_after_s.is_nan() || t.retry_after_s <= 0.0 {
        return Err(conflict(
            "security.threshold.retry_after_s",
            "must be positive",
        ));
    }
    if t.member_latency_ms.is_nan() || t.member_latency_ms < 0.0 {
        return Err(conflict(
            "security.threshold.member_latency_ms",
            "must not be negative",
        ));
    }
    if t.umbrella_leaves == 0 {
        return Err(conflict(
            "security.threshold.umbrella_leaves",
            "an umbrella covers at least one leaf",
        ));
    }
    Ok(())
}

/// Splits `pca-member-2` into (`pca`, 2).
fn member_name(name: &str) -> Option<(&str, u16)> {
    let (role, k) = name.split_once("-member-")?;
    Some((role, k.parse().ok()?))
}

/// The backend node a failure names among the home provider's components, in the run
/// `run` hosts (the role or committee member, unqualified).
#[must_use]
pub fn failure_node(name: &str, run: &ScmsRun) -> Option<NodeId> {
    role_node(name, &run.state.nodes, run)
}

/// `role` (or `<role>-member-<k>`) among the components `n` names. Committee member ids
/// are the same block in every provider's run, so any provider's `run` resolves them.
fn role_node(
    name: &str,
    n: &v2xw_proto::scms::params::ScmsNodes,
    run: &ScmsRun,
) -> Option<NodeId> {
    if let Some((role, k)) = member_name(name) {
        return run.committee_members(role).get(usize::from(k)).copied();
    }
    Some(match name {
        "ra" => n.ra,
        "pca" => n.pca,
        "la1" => n.la1,
        "la2" => n.la2,
        "ma" => n.ma,
        "crlg" => n.crlg,
        "lop" => n.lop,
        "crl_store" | "crl-store" => n.crl_store,
        "crl_broadcast" | "crl-broadcast" => n.crl_broadcast,
        "eca" => n.eca,
        "dcm" => n.dcm,
        _ => return None,
    })
}

/// Why a failure's node does not resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureNodeError {
    /// `<provider>/...` names no provider of the deployment; the providers it has.
    Provider(String),
    /// The role or member is unknown.
    Role,
}

/// The provider index and node a failure names, in the deployment `home` (provider 0's
/// run) hosts.
///
/// An unqualified name (`crlg`) is the home provider's component, as before. A
/// provider-qualified one (`provider-b/crlg`, `provider-b/pca-member-1`) names that
/// provider's: in a multi-provider deployment each provider runs its own CRL Generator,
/// CRL Store, MA, RA and ACA (WP-D1), and an outage of one provider's server delays only
/// what that server carries.
///
/// # Errors
/// [`FailureNodeError`] when the provider or role is unknown.
pub fn failure_target(name: &str, home: &ScmsRun) -> Result<(usize, NodeId), FailureNodeError> {
    let Some((provider, role)) = name.split_once('/') else {
        return failure_node(name, home)
            .map(|n| (0, n))
            .ok_or(FailureNodeError::Role);
    };
    let d = &home.state.deployment;
    let k = d
        .providers
        .iter()
        .position(|p| p.name == provider)
        .ok_or_else(|| {
            FailureNodeError::Provider(
                d.providers
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        })?;
    let nodes = if k == 0 {
        home.state.nodes
    } else {
        d.nodes(k).online
    };
    role_node(role, &nodes, home)
        .map(|n| (k, n))
        .ok_or(FailureNodeError::Role)
}

/// The faults `actors.backend.failures` declares, resolved against the deployment `home`
/// (provider 0's run) hosts, each with the index of the provider whose run hosts its node
/// (the run that delivers to it, and so the one whose fault plan must hold it).
///
/// # Errors
/// A conflict naming the failure: an unknown provider or server, an unknown mode, a slow
/// server with no extra time.
pub fn backend_faults(s: &Scenario, home: &ScmsRun) -> Result<Vec<(usize, Fault)>, EngineError> {
    let mut out = Vec::new();
    for (i, f) in s.actors.backend.failures.iter().enumerate() {
        let field = format!("actors.backend.failures[{i}]");
        let (provider, node) = failure_target(&f.node, home).map_err(|e| {
            conflict(
                &format!("{field}.node"),
                match e {
                    FailureNodeError::Provider(names) => format!(
                        "'{}' names no provider of this deployment; providers: {names}",
                        f.node
                    ),
                    FailureNodeError::Role => format!(
                        "'{}' is neither a backend role (ra, pca, la1, la2, ma, crlg, lop, \
                         crl_store, crl_broadcast, eca, dcm) nor a member of a configured \
                         committee (<role>-member-<k>, k below security.threshold's size), \
                         optionally qualified by its provider (<provider>/<role>)",
                        f.node
                    ),
                },
            )
        })?;
        let kind = match f.mode.as_str() {
            "offline" => FaultKind::Offline,
            "malicious" => FaultKind::Malicious,
            "slow" => {
                let extra = f.extra_ms.filter(|x| *x > 0.0).ok_or_else(|| {
                    conflict(
                        &format!("{field}.extra_ms"),
                        "a slow server needs extra_ms > 0",
                    )
                })?;
                FaultKind::Slow { extra: ms(extra) }
            }
            other => {
                return Err(conflict(
                    &format!("{field}.mode"),
                    format!(
                        "'{other}' is not a fault mode; allowed: {}",
                        FAILURE_MODES.join(", ")
                    ),
                ));
            }
        };
        let from = (f.from_s.max(0.0) * 1e9).round() as u64;
        let until = f
            .to_s
            .map_or(u64::MAX, |t| (t.max(0.0) * 1e9).round() as u64);
        out.push((
            provider,
            Fault {
                node,
                kind,
                from,
                until,
            },
        ));
    }
    Ok(out)
}

/// Everything above, for the loader: the scenario's crypto and failure settings resolve.
///
/// # Errors
/// The first conflict.
pub fn check(s: &Scenario) -> Result<(), EngineError> {
    let t = threshold_deployment(s)?;
    if s.security.threshold.is_some() {
        let etsi = s.actors.backend.protocol.as_deref() == Some(crate::phase2::ETSI_PKI)
            || s.security
                .protocol
                .as_ref()
                .is_some_and(|p| p.id == crate::phase2::ETSI_PKI);
        if etsi {
            return Err(conflict(
                "security.threshold",
                "committees are wired into the CAMP SCMS (PCA, CRL Generator, linkage \
                 authorities); the ETSI PKI does not run them yet",
            ));
        }
    }
    if s.actors.backend.failures.is_empty() {
        return Ok(());
    }
    let params = v2xw_proto::ScmsParams {
        threshold: t,
        ..v2xw_proto::ScmsParams::default()
    };
    // The scenario's own deployment, so a provider-qualified node resolves against the
    // providers it runs; a deployment that does not load is refused by its own check.
    let camp = s
        .actors
        .backend
        .protocol
        .clone()
        .or_else(|| s.security.protocol.as_ref().map(|c| c.id.clone()))
        .as_deref()
        == Some(crate::phase2::CAMP_SCMS);
    let run = match crate::phase2::deployment_of(s, camp) {
        Ok(d) => ScmsRun::new_deployed(params, 0, d),
        Err(_) => ScmsRun::new(params),
    }
    .map_err(|e| conflict("actors.backend", e.to_string()))?;
    backend_faults(s, &run).map(|_| ())
}
