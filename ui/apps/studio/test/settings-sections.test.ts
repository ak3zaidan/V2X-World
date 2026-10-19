/** The settings window's research sections and the quick panel's list share (`model.ts`, `quick.ts`). */

import { describe, expect, it } from "vitest";

import { buildResearchTree, researchSectionOf, type Field } from "../src/settings/model.js";
import { currentShare, listWithShare, patchHolds } from "../src/settings/quick.js";

const f = (path: string, group = "Other"): Field =>
  ({ pointer: `/${path.replace(/\[\]/g, "/0").split(".").join("/")}`, path, label: path, group, kind: "number" }) as unknown as Field;

describe("research sections", () => {
  it("files fields by topic before their scenario key", () => {
    expect(researchSectionOf(f("radio.models.dcc"))).toBe("dcc");
    expect(researchSectionOf(f("radio.mac.aifsn"))).toBe("mac");
    expect(researchSectionOf(f("net.channel_access"))).toBe("mac");
    expect(researchSectionOf(f("net.uu.latency_ms"))).toBe("backend");
    expect(researchSectionOf(f("net.geonet.repetition_interval_ms"))).toBe("net");
    expect(researchSectionOf(f("threats.attackers[].fraction"))).toBe("attackers");
    expect(researchSectionOf(f("time.duration_s"))).toBe("run");
    expect(researchSectionOf(f("wholly.new_key"))).toBeNull();
  });

  it("orders sections as a researcher works and keeps unclaimed groups after them", () => {
    const tree = buildResearchTree([f("wholly.new_key", "Novel"), f("security.signature"), f("radio.rat"), f("radio.devices.obu.tx_power_dbm"), f("time.duration_s")]);
    expect(tree.map((g) => g.name)).toEqual(["Run", "Radio & Channel", "Security & Crypto", "Novel"]);
    expect(tree[1].subs.map((s) => s.label)).toEqual(["General", "Devices"]);
  });

  it("gathers the certificate system under Certificates & SCMS, after Security", () => {
    for (const p of [
      "security.scms.lifecycle.certs_per_period",
      "security.scms.deployment.preset",
      "security.protocol.id",
      "security.pseudonym_change.period_s",
      "security.signer_id_policy.full_cert_every_ms",
      "security.threshold.members",
      "actors.backend.entities",
      "detection.ma",
    ])
      expect(researchSectionOf(f(p)), p).toBe("scms");
    expect(researchSectionOf(f("security.signature"))).toBe("security");
    expect(researchSectionOf(f("nodes.peer_cert_cache"))).toBe("security");
    expect(researchSectionOf(f("net.uu.latency_ms"))).toBe("backend");
    const tree = buildResearchTree([
      f("security.scms.lifecycle.crl_fetch_interval_s"),
      f("security.scms.pki_view"),
      f("security.scms.deployment.preset"),
      f("security.signature"),
      f("security.pseudonym_change.strategy"),
    ]);
    expect(tree.map((g) => g.name)).toEqual(["Security & Crypto", "Certificates & SCMS"]);
    expect(tree[1].subs.map((s) => s.label)).toEqual([
      "General",
      "Certificates, shuffle, downloads, revocation and MA",
      "Deployment and providers",
      "Pseudonym change",
    ]);
  });

  it("files the traffic, sidelink and queue settings, and never repeats a section id", () => {
    expect(researchSectionOf(f("traffic.gridlock_timeout_s"))).toBe("traffic");
    expect(researchSectionOf(f("radio.sidelink.candidate_percent"))).toBe("mac");
    expect(researchSectionOf(f("net.access_categories"))).toBe("mac");
    const tree = buildResearchTree([f("traffic.reroute_period_s", "Traffic"), f("wholly.new_key", "Traffic")]);
    const ids = tree.map((g) => g.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("list share", () => {
  it("sets the share on every element and drops the exclusive keys", () => {
    const list = [{ id: "a", count: 3 }, { id: "b", fraction: 0.05 }];
    const out = listWithShare(list, "fraction", 0.2);
    expect(out).toEqual([{ id: "a", fraction: 0.2 }, { id: "b", fraction: 0.2 }]);
    expect(currentShare(out, "fraction")).toBe(0.2);
    expect(currentShare(list, "fraction")).toBeNull();
    expect(currentShare([], "fraction")).toBeNull();
  });

  it("treats an absent option as the null a choice writes", () => {
    expect(patchHolds({ messages: {} }, [{ path: "messages.generator", pointer: "/messages/generator", value: null }])).toBe(true);
  });
});
