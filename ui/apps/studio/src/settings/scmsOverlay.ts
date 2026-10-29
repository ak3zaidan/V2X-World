/**
 * Words, defaults and "when does this matter" for the SCMS and security settings, where the
 * engine's published row has none or has only its internal spelling ("Elector count (5).",
 * "`region`: latency from great-circle distance…").
 *
 * Only presentation lives here: a short label, one line of help, the default the engine applies
 * when the key is absent (each one quoted from the engine's own description of the key), the
 * setting it depends on, and whether a newcomer sees it on the Basic page. Which keys exist, their
 * choices, ranges and whether the engine acts on them stay the engine's.
 */

import type { RelevantWhen } from "../lib/schema.js";

export interface Overlay {
  readonly label?: string;
  readonly help?: string;
  readonly unit?: string;
  /** The value the engine uses when the key is absent, from its own description. */
  readonly defaultValue?: unknown;
  readonly relevantWhen?: RelevantWhen;
  readonly level?: "basic" | "advanced";
  /** A `[from, to]` list edited as two boxes. */
  readonly pair?: boolean;
  readonly min?: number;
  readonly max?: number;
  /** What an empty box means ("never"); on a menu, the words for "unset". */
  readonly emptyText?: string;
  /** What one element of a list is called on its Add button. */
  readonly noun?: string;
}

const strategy = "security.pseudonym_change.strategy";
const latency = "security.scms.deployment.latency";
const policy = "security.verification_policy";
const committee = { path: "security.threshold.protocol", in: [], notIn: [null] };

export const SCMS_OVERLAY: Readonly<Record<string, Overlay>> = {
  // Deployment.
  "/security/protocol/id": { label: "Credential system", help: "Who issues vehicles' certificates: the US SCMS or the European CCMS." },
  "/actors/backend/protocol": { label: "Backend credential system", help: "The credential system the backend servers run; keep it the same as above." },
  "/security/protocol/params": {
    label: "Older parameter block",
    help: "The lifecycle settings in their older spelling. A field in the form wins over the same key here.",
  },
  "/security/scms/deployment/preset": {
    label: "Providers",
    help: "Which companies run the SCMS. The US today: two providers on one trust list.",
    defaultValue: "us-production-2025",
    level: "basic",
  },
  "/security/scms/deployment/providers": { label: "Provider details", level: "basic" },
  "/security/scms/deployment/electors": { label: "Trust-list electors", help: "Electors who vote roots onto the trust list. US: 5.", defaultValue: 5, unit: "" },
  "/security/scms/deployment/quorum": { label: "Votes to sign the trust list", help: "Elector signatures a new trust list needs. US: 3.", defaultValue: 3, unit: "" },
  "/security/scms/deployment/options/lop": {
    label: "Location Obscurer Proxy (LOP)",
    help: "A proxy that hides a vehicle's address from the RA. Not in the US system.",
    defaultValue: false,
  },
  "/security/scms/deployment/options/butterfly": {
    label: "Butterfly key expansion",
    help: "How the RA turns one request into a week of certificates. The US uses the original.",
    defaultValue: "original",
  },
  "/security/scms/deployment/latency": { label: "Backend delay model", help: "How long a message takes between two SCMS servers.", defaultValue: "region" },
  "/security/scms/deployment/fibre_fraction": {
    label: "Signal speed in fibre",
    help: "As a fraction of the speed of light.",
    unit: "× c",
    defaultValue: 0.667,
    relevantWhen: { path: latency, in: ["region", null] },
  },
  "/security/scms/deployment/per_link_ms": {
    label: "Delay added per hop",
    help: "Fixed processing time on every backend hop.",
    defaultValue: 1,
    relevantWhen: { path: latency, in: ["region", null] },
  },
  "/security/scms/deployment/ma_report_sharing": {
    label: "Share reports between MAs",
    help: "Each MA forwards a report about another provider's certificate to that provider's MA.",
    defaultValue: true,
  },
  "/security/scms/chain_file": { label: "CA certificates each device holds", help: "Which providers' CA certificates a device stores.", defaultValue: "all-providers" },
  "/security/scms/pki_view": { label: "Record certificate traffic", help: "Feeds the PKI panels. Off keeps long recordings small.", defaultValue: true },
  "/security/scms/report_records": { label: "Record every report's fate", help: "Logs what happens to each misbehaviour report. Off keeps recordings small.", defaultValue: false },

  // Certificate lifecycle.
  "/security/pseudonym_change/period_s": { relevantWhen: { path: strategy, in: ["time", "j2945-1", "nyc-pilot", null] } },
  "/security/pseudonym_change/distance_m": {
    label: "Change pseudonym every (distance)",
    help: "How far a vehicle drives on one pseudonym.",
    relevantWhen: { path: strategy, in: ["distance", "j2945-1", "nyc-pilot"] },
  },
  "/security/pseudonym_change/mix_zone_radius_m": {
    label: "Mix-zone radius",
    help: "Size of the zone around a signalised junction where vehicles change pseudonym.",
    relevantWhen: { path: strategy, in: ["mix-zone"] },
  },
  "/security/pseudonym_change/mix_zone_min_age_s": {
    label: "Youngest pseudonym a mix zone changes",
    help: "A pseudonym younger than this is kept through the zone.",
    relevantWhen: { path: strategy, in: ["mix-zone"] },
  },
  "/security/pseudonym_change/silent_period_s": {
    label: "Silent after a change",
    help: "Sends no safety messages for a random time in this range after each change. Empty: none.",
    pair: true,
    min: 0,
  },
  "/security/signer_id_policy/full_cert_every_ms": {
    label: "Attach the full certificate every",
    help: "Other messages carry an 8-byte digest instead. US: once a second.",
  },
  "/security/signer_id_policy/digest_otherwise": { label: "Send a digest between full certificates", help: "Off sends nothing in their place." },
  "/security/signer_id_policy/on_new_neighbour": {
    label: "Full certificate for new neighbours",
    help: "Also attach it when a vehicle not heard before comes into range (the European rule).",
    defaultValue: false,
  },
  "/security/scms/lifecycle/root_rollover_at_s": { emptyText: "never" },
  "/security/scms/lifecycle/peer_root_removal_at_s": { emptyText: "never" },
  "/security/scms/lifecycle/crl_cadence_s": { emptyText: "each provider's rule" },
  "/security/scms/lifecycle/root_removal_at_s": {
    emptyText: "never",
    relevantWhen: { path: "security.scms.lifecycle.root_rollover_at_s", in: [], notIn: [null] },
  },
  "/security/scms/lifecycle/clock_skew_share": {
    relevantWhen: { path: "security.scms.lifecycle.clock_skew_s", in: [], notIn: [null, 0] },
  },

  // Receiver checks.
  "/security/on_demand_threshold": { defaultValue: 0.5, relevantWhen: { path: policy, in: ["on-demand"] } },
  "/security/prioritized_horizon_m": { defaultValue: 300, relevantWhen: { path: policy, in: ["prioritized"] } },

  // Threshold committees: absent, one server signs; every other key only matters with a protocol.
  "/security/threshold/protocol": {
    label: "Signing committees",
    help: "Split the pseudonym CA's and CRL Generator's keys across a committee of servers.",
    emptyText: "None (one server signs)",
  },
  "/security/threshold/members": { help: "Servers in each committee.", defaultValue: 5, relevantWhen: committee },
  "/security/threshold/quorum": { help: "Servers that must take part in each signature.", defaultValue: 3, relevantWhen: committee },
  "/security/threshold/roles": { label: "Committees for", help: "Which authorities sign as a committee.", defaultValue: ["pca", "crl", "linkage"], relevantWhen: committee },
  "/security/threshold/linkage_members": { help: "When it differs from the committee size.", emptyText: "same as above", relevantWhen: committee },
  "/security/threshold/linkage_quorum": { help: "When it differs from the committee quorum.", emptyText: "same as above", relevantWhen: committee },
  "/security/threshold/umbrella_leaves": { defaultValue: 20 },
  "/security/threshold/umbrella_inner": { defaultValue: "frost-p256" },
  "/security/threshold/preprocessed": { help: "Precompute the offline round, so a signature needs one online round.", defaultValue: false, relevantWhen: committee },
  "/security/threshold/round_timeout_ms": { help: "How long to wait for a member before leaving it out.", defaultValue: 500, relevantWhen: committee },
  "/security/threshold/retry_after_s": { help: "Wait before retrying a signing that ran out of members.", defaultValue: 30, relevantWhen: committee },
  "/security/threshold/ask_all": { label: "Ask every member, keep the first quorum", help: "Off asks exactly a quorum.", defaultValue: true, relevantWhen: committee },
  "/security/threshold/member_latency_ms": { help: "One-way delay between committee servers.", defaultValue: 20, relevantWhen: committee },

  // Failures & servers.
  "/actors/backend/entities": { label: "Server hardware", help: "A different machine or queue for one server role." },
  "/actors/backend/links": { label: "Backend links", help: "Latency and capacity of one link between two servers.", noun: "link" },
  "/actors/backend/failures": { label: "Server failures", help: "A server that is down, slow or lying for part of the run.", noun: "failure" },

  // Security & Crypto: the hardware that signs and checks.
  "/nodes/per_class": { label: "On-board unit per vehicle class", help: "A different unit for one class of vehicle; the rest use the one above." },
  "/nodes/verify_queue_depth": { help: "Messages waiting for a signature check; more are dropped.", defaultValue: 64 },
  "/nodes/peer_cert_cache": { help: "Other stations' certificates each unit keeps.", defaultValue: 128 },
  "/nodes/hsm_sign_lower_us": { help: "For a profile that publishes only a worst case; each signature takes between the two.", emptyText: "the bound only" },
  "/nodes/compute_tier": { help: "How carefully a unit's own processing time is modelled." },
  "/nodes/backend_tier": { help: "How carefully the SCMS servers' processing time is modelled." },

  // Misbehaviour authority.
  "/detection/ma/params": { label: "MA parameters", help: "Overrides for the authority's thresholds, by name." },
};

/** The overlay for a field, or undefined. */
export function overlayOf(pointer: string): Overlay | undefined {
  return SCMS_OVERLAY[pointer];
}
