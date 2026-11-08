/**
 * The certificate views after the second fresh-eyes review: one name per vehicle (its map number,
 * with its device id beside it), whose revocation list a vehicle holds, how a revoked vehicle is
 * revoked, the newest list in the health strip, one number per concept on a card, acronyms routed
 * through the glossary, messages in words, and the map's rings in the badges' colours.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { beforeEach, describe, expect, it } from "vitest";

import { trustOf } from "../src/lib/pki.js";
import { evidencePlain, caseSteps } from "../src/lib/pkiCases.js";
import { glossTerms, glossary } from "../src/lib/pkiGlossary.js";
import { deviceForQuery, noteDeviceNodes, resetDeviceNodes, vehicleMatches, vehicleName, deviceLine } from "../src/lib/pkiNames.js";
import { piggyback, transportText } from "../src/lib/pkiPdu.js";
import {
  CREDENTIAL_COLORS,
  GROUND_TRUTH,
  asCases,
  asCrls,
  asVehicles,
  installedList,
  installedListText,
  mapTags,
  revocationStory,
} from "../src/lib/pkiRecords.js";
import { componentName, counterConcept, healthOf } from "../src/lib/pkiTopology.js";

const dir = (f: string): string => join(fileURLToPath(new URL(".", import.meta.url)), "fixtures/pki", f);
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const tabs = JSON.parse(readFileSync(dir("pki-tabs.json"), "utf8")) as Record<string, any>;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const studio = JSON.parse(readFileSync(dir("pki-studio.json"), "utf8")) as Record<string, any>;
const vehicles = asVehicles(tabs.vehicles)!;
const crls = asCrls(tabs.crls)!;
const cases = asCases(tabs.cases[GROUND_TRUTH])!;

describe("one name per vehicle", () => {
  beforeEach(() => resetDeviceNodes());

  it("names a vehicle by its map number once an answer pairs it, and by its device id, said so, before", () => {
    expect(vehicleName("ee/1000020")).toBe("Vehicle (device 1000020)");
    expect(deviceLine("ee/1000020")).toBeNull();
    noteDeviceNodes(tabs.vehicles);
    expect(vehicleName("ee/1000020")).toBe("Vehicle 20");
    expect(deviceLine("ee/1000020")).toBe("device 1000020");
    // Every view's builder agrees: the component namer and the lane label read the same pairs.
    expect(componentName("ee/1000020")).toBe("Vehicle 20");
    expect(vehicleName("rsu/3")).toBe("Roadside unit 3");
  });

  it("learns the pairs from cases and CRL owners too, and never guesses from the digits", () => {
    noteDeviceNodes(tabs.cases[GROUND_TRUTH]);
    expect(vehicleName("ee/1000025")).toBe("Vehicle 25");
    // A reporter the records name without a node keeps its device id.
    expect(vehicleName("ee/900105")).toBe("Vehicle (device 900105)");
    expect(vehicleName("ee/1000099")).toBe("Vehicle (device 1000099)");
  });

  it("finds a vehicle by either number, and turns the map's number into the device id the engine filters by", () => {
    noteDeviceNodes(tabs.vehicles);
    expect(vehicleMatches("20", "ee/1000020", 20)).toBe(true);
    expect(vehicleMatches("1000020", "ee/1000020", 20)).toBe(true);
    expect(vehicleMatches("Vehicle 20", "ee/1000020", 20)).toBe(true);
    expect(vehicleMatches("2", "ee/1000020", 20)).toBe(false);
    expect(vehicleMatches("", "ee/1000020", 20)).toBe(true);
    expect(deviceForQuery("20")).toBe("ee/1000020");
    expect(deviceForQuery("vehicle 20")).toBe("ee/1000020");
    expect(deviceForQuery("1000020")).toBe("ee/1000020");
    expect(deviceForQuery("ee/1000020")).toBe("ee/1000020");
    expect(deviceForQuery("777")).toBe("777");
  });
});

describe("revocation, said once and the same way", () => {
  it("names whose list a vehicle installed when exactly one provider's head matches", () => {
    const l = installedList({ crl_version: 2, crl_entries: 2 }, crls.crls);
    expect(l).toEqual({ provider: "provider-a", version: 2, entries: 2 });
    expect(installedListText(l)).toBe("provider-a v2");
    // Version 1 with no entries is both providers' first list: no provider is named.
    expect(installedListText(installedList({ crl_version: 1, crl_entries: 0 }, crls.crls))).toBe("v1");
    expect(installedListText(installedList({}, crls.crls))).toBe("none");
  });

  it("tells a revoked vehicle's story: the list that names it, its case and how many refuse it", () => {
    const v = vehicles.vehicles.find((x) => (x.truth?.revoked_certs ?? 0) > 0)!;
    const s = revocationStory(v.device, v.truth?.revoked_by_crls, crls.crls, cases.cases)!;
    expect(s.lists.map((h) => `${h.provider} v${h.version}`)).toEqual(["provider-a v2"]);
    const c = cases.cases.find((x) => x.truth?.accused.some((a) => a.device === v.device));
    expect(s.caseId).toBe(c?.case ?? null);
    if (c) expect(s.refusing).not.toBeNull();
    expect(revocationStory("ee/1000019", [], crls.crls, cases.cases)).toBeNull();
  });

  it("dates the strip's list by the newest of any provider, and names it", () => {
    const newest = trustOf(studio.run["inspect.trust"].newest)!;
    const h = healthOf(newest, null, null);
    const heads = newest.state.crls;
    if (heads.length > 0) {
      const top = heads.reduce((a, b) => (b.issued_at > a.issued_at ? b : a));
      expect(h.crlNewest).toEqual({ provider: top.provider, version: top.version });
      expect(h.crlAgeS).toBeCloseTo(Math.max(0, (newest.t_ns - top.issued_at) / 1e9), 6);
    }
  });
});

describe("cards and cases in words", () => {
  it("counts one concept once on a card", () => {
    expect(counterConcept("reports")).toBe(counterConcept("reports_received"));
    expect(counterConcept("requests")).toBe(counterConcept("requests_accepted"));
    expect(counterConcept("opened_reports")).not.toBe(counterConcept("reports_received"));
    expect(counterConcept("enrolments")).not.toBe(counterConcept("enrolled_devices"));
  });

  it("routes every acronym the engine writes through the glossary", () => {
    const parts = glossTerms("the RA sees B_ij, J_ij and the PLVs sealed to the LA-ACA key, from the caterpillar and cocoon keys");
    const terms = parts.filter((p) => p.term).map((p) => p.term);
    expect(terms).toEqual(["jij", "plv", "la-aca-key", "butterfly"]);
    expect(parts.map((p) => p.text).join("")).toBe("the RA sees B_ij, J_ij and the PLVs sealed to the LA-ACA key, from the caterpillar and cocoon keys");
    for (const t of terms) expect(glossary(t as string), t as string).not.toBeNull();
  });

  it("says what unverified evidence means without capitals, and how many reports the rule acted on", () => {
    expect(evidencePlain({ type_name: "1VInconsistent", subtypes: [], verified: false })).not.toMatch(/NOT/);
    const c = cases.cases[0];
    const corroborated = caseSteps(c).find((s) => s.stage === "corroborated")!;
    expect(corroborated.detail).toMatch(/enough for the MA's rule to act/);
    const total = (c.truth?.reporters ?? []).reduce((n, r) => n + (r.reports ?? 0), 0);
    if (total > 1) expect(corroborated.detail).toContain(`${total} reports`);
  });

  it("says how a message travelled and why it carries two secured messages", () => {
    expect(transportText("cellular-uu")).toMatch(/^cellular \(Uu/);
    expect(transportText("backend-net")).toBe("the authorities' backend network");
    expect(piggyback({ pdus: [{ type: "SecuredMisbehaviorReport" }, { type: "EeRaDownloadRequestSpdu" }] })).toMatch(/rides with the vehicle's next certificate download request/);
    expect(piggyback({ pdus: [{ type: "EeRaCertRequestSpdu" }] })).toBeNull();
  });
});

describe("the map's rings", () => {
  it("use the badges' hues, with revoked drawn as a double ring", () => {
    expect(CREDENTIAL_COLORS["topup-due"]).not.toBe(CREDENTIAL_COLORS.starving);
    expect(CREDENTIAL_COLORS.revoked).not.toBe(CREDENTIAL_COLORS.untrusted);
    const rings = mapTags("credential", { vehicles: vehicles.vehicles, crls, cases: [], view: GROUND_TRUTH, nowNs: vehicles.t_ns });
    const revoked = vehicles.vehicles.filter((v) => (v.truth?.revoked_certs ?? 0) > 0);
    expect(revoked.length).toBeGreaterThan(0);
    for (const v of revoked) expect(rings.get(v.node)).toEqual({ color: CREDENTIAL_COLORS.revoked, emphasis: true });
  });
});
