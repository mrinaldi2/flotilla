// The price table (0.24.0, `usage_v1`): US dollars per million tokens of each
// kind, per model family, which every cost on the Usage page is estimated
// from. Flotilla seeds its own estimates; the owner edits cells, adds and
// removes models, and saves the whole table.

import { useCallback, useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import type { UsagePrice } from "../../protocol/usage";
import { errText, fmtTimestamp } from "../../util";

/** The most rows the service keeps. */
const MAX_ROWS = 64;
const FALLBACK = "*";
const KINDS = ["input", "output", "cache_read", "cache_write"] as const;
type Kind = (typeof KINDS)[number];
const KIND_LABELS: Readonly<Record<Kind, string>> = {
  input: "Input",
  output: "Output",
  cache_read: "Cache read",
  cache_write: "Cache write",
};

/** One row as typed: every cell a string until it's saved. */
type Row = { readonly key: number; readonly model: string } & Readonly<Record<Kind, string>>;

interface PriceTableProps {
  readonly client: DaemonApi;
  readonly connected: boolean;
  /** The table changed: costs read again. */
  readonly onSaved: () => void;
}

interface Loaded {
  readonly prices: readonly UsagePrice[];
  readonly asOf: string | null;
}

function rowsOf(prices: readonly UsagePrice[]): readonly Row[] {
  return prices.map((p, key) => ({
    key,
    model: p.model,
    input: String(p.input),
    output: String(p.output),
    cache_read: String(p.cache_read),
    cache_write: String(p.cache_write),
  }));
}

function dollars(cell: string): number | undefined {
  const n = Number(cell.trim());
  return cell.trim() !== "" && Number.isFinite(n) && n >= 0 ? n : undefined;
}

/** What's wrong with a row's name, if anything. */
function modelProblem(rows: readonly Row[], row: Row): string | null {
  const model = row.model.trim();
  if (model === "" || model.length > 64) {
    return "Name every model, in at most 64 characters.";
  }
  const twice = rows.filter((r) => r.model.trim().toLowerCase() === model.toLowerCase());
  return twice.length > 1 ? `${model} is listed twice.` : null;
}

/** The table to send, or the first thing wrong with the draft. */
function checked(rows: readonly Row[]): readonly UsagePrice[] | string {
  if (rows.length > MAX_ROWS) {
    return `At most ${MAX_ROWS} models.`;
  }
  if (!rows.some((r) => r.model.trim() === FALLBACK)) {
    return "Keep the * row: it prices every model no other row matches.";
  }
  const prices: UsagePrice[] = [];
  for (const row of rows) {
    const problem = modelProblem(rows, row);
    if (problem !== null) {
      return problem;
    }
    const [input, output, cacheRead, cacheWrite] = KINDS.map((kind) => dollars(row[kind]));
    if (
      input === undefined ||
      output === undefined ||
      cacheRead === undefined ||
      cacheWrite === undefined
    ) {
      return `${row.model.trim()}: every price is a number of dollars, 0 or more.`;
    }
    prices.push({
      model: row.model.trim(),
      input,
      output,
      cache_read: cacheRead,
      cache_write: cacheWrite,
    });
  }
  return prices;
}

/** A row's cells, without its React key. */
function cellsOf(rows: readonly Row[]): string {
  return JSON.stringify(rows.map(({ key: _key, ...cells }) => cells));
}

function same(a: readonly Row[], b: readonly Row[]): boolean {
  return cellsOf(a) === cellsOf(b);
}

function usePrices(client: DaemonApi, connected: boolean) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [error, setError] = useState<string | null>(null);
  const take = useCallback((prices: readonly UsagePrice[], asOf: string | null | undefined) => {
    setLoaded({ prices, asOf: asOf ?? null });
    setError(null);
  }, []);
  const load = useCallback(async (): Promise<void> => {
    try {
      const reply = await client.request({ type: "usage_prices" }, "usage_prices");
      take(reply.prices, reply.prices_as_of);
    } catch (failure) {
      setError(errText(failure));
    }
  }, [client, take]);
  useLoadOnConnect(connected, load);
  return { loaded, error, take };
}

/** Where the prices come from: Flotilla's seeded estimates, or the owner's table. */
function Source({ loaded }: { readonly loaded: Loaded }): ReactElement {
  const seeded = loaded.prices.some((p) => p.estimate === true);
  if (seeded) {
    return (
      <p className="field-hint usage-note">
        These are Flotilla&apos;s own estimates, not your plan&apos;s prices. Set your own to make
        the costs yours.
      </p>
    );
  }
  return (
    <p className="field-hint usage-note">
      Your prices{loaded.asOf === null ? "" : `, set ${fmtTimestamp(loaded.asOf)}`}.
    </p>
  );
}

function ReadOnlyRows({ prices }: { readonly prices: readonly UsagePrice[] }): ReactElement {
  return (
    <tbody>
      {prices.map((p) => (
        <tr key={p.model}>
          <th scope="row">{p.model}</th>
          {KINDS.map((kind) => (
            <td key={kind}>${p[kind]}</td>
          ))}
        </tr>
      ))}
    </tbody>
  );
}

function EditRows(props: {
  readonly rows: readonly Row[];
  readonly disabled: boolean;
  readonly onChange: (key: number, field: "model" | Kind, value: string) => void;
  readonly onRemove: (key: number) => void;
}): ReactElement {
  return (
    <tbody>
      {props.rows.map((row) => {
        const fallback = row.model.trim() === FALLBACK;
        const name = row.model.trim() === "" ? "new model" : row.model.trim();
        return (
          <tr key={row.key}>
            <th scope="row">
              <input
                type="text"
                aria-label="Model"
                value={row.model}
                readOnly={fallback}
                disabled={props.disabled}
                onChange={(e) => {
                  props.onChange(row.key, "model", e.target.value);
                }}
              />
            </th>
            {KINDS.map((kind) => (
              <td key={kind}>
                <input
                  type="text"
                  inputMode="decimal"
                  aria-label={`${name} ${KIND_LABELS[kind].toLowerCase()}`}
                  value={row[kind]}
                  disabled={props.disabled}
                  onChange={(e) => {
                    props.onChange(row.key, kind, e.target.value);
                  }}
                />
              </td>
            ))}
            <td>
              {fallback ? null : (
                <button
                  type="button"
                  className="btn btn-small"
                  aria-label={`Remove ${name}`}
                  disabled={props.disabled}
                  onClick={() => {
                    props.onRemove(row.key);
                  }}
                >
                  Remove
                </button>
              )}
            </td>
          </tr>
        );
      })}
    </tbody>
  );
}

function Head({ editing }: { readonly editing: boolean }): ReactElement {
  return (
    <thead>
      <tr>
        <th scope="col">Model</th>
        {KINDS.map((kind) => (
          <th key={kind} scope="col">
            {KIND_LABELS[kind]}
          </th>
        ))}
        {editing ? (
          <th scope="col">
            <span className="visually-hidden">Remove</span>
          </th>
        ) : null}
      </tr>
    </thead>
  );
}

/** The owner's table: cells to edit, rows to add and remove, one Save. */
function Editor(props: {
  readonly client: DaemonApi;
  readonly connected: boolean;
  readonly loaded: Loaded;
  readonly onTaken: (prices: readonly UsagePrice[], asOf: string | null | undefined) => void;
  readonly onSaved: () => void;
}): ReactElement {
  const saved = rowsOf(props.loaded.prices);
  const [rows, setRows] = useState<readonly Row[]>(saved);
  const [nextKey, setNextKey] = useState(saved.length);
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const result = checked(rows);
  const problem = typeof result === "string" ? result : null;
  const dirty = !same(rows, saved);

  const change = (key: number, field: "model" | Kind, value: string): void => {
    setRows(rows.map((r) => (r.key === key ? { ...r, [field]: value } : r)));
  };
  const add = (): void => {
    const empty = { input: "", output: "", cache_read: "", cache_write: "" };
    setRows([...rows, { key: nextKey, model: "", ...empty }]);
    setNextKey(nextKey + 1);
  };
  const save = async (event: FormEvent): Promise<void> => {
    event.preventDefault();
    if (typeof result === "string" || !dirty) {
      return;
    }
    setSaving(true);
    setFailure(null);
    try {
      const reply = await props.client.request(
        { type: "set_usage_prices", prices: result },
        "usage_prices",
      );
      props.onTaken(reply.prices, reply.prices_as_of);
      props.onSaved();
    } catch (error) {
      setFailure(errText(error));
    } finally {
      setSaving(false);
    }
  };

  const off = !props.connected || saving;
  return (
    <form onSubmit={(e) => void save(e)}>
      <table className="usage-table usage-prices">
        <Head editing />
        <EditRows
          rows={rows}
          disabled={off}
          onChange={change}
          onRemove={(key) => {
            setRows(rows.filter((r) => r.key !== key));
          }}
        />
      </table>
      {dirty && problem !== null ? (
        <p className="field-error" role="alert">
          {problem}
        </p>
      ) : null}
      {failure === null ? null : (
        <p className="field-error" role="alert">
          Couldn&apos;t save the prices: {failure}
        </p>
      )}
      <div className="panel-actions">
        <button type="submit" className="btn btn-primary" disabled={off || !dirty || !!problem}>
          {saving ? "Saving…" : "Save prices"}
        </button>
        <button
          type="button"
          className="btn"
          disabled={off || rows.length >= MAX_ROWS}
          onClick={add}
        >
          Add a model
        </button>
      </div>
    </form>
  );
}

export default function PriceTable(props: PriceTableProps): ReactElement {
  const { client, connected } = props;
  const { loaded, error, take } = usePrices(client, connected);
  const owner = client.hasGrant("approve");
  return (
    <section className="usage-card" aria-labelledby="usage-prices-title">
      <h2 className="usage-card-title" id="usage-prices-title">
        Price table
      </h2>
      <p className="field-hint usage-note">
        US dollars per million tokens. A model takes the row with the longest name it contains, else
        the * row.
      </p>
      {loaded === null ? (
        <p className="usage-dim">
          {error === null ? "Loading prices…" : `Couldn't load: ${error}`}
        </p>
      ) : (
        <>
          <Source loaded={loaded} />
          {owner ? (
            <Editor
              key={JSON.stringify(loaded.prices)}
              client={client}
              connected={connected}
              loaded={loaded}
              onTaken={take}
              onSaved={props.onSaved}
            />
          ) : (
            <>
              <table className="usage-table usage-prices">
                <Head editing={false} />
                <ReadOnlyRows prices={loaded.prices} />
              </table>
              <p className="field-hint usage-note">
                Only the owner changes prices, from their app or a paired phone.
              </p>
            </>
          )}
        </>
      )}
    </section>
  );
}
