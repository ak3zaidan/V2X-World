/**
 * The SCMS dependent sets and the US banner (docs/design/scms-real/08-settings-ux-and-gaps.md §4.1
 * item 6, §4.2 items 2 and 7): a failure's server is a provider of the deployment the document
 * names, a provider's name is one its preset deploys, and the engine's US verdict reaches the store.
 */

import { describe, expect, it } from "vitest";

import { composeOf, joinComposed, presetIn, providerSetsOf, providersIn, splitComposed } from "../src/lib/schema.js";
import { usSystemOf } from "../src/settings/model.js";

/** As `publish.rs` writes it on `actors.backend.failures[].node`. */
const PROVIDERS = {
  preset_path: "security.scms.deployment.preset",
  default_preset: "us-production-2025",
  custom_path: "security.scms.deployment.providers",
  by_preset: {
    "us-production-2025": ["provider-a", "provider-b"],
    "us-test-ctl-2025": ["provider-a", "provider-b", "provider-c", "provider-d", "provider-e", "provider-f"],
    "camp-reference": ["provider-a"],
  },
};
const SETS = providerSetsOf(PROVIDERS);

describe("dependent sets", () => {
  it("reads the providers the document deploys: the default preset, a named one, a custom list", () => {
    expect(SETS).toBeDefined();
    if (!SETS) return;
    expect(providersIn(SETS, {})).toEqual(["provider-a", "provider-b"]);
    const test = { security: { scms: { deployment: { preset: "us-test-ctl-2025" } } } };
    expect(providersIn(SETS, test)).toHaveLength(6);
    const custom = { security: { scms: { deployment: { preset: "custom", providers: [{ name: "acme" }, { name: "zeta" }] } } } };
    expect(presetIn(SETS, custom)).toBe("custom");
    expect(providersIn(SETS, custom)).toEqual(["acme", "zeta"]);
  });

  it("parses the composite picker's description off a published row", () => {
    const c = composeOf({ "x-options-compose": { qualifier: "provider", separator: "/", providers: PROVIDERS } });
    expect(c?.separator).toBe("/");
    expect(c?.providers.byPreset["camp-reference"]).toEqual(["provider-a"]);
    expect(composeOf({})).toBeUndefined();
  });
});

describe("the provider and server picker", () => {
  const providers = ["provider-a", "provider-b"];
  it("writes the home provider's server bare and another's qualified, and reads both back", () => {
    expect(joinComposed("provider-a", "crlg", "/", providers)).toBe("crlg");
    expect(joinComposed("provider-b", "crlg", "/", providers)).toBe("provider-b/crlg");
    expect(joinComposed("provider-b", "", "/", providers)).toBeUndefined();
    expect(splitComposed("provider-b/ma", "/", providers)).toEqual({ provider: "provider-b", role: "ma" });
    expect(splitComposed("ra", "/", providers)).toEqual({ provider: "provider-a", role: "ra" });
  });
});

describe("the US banner", () => {
  it("carries the engine's verdict into the validation view, and nothing when there is none", () => {
    expect(usSystemOf({ valid: true, us_system: { is_us: false, reasons: ["electors 7"] } })).toEqual({
      usSystem: { isUs: false, reasons: ["electors 7"] },
    });
    expect(usSystemOf({ valid: true, us_system: { is_us: true, reasons: [] } }).usSystem?.isUs).toBe(true);
    expect(usSystemOf({ valid: true })).toEqual({});
  });
});
