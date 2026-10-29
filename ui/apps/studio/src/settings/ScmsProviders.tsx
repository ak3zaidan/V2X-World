/**
 * The SCMS providers, one card each (owner, 2026-10-29: "when I want to set multiple ACAs for a
 * provider, I have to go modify the JSON"). Every server role is a stepper with its limits, and the
 * provider's share, region, trust-list state, MA and revocation rule sit beside it. The logic is in
 * `scmsDeployment.ts`.
 */

import { useContext } from "react";

import type { Choice, ItemField } from "../lib/schema.js";
import { SettingsDocContext } from "./docContext.js";
import {
  EXTRA_ROLES,
  US_ROLES,
  customBase,
  isUsProvider,
  presetOf,
  presetProviders,
  providerCards,
  providerProblems,
  roleLimits,
  shareTotal,
  strayEntries,
  writeProvider,
  type Override,
  type ProviderCard,
} from "./scmsDeployment.js";

import "../styles/scms-providers.css";

export function Stepper({
  id,
  value,
  min,
  max,
  base,
  onChange,
  label,
}: {
  id: string;
  value: number;
  min: number;
  max: number;
  base: number;
  label: string;
  onChange: (n: number) => void;
}): React.JSX.Element {
  const fixed = min === max;
  return (
    <span className={`stepper${value !== base ? " changed" : ""}`} data-testid="stepper">
      <button type="button" aria-label={`Fewer: ${label}`} disabled={fixed || value <= min} onClick={() => onChange(value - 1)}>
        −
      </button>
      <output id={id} aria-live="polite" data-testid="stepper-value">
        {value}
      </output>
      <button type="button" aria-label={`More: ${label}`} disabled={fixed || value >= max} onClick={() => onChange(value + 1)}>
        +
      </button>
    </span>
  );
}

function Select({
  id,
  choices,
  value,
  base,
  onChange,
}: {
  id: string;
  choices: readonly Choice[];
  value: string;
  base: string;
  onChange: (v: string) => void;
}): React.JSX.Element {
  return (
    <select id={id} value={value} className={value !== base ? "changed" : ""} onChange={(e) => onChange(e.target.value)}>
      {choices.some((c) => String(c.value) === value) ? null : <option value={value}>{value}</option>}
      {choices.map((c) => (
        <option key={String(c.value)} value={String(c.value)} disabled={!c.available} title={c.description}>
          {c.label}
          {c.us === false ? " (not US)" : ""}
        </option>
      ))}
    </select>
  );
}

const ROLE_SHORT: Record<string, string> = {
  root: "Root CA",
  ica: "Intermediate CA",
  eca: "Enrolment CA",
  ra: "Registration Authority",
  aca: "Pseudonym CA (ACA)",
  la1: "Linkage Authority 1",
  la2: "Linkage Authority 2",
  ma: "Misbehaviour Authority",
  crlg: "CRL Generator",
  pg: "Policy Generator",
  lop: "Location Obscurer Proxy",
  crl_store: "CRL Store",
  crl_broadcast: "CRL Broadcast",
};

function roleHint(role: string): string {
  const { min, max } = roleLimits(role);
  if (min === max) return `always ${min}`;
  if (min === 1) return "at least 1";
  return `0 to ${max}`;
}

function Card({
  card,
  idBase,
  fields,
  custom,
  names,
  doc,
  onChange,
}: {
  card: ProviderCard;
  idBase: string;
  fields: readonly ItemField[];
  custom: boolean;
  names: readonly Choice[];
  doc: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const { values: v, base } = card;
  const set = (key: string, value: unknown): void => onChange(writeProvider(doc, v.name, key, value));
  const field = (key: string): ItemField | undefined => fields.find((f) => f.key === `/${key}`);
  const roleChoices = field("components")?.keyChoices ?? [];
  const roleDesc = (r: string): string | undefined => roleChoices.find((c) => c.value === r)?.description;
  const extrasOn = EXTRA_ROLES.some((r) => (v.components[r] ?? 0) > 0);
  const roles: string[] = [...US_ROLES, ...(extrasOn ? EXTRA_ROLES : [])];
  const id = `${idBase}-${v.name.replace(/\W/g, "_")}`;
  const usOk = isUsProvider(v);
  const anySoon = fields.some((f) => /^\/(site|sites|lat_deg|lon_deg|capacity|ma_policy|ica_levels)(\/|$)/.test(f.key));
  return (
    <div className="prov-card" data-testid="provider-card" data-provider={v.name}>
      <div className="prov-head">
        {custom ? (
          <select
            aria-label="Provider name"
            value={v.name}
            onChange={(e) => {
              const list = (getList(doc) ?? []).map((x) => (x.name === v.name ? { ...x, name: e.target.value } : x));
              onChange(list);
            }}
          >
            {names
              .filter((c) => String(c.value) === v.name || !providerCards(doc).some((x) => x.values.name === String(c.value)))
              .map((c) => (
                <option key={String(c.value)} value={String(c.value)}>
                  {c.label}
                </option>
              ))}
          </select>
        ) : (
          <b>{v.name}</b>
        )}
        {card.home ? <span className="prov-tag">home</span> : null}
        {usOk ? null : (
          <span className="badge-not-us" title="Every US provider runs one of each US server and none of the CAMP-only ones">
            not US
          </span>
        )}
        <span className="grow" />
        {!custom && card.entry ? (
          <button type="button" className="linkbutton" data-testid="provider-reset" onClick={() => onChange(removeEntry(doc, v.name))}>
            Reset to preset
          </button>
        ) : null}
        {custom ? (
          <button type="button" className="linkbutton" data-testid="provider-remove" onClick={() => onChange(removeEntry(doc, v.name))}>
            Remove
          </button>
        ) : null}
      </div>

      <div className="prov-label">Servers</div>
      <div className="prov-roles">
        {roles.map((r) => {
          const lim = roleLimits(r);
          const n = v.components[r] ?? 0;
          const notUs = (US_ROLES as readonly string[]).includes(r) ? n === 0 : n > 0;
          return (
            <div key={r} className={`prov-role${lim.min === lim.max ? " fixed" : ""}`} title={roleDesc(r)} data-role={r}>
              <label htmlFor={`${id}-${r}`} className="prov-role-name">
                {ROLE_SHORT[r] ?? r}
                <span className="prov-role-hint">{notUs ? <span className="warn-text">not US</span> : roleHint(r)}</span>
              </label>
              {lim.min === lim.max ? (
                <span className="stepper fixed" id={`${id}-${r}`}>
                  <output>{n}</output>
                </span>
              ) : (
                <Stepper
                  id={`${id}-${r}`}
                  label={ROLE_SHORT[r] ?? r}
                  value={n}
                  min={lim.min}
                  max={lim.max}
                  base={base.components[r] ?? 0}
                  onChange={(x) => set(`components/${r}`, x)}
                />
              )}
            </div>
          );
        })}
      </div>
      {!extrasOn ? (
        <button
          type="button"
          className="linkbutton prov-extras"
          onClick={() => set("components/lop", 1)}
          title="Location Obscurer Proxy, CRL Store and CRL Broadcast: parts of the CAMP reference design the US does not run"
        >
          + CAMP-only servers (not US)
        </button>
      ) : null}
      {(v.components.aca ?? 0) > 1 ? <div className="faint prov-note">Extra ACAs are listed but do not share the load yet.</div> : null}

      <div className="prov-label">Operations</div>
      <div className="prov-fields">
        <label className="prov-field">
          <span>Share of vehicles</span>
          <span className="prov-inline">
            <input
              type="number"
              min={0}
              max={100}
              step={1}
              className={v.fleet_share !== base.fleet_share ? "changed" : ""}
              value={Math.round(v.fleet_share * 1000) / 10}
              onChange={(e) => set("fleet_share", e.target.value === "" ? undefined : Math.max(0, Math.min(100, Number(e.target.value))) / 100)}
            />
            <span className="unit">%</span>
          </span>
        </label>
        {field("region")?.choices ? (
          <label className="prov-field">
            <span>Region (assumed)</span>
            <Select id={`${id}-region`} choices={field("region")?.choices ?? []} value={v.region} base={base.region} onChange={(x) => set("region", x)} />
          </label>
        ) : null}
        <label className="prov-field">
          <span>Root on the trust list</span>
          <span className="prov-inline">
            <input type="checkbox" checked={v.on_ctl} disabled={card.home && v.on_ctl} onChange={(e) => set("on_ctl", e.target.checked)} />
            <span className="dim">{v.on_ctl ? "trusted" : "not trusted"}</span>
          </span>
        </label>
        {field("ma_mode")?.choices ? (
          <label className="prov-field">
            <span>MA decides</span>
            <Select id={`${id}-ma`} choices={field("ma_mode")?.choices ?? []} value={v.ma_mode} base={base.ma_mode} onChange={(x) => set("ma_mode", x)} />
          </label>
        ) : null}
        {v.ma_mode === "manual-review" ? (
          <label className="prov-field">
            <span>Review time</span>
            <span className="prov-inline">
              <input
                type="number"
                min={0.01}
                step={0.5}
                className={v.ma_review_s !== base.ma_review_s ? "changed" : ""}
                value={Math.round((v.ma_review_s / 3600) * 100) / 100}
                onChange={(e) => set("ma_review_s", e.target.value === "" ? undefined : Math.max(1, Number(e.target.value) * 3600))}
              />
              <span className="unit">h</span>
            </span>
          </label>
        ) : null}
        {field("crl_publication")?.choices ? (
          <label className="prov-field">
            <span>Revocations published</span>
            <Select
              id={`${id}-crl`}
              choices={field("crl_publication")?.choices ?? []}
              value={v.crl_publication}
              base={base.crl_publication}
              onChange={(x) => set("crl_publication", x)}
            />
          </label>
        ) : null}
      </div>
      {anySoon ? (
        <div className="faint prov-soon" data-testid="provider-soon">
          Coming soon: server sites, server capacity, per-provider MA policy, more CA levels.
        </div>
      ) : null}
    </div>
  );
}

function getList(doc: unknown): Override[] | undefined {
  const s = (doc as { security?: { scms?: { deployment?: { providers?: unknown } } } } | null)?.security?.scms?.deployment?.providers;
  return Array.isArray(s) ? (s as Override[]) : undefined;
}

function removeEntry(doc: unknown, name: string): Override[] | undefined {
  const list = (getList(doc) ?? []).filter((x) => x.name !== name);
  return list.length === 0 ? undefined : list;
}

export function ScmsProviders({
  idBase,
  fields,
  onChange,
}: {
  idBase: string;
  fields: readonly ItemField[];
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const doc = useContext(SettingsDocContext);
  const preset = presetOf(doc);
  const custom = preset === "custom";
  const cards = providerCards(doc);
  const problems = providerProblems(cards, preset);
  const stray = strayEntries(doc);
  const names = fields.find((f) => f.key === "/name")?.choices ?? [];
  const free = names.filter((c) => !cards.some((x) => x.values.name === String(c.value)));
  const total = shareTotal(cards);
  return (
    <div className="prov-list" data-testid="scms-providers">
      {cards.map((c) => (
        <Card key={c.values.name} card={c} idBase={idBase} fields={fields} custom={custom} names={names} doc={doc} onChange={onChange} />
      ))}
      {custom && cards.length === 0 ? (
        <div className="prov-empty">
          <span className="dim">No providers yet.</span>
          <button
            type="button"
            className="btn small"
            onClick={() =>
              onChange(
                presetProviders("us-production-2025").map((p, i) => ({
                  name: p.name,
                  fleet_share: p.fleet_share,
                  ...(i === 1 ? { components: { aca: 2 } } : {}),
                })),
              )
            }
          >
            Start from the US providers
          </button>
        </div>
      ) : null}
      <div className="prov-foot">
        {custom && free.length > 0 && cards.length < 9 ? (
          <button
            type="button"
            className="btn small"
            data-testid="provider-add"
            onClick={() => onChange([...(getList(doc) ?? []), { name: String(free[0].value), fleet_share: customBase("").fleet_share }])}
          >
            Add provider
          </button>
        ) : null}
        {!custom ? <span className="faint">To add or remove providers, choose Custom providers above.</span> : null}
        <span className="grow" />
        {cards.length > 1 ? (
          <span className={Math.abs(total - 1) > 1e-6 ? "err-text" : "faint"}>Shares total {Math.round(total * 1000) / 10} %</span>
        ) : null}
      </div>
      {problems.length > 0 || stray.length > 0 ? (
        <div className="setting-error" role="alert" data-testid="provider-problems">
          {problems.map((p) => (
            <div key={p}>{p}</div>
          ))}
          {stray.length > 0 ? (
            <div>
              {stray.join(", ")} {stray.length === 1 ? "is" : "are"} not in this layout.{" "}
              <button type="button" className="linkbutton" onClick={() => onChange((getList(doc) ?? []).filter((x) => !stray.includes(x.name)))}>
                Remove
              </button>
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
