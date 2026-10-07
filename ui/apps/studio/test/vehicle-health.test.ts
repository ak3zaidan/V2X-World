/**
 * The selected vehicle's health line (`lib/vehicleHealth.ts`) and the feed's reading pause
 * (`lib/feed.ts`, `readMessage`): each check turns red on the fault it names, and only on it.
 */

import { describe, expect, it } from "vitest";

import type { NodeTelemetry } from "@vwp/protocol";

import { EMPTY_FEED, applyPush, followFeed, keyOf, readMessage, setPaused } from "../src/lib/feed.js";
import { CBR_BUSY, CBR_CONGESTED, vehicleHealth } from "../src/lib/vehicleHealth.js";

/** A quiet, healthy radio: every count zero, the channel at 10 %. */
const QUIET: NodeTelemetry = {
  storageUsedB: 0n, storageTotalB: 0n, nextTopupNs: 0n, crlBytes: 0n, outboxBytes: 0n, clockOffsetNs: 0n,
  nodeId: 1, ramUsedKib: 0, ramTotalKib: 0,
  dropRxOverflow: 0, dropVerifyPolicySkip: 0, dropVerifyOverflow: 0, dropTxOverflow: 0, dropReassemblyTimeout: 0, dropCrlBacklog: 0,
  certStored: 0, crlEntries: 0, outboxMsgs: 0, peerCacheEntries: 0, p2pcdRequests: 0, fullCertMsgs: 0,
  msgsInPerS: 50, msgsOutPerS: 10, verificationsPerS: 50, verifyWaitP50Ms: 1, verifyWaitP95Ms: 4,
  gnssHdop: 1, gnssSigmaM: 1, clockDriftPpm: 0, posErrorM: 1, airtimeMsPerS: 5, cpuUtilPm: 100, hsmUtilPm: 100,
  qRxP50: 0, qRxP95: 0, qVerifyP50: 0, qVerifyP95: 0, qAppP50: 0, qAppP95: 0, qTxP50: 0, qTxP95: 0, qCrlP50: 0, qCrlP95: 0,
  dccState: 0, cbrPm: 100, txPowerCdbm: 2000, nbrTotal: 5, nbrVerified: 5, nbrUnverified: 0, nbrRevoked: 0,
  certActive: 1, crlExpansionPm: 0, unverifiedRatioPm: 0, gnssFix: 2, nodeState: 0, verifyPolicy: 0,
};

const keys = (t: NodeTelemetry, sec: Parameters<typeof vehicleHealth>[1] = null): string[] => vehicleHealth(t, sec).map((h) => `${h.key}:${h.level}`);

describe("vehicle health", () => {
  it("is empty for a quiet radio with certificates and its backend up", () => {
    expect(keys(QUIET, { pool_valid: 20, link: "cellular", link_up: true })).toEqual([]);
  });

  it("names a busy and a congested channel at the ETSI TS 102 687 thresholds", () => {
    expect(keys({ ...QUIET, cbrPm: CBR_BUSY * 1000 - 1 })).toEqual([]);
    expect(keys({ ...QUIET, cbrPm: CBR_BUSY * 1000 })).toEqual(["cbr:warn"]);
    expect(keys({ ...QUIET, cbrPm: CBR_CONGESTED * 1000 })).toEqual(["cbr:warn"]);
    expect(keys({ ...QUIET, cbrPm: CBR_CONGESTED * 1000 + 1 })).toEqual(["cbr:bad"]);
    // The not-modelled sentinel is not a channel load.
    expect(keys({ ...QUIET, cbrPm: 0xffff })).toEqual([]);
  });

  it("names drops, slow verification, a lost backend, no certificate and revocation, worst first", () => {
    expect(keys({ ...QUIET, dropTxOverflow: 3 })).toEqual(["drops:warn"]);
    expect(keys({ ...QUIET, verifyWaitP95Ms: 101 })).toEqual(["verify-wait:warn"]);
    expect(keys({ ...QUIET, verifyWaitP95Ms: 100 })).toEqual([]);
    expect(keys(QUIET, { link: "cellular", link_up: false })).toEqual(["backend:warn"]);
    expect(keys({ ...QUIET, dropRxOverflow: 1 }, { pool_valid: 0 })).toEqual(["no-cert:bad", "drops:warn"]);
    expect(keys(QUIET, { pool_valid: 0, self_revoked: true })).toEqual(["revoked:bad"]);
  });
});

describe("reading a message pauses the feed", () => {
  const push = (node: number, from: number, n: number) => ({
    v: 1,
    node,
    t_ns: from * 100_000_000,
    reset: false,
    sent: Array.from({ length: n }, (_, i) => ({ msg: from + n - i, t_ns: (from + n - i) * 100_000_000, type: "bsm" })),
    received: [],
    queues: null,
    omitted: { sent: 0, received: 0 },
    undetected: 0,
  }) as unknown as Parameters<typeof applyPush>[1];

  it("holds pushes while a message is open, and closing it resumes", () => {
    let f = applyPush(followFeed(EMPTY_FEED, 7), push(7, 0, 3));
    const key = keyOf("sent", f.sent[1]);
    f = readMessage(f, "sent", key);
    expect(f.paused && f.pausedForReading).toBe(true);
    f = applyPush(f, push(7, 3, 2));
    expect(f.sent).toHaveLength(3);
    expect(f.held).toHaveLength(1);
    f = readMessage(f, "sent", null);
    expect(f.open).toBeNull();
    expect(f.paused).toBe(false);
    expect(f.sent).toHaveLength(5);
  });

  it("leaves a pause the reader made alone", () => {
    let f = setPaused(applyPush(followFeed(EMPTY_FEED, 7), push(7, 0, 3)), true);
    f = readMessage(f, "sent", keyOf("sent", f.sent[0]));
    expect(f.pausedForReading).toBe(false);
    f = readMessage(f, "sent", null);
    expect(f.paused).toBe(true);
  });
});
