import { Fragment, useState } from "react";
import { BarChart3, ChevronRight } from "lucide-react";
import {
  activityBuckets,
  type Stats,
  type Tokens,
} from "../../domain/usage/statistics.js";

const count = (value: number) => value.toLocaleString();
const total = (tokens: Tokens) =>
  tokens.input +
  tokens.output +
  tokens.reasoning +
  tokens.cache.read +
  tokens.cache.write;
const money = (value: number) =>
  value.toLocaleString(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 4,
  });
const dayLabel = (date: string) =>
  new Date(`${date}T00:00:00`).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });

function TokenBreakdown({ tokens }: { tokens: Tokens }) {
  return (
    <dl className="stats-tokens">
      {(
        [
          ["Input", tokens.input],
          ["Output", tokens.output],
          ["Reasoning", tokens.reasoning],
          ["Cache read", tokens.cache.read],
          ["Cache write", tokens.cache.write],
        ] as const
      ).map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{count(value)}</dd>
        </div>
      ))}
    </dl>
  );
}

function ActivityChart({ data }: { data: Stats }) {
  const buckets = activityBuckets(data.activity, data.range);
  const [active, setActive] = useState<number | null>(null);
  const selected = active === null ? null : buckets[active];
  const max = Math.max(1, ...buckets.map((item) => item.steps));
  const width = 600 / Math.max(1, buckets.length);
  return (
    <section className="stats-section" aria-label="Daily activity">
      <div className="stats-section-heading">
        <h2>Activity</h2>
        <span>
          {count(data.activeDays)} active days · {count(data.streak)} day streak
        </span>
      </div>
      <div className="stats-chart-caption" aria-live="polite">
        {selected
          ? `${dayLabel(selected.from)}${selected.to !== selected.from ? ` – ${dayLabel(selected.to)}` : ""} · ${count(selected.steps)} steps`
          : `${count(data.steps)} model steps`}
      </div>
      <svg
        className="stats-chart"
        viewBox="0 0 600 100"
        preserveAspectRatio="none"
        role="group"
        aria-label="Model activity chart"
        onMouseLeave={() => setActive(null)}
      >
        {[0, 45, 90].map((y) => (
          <line key={y} x1="0" y1={y + 1} x2="600" y2={y + 1} />
        ))}
        {buckets.map((item, index) => {
          const height = Math.max(2, (item.steps / max) * 88);
          const label = `${item.from}${item.to !== item.from ? ` through ${item.to}` : ""}: ${count(item.steps)} model steps`;
          return (
            <rect
              key={item.from}
              x={index * width + 1}
              y={92 - height}
              width={Math.max(1, width - 2)}
              height={height}
              rx="2"
              role="img"
              aria-label={label}
              tabIndex={0}
              data-active={active === index}
              data-empty={!item.steps}
              onMouseEnter={() => setActive(index)}
              onFocus={() => setActive(index)}
              onBlur={() => setActive(null)}
            >
              <title>{label}</title>
            </rect>
          );
        })}
      </svg>
      <div className="stats-chart-axis">
        <span>{buckets[0] && dayLabel(buckets[0].from)}</span>
        <span>{buckets.at(-1) && dayLabel(buckets.at(-1)!.to)}</span>
      </div>
    </section>
  );
}

export function StatsOverview({ data }: { data: Stats }) {
  const [expanded, setExpanded] = useState<string | null>(null);
  return (
    <>
      <dl className="stats-summary" aria-label="Usage totals">
        {[
          [
            "Total tokens",
            count(total(data.tokens)),
            "Including cached tokens",
          ],
          ["Recorded cost (USD)", money(data.cost), "Reported by OpenCode"],
          [
            "Sessions",
            count(data.sessions),
            `${count(data.subagents)} subagent sessions`,
          ],
          ["Prompts", count(data.prompts), `${count(data.steps)} model steps`],
        ].map(([label, value, note]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
            <small>{note}</small>
          </div>
        ))}
      </dl>
      <details className="stats-token-details">
        <summary>Token breakdown</summary>
        <TokenBreakdown tokens={data.tokens} />
      </details>
      {!data.steps ? (
        <div className="stats-empty">
          <BarChart3 size={28} />
          <h2>No recorded model usage in this range.</h2>
          <p>Try a wider date range or another project.</p>
        </div>
      ) : (
        <>
          <ActivityChart data={data} />
          <section className="stats-section" aria-label="Usage by model">
            <div className="stats-section-heading">
              <h2>Models</h2>
              <span>{data.models.length} used</span>
            </div>
            <div className="stats-table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Model / provider</th>
                    <th>Steps</th>
                    <th>Tokens</th>
                    <th>Cost (USD)</th>
                  </tr>
                </thead>
                <tbody>
                  {data.models.map((item) => {
                    const key = JSON.stringify(item.model);
                    const open = expanded === key;
                    return (
                      <Fragment key={key}>
                        <tr>
                          <th scope="row">
                            <button
                              type="button"
                              className="stats-model"
                              aria-expanded={open}
                              aria-label={`Token details for ${item.model.id} · ${item.model.providerID}${item.model.variant ? ` · ${item.model.variant}` : ""}`}
                              onClick={() => setExpanded(open ? null : key)}
                            >
                              <ChevronRight
                                size={13}
                                className={open ? "expanded" : undefined}
                              />
                              <span>
                                {item.model.id}
                                <small>
                                  {item.model.providerID}
                                  {item.model.variant
                                    ? ` · ${item.model.variant}`
                                    : ""}
                                </small>
                              </span>
                            </button>
                          </th>
                          <td>{count(item.steps)}</td>
                          <td>{count(total(item.tokens))}</td>
                          <td>{money(item.cost)}</td>
                        </tr>
                        {open && (
                          <tr className="stats-model-tokens">
                            <td colSpan={4}>
                              <TokenBreakdown tokens={item.tokens} />
                            </td>
                          </tr>
                        )}
                      </Fragment>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>
        </>
      )}
      <p className="usage-caption stats-note">
        Tokens include input, output, reasoning and cache. Recorded cost may be
        zero for subscription models or missing pricing.
      </p>
    </>
  );
}
