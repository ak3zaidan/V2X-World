/**
 * The SCMS provider cards and the window's own SCMS words (`settings/scmsDeployment.ts`,
 * `settings/legacyParams.ts`, `model.refineError`).
 */

import { describe, expect, it } from "vitest";

import { inheritedValue, moveIntoFields } from "../src/settings/legacyParams.js";
import { refineError } from "../src/settings/model.js";
import { localUsVerdict, providerCards, providerProblems, roleLimits, writeProvider } from "../src/settings/scmsDeployment.js";

const doc = (deployment: unknown): unknown => ({ security: { scms: { deployment } } });

describe("SCMS provider cards", () => {
  it("shows the preset's providers with their servers when the document names none", () => {
    const cards = providerCards({});
    expect(cards.map((c) => c.values.name)).toEqual(["provider-a", "provider-b"]);
    expect(cards[1].values.components.aca).toBe(2);
    expect(cards[0].values.on_ctl).toBe(true);
  });

  it("writes the smallest override and drops it when it matches the preset again", () => {
    const once = writeProvider({}, "provider-b", "components/aca", 3);
    expect(once).toEqual([{ name: "provider-b", components: { aca: 3 } }]);
    const back = writeProvider(doc({ providers: once }), "provider-b", "components/aca", 2);
    expect(back).toBeUndefined();
  });

  it("says the review time's mode with it, and drops it under automatic", () => {
    expect(writeProvider({}, "provider-a", "ma_review_s", 3600)).toEqual([{ name: "provider-a", ma_review_s: 3600 }]);
    const d = doc({ providers: [{ name: "provider-a", ma_mode: "manual-review", ma_review_s: 3600 }] });
    expect(writeProvider(d, "provider-a", "ma_mode", "automatic")).toEqual([{ name: "provider-a", ma_mode: "automatic" }]);
  });

  it("keeps the engine's limits: one root and ICA, at least one RA and ACA", () => {
    expect(roleLimits("root")).toEqual({ min: 1, max: 1 });
    expect(roleLimits("aca")).toEqual({ min: 1, max: 8 });
    expect(roleLimits("la1")).toEqual({ min: 0, max: 8 });
  });

  it("says in words when the shares do not add up", () => {
    const d = doc({ providers: [{ name: "provider-a", fleet_share: 0.7 }] });
    expect(providerProblems(providerCards(d), "us-production-2025")).toEqual(["Shares of vehicles add up to 120 %; they must add up to 100 %."]);
    expect(providerProblems(providerCards(doc({ preset: "custom" })), "custom")).toEqual(["Add at least one provider."]);
  });

  it("is not the US system when a provider drops a US server", () => {
    const d = doc({ providers: [{ name: "provider-b", components: { la1: 0 } }] });
    expect(localUsVerdict({}, []).isUs).toBe(true);
    expect(localUsVerdict(d, [])).toEqual({ isUs: false, reasons: ["provider-b's servers"] });
  });
});

describe("the older parameter block", () => {
  const d = { security: { protocol: { params: { certs_per_period: 5, other: 1 } } } };
  it("feeds a lifecycle field while the field is unset", () => {
    expect(inheritedValue(d, "/security/scms/lifecycle/certs_per_period")).toBe(5);
    expect(inheritedValue(d, "/security/scms/tls")).toBeUndefined();
  });
  it("moves into the form, keeping keys that have no field", () => {
    expect(moveIntoFields(d, ["certs_per_period"])).toEqual([
      { pointer: "/security/scms/lifecycle/certs_per_period", value: 5 },
      { pointer: "/security/protocol/params", value: { other: 1 } },
    ]);
  });
});

describe("deployment errors", () => {
  it("are moved to the field they name, in plain words", () => {
    const e = refineError({ path: "/security/scms/deployment", message: "security.scms.deployment: quorum: 3 of 2 electors; it must be 1 to the elector count (≤ 32)" });
    expect(e.path).toBe("/security/scms/deployment/quorum");
    expect(e.message).toBe("3 of 2 electors; it must be 1 to the elector count (≤ 32)");
  });
});
