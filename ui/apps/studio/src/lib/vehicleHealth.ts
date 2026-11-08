/**
 * Whether anything about the selected radio is unhealthy, in words a reader can act on.
 *
 * The chase HUD and the vehicle overlay show one line: "healthy", or the problems found, worst
 * first. Each check is a published threshold or a plain fact (a dropped message is a lost message);
 * nothing here is a tuned number.
 *
 *  * **Channel load.** ETSI TS 102 687 V1.2.1 Annex A (Tables A.1 and A.2): the reactive DCC leaves
 *    its Relaxed state at a channel busy ratio of 0.30, and reaches Restrictive above 0.60 (Table
 *    A.2; 0.65 in Table A.1 — the lower is used, so the warning comes early rather than late). At
 *    Restrictive a station sends at its minimum rate, so awareness of it falls. The adaptive DCC of
 *    the same document steers to 0.68 (§5.4, Table 3); above that the channel is past its design
 *    load under either algorithm.
 *  * **Drops.** Any message dropped from a queue in the telemetry window (overflow, policy skip,
 *    reassembly timeout, CRL backlog) is a message an application never saw.
 *  * **Verification delay.** A BSM is sent every 100 ms at the nominal 10 Hz of SAE J2945/1, so a
 *    95th-percentile wait before verification longer than 100 ms means the receiver is checking a
 *    message after its successor has already arrived.
 *  * **Credentials.** A radio with no valid pseudonym certificate cannot sign, so under IEEE
 *    1609.2 nothing it sends will verify; one on the CRL has stopped sending; one whose backend is
 *    out of reach cannot top its certificates up (IEEE 1609.2.1 provisioning).
 *
 * Framework- and DOM-free, so it can be tested in plain Node.
 */

import type { NodeTelemetry } from "@vwp/protocol";

import { isU16Sentinel } from "./format.js";
import { totalDrops } from "./telemetry.js";
import type { NodeSecurityRow } from "./security.js";

/** ETSI TS 102 687 V1.2.1 Annex A: the reactive DCC leaves Relaxed at this channel busy ratio. */
export const CBR_BUSY = 0.3;
/** ETSI TS 102 687 V1.2.1 Annex A Table A.2: Restrictive is entered above this channel busy ratio. */
export const CBR_CONGESTED = 0.6;
/** SAE J2945/1: the nominal BSM interval at 10 Hz, ms. */
export const BSM_INTERVAL_MS = 100;

export type HealthLevel = "warn" | "bad";

export interface HealthIssue {
  readonly key: string;
  readonly level: HealthLevel;
  /** A short phrase for the HUD ("channel congested (CBR 0.71)"). */
  readonly text: string;
  /** Why it matters and the standard behind the threshold, for a tooltip. */
  readonly why: string;
}

/** Every problem found, worst first; empty when the radio looks healthy. */
export function vehicleHealth(t: NodeTelemetry | null, sec: NodeSecurityRow | null): HealthIssue[] {
  const out: HealthIssue[] = [];
  if (sec?.self_revoked === true) {
    out.push({
      key: "revoked",
      level: "bad",
      text: "revoked: stopped sending",
      why: "This radio found itself on the certificate revocation list and stopped broadcasting.",
    });
  }
  if (sec && typeof sec.pool_valid === "number" && sec.pool_valid === 0 && sec.self_revoked !== true) {
    out.push({
      key: "no-cert",
      level: "bad",
      text: "no valid certificate",
      why: "With no valid pseudonym certificate the radio cannot sign, so receivers cannot verify what it sends (IEEE 1609.2).",
    });
  }
  if (t) {
    if (!isU16Sentinel(t.cbrPm)) {
      const cbr = t.cbrPm / 1000;
      if (cbr > CBR_CONGESTED) {
        out.push({
          key: "cbr",
          level: "bad",
          text: `channel congested (CBR ${cbr.toFixed(2)})`,
          why: `The channel is busy ${(cbr * 100).toFixed(0)} % of the time. Above ${CBR_CONGESTED} the ETSI reactive DCC is Restrictive and stations send at their minimum rate (ETSI TS 102 687 V1.2.1 Annex A).`,
        });
      } else if (cbr >= CBR_BUSY) {
        out.push({
          key: "cbr",
          level: "warn",
          text: `channel busy (CBR ${cbr.toFixed(2)})`,
          why: `The channel is busy ${(cbr * 100).toFixed(0)} % of the time. From ${CBR_BUSY} the ETSI reactive DCC leaves Relaxed and starts slowing transmissions (ETSI TS 102 687 V1.2.1 Annex A). Below ${CBR_BUSY} is healthy.`,
        });
      }
    }
    const drops = totalDrops(t);
    if (drops > 0) {
      out.push({
        key: "drops",
        level: "warn",
        text: `${drops} message${drops === 1 ? "" : "s"} dropped`,
        why: "Messages dropped from this radio's queues in the last telemetry window. A dropped message never reaches an application. The Queues section of the details breaks them down by cause.",
      });
    }
    if (Number.isFinite(t.verifyWaitP95Ms) && t.verifyWaitP95Ms > BSM_INTERVAL_MS) {
      out.push({
        key: "verify-wait",
        level: "warn",
        text: `verification slow (${t.verifyWaitP95Ms.toFixed(0)} ms)`,
        why: `95 % of messages wait up to ${t.verifyWaitP95Ms.toFixed(0)} ms before their signature is checked, longer than the ${BSM_INTERVAL_MS} ms between two BSMs at 10 Hz (SAE J2945/1).`,
      });
    }
  }
  if (sec && sec.link_up === false && sec.link !== "offline") {
    out.push({
      key: "backend",
      level: "warn",
      text: "backend out of reach",
      why: "The radio cannot reach the credential backend, so it cannot top up its certificates or fetch a new revocation list until it does (IEEE 1609.2.1).",
    });
  }
  return out.sort((a, b) => (a.level === b.level ? 0 : a.level === "bad" ? -1 : 1));
}
