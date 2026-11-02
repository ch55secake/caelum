import { Fragment, useState } from "react";
import { BarChart3, ChevronRight } from "lucide-react";
import {
  activityBuckets,
  statsCount as count,
  type Stats,
  type Tokens,
} from "../../domain/usage/statistics.js";

const Count = ({ value }: { value: number }) => (
  <span title={value.toLocaleString()}>{count(value)}</span>
);
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
          <dd>
            <Count value={value} />
          </dd>
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
          <Count value={data.activeDays} /> active days ·{" "}
          <Count value={data.streak} /> day streak
        </span>
      </div>
      <div
        className="stats-chart-caption"
        aria-live="polite"
        title={`${(selected?.steps ?? data.steps).toLocaleString()} model steps`}
      >
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
          const label = `${item.from}${item.to !== item.from ? ` through ${item.to}` : ""}: ${item.steps.toLocaleString()} model steps`;
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
        {(
          [
            [
              "Total tokens",
              <Count value={total(data.tokens)} />,
              "Including cached tokens",
            ],
            ["Recorded cost (USD)", money(data.cost), "Reported by OpenCode"],
            [
              "Sessions",
              <Count value={data.sessions} />,
              <><Count value={data.subagents} /> subagent sessions</>,
            ],
            [
              "Prompts",
              <Count value={data.prompts} />,
              <><Count value={data.steps} /> model steps</>,
            ],
          ] as const
        ).map(([label, value, note]) => (
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
          <h2>No recorded model usage.</h2>
          <p>Choose another project or run a session to see its usage.</p>
        </div>
      ) : (
        <>
          <ActivityChart data={data} />
          <section className="stats-section" aria-label="Usage by model">
            <div className="stats-section-heading">
              <h2>Models</h2>
              <span>
                <Count value={data.models.length} /> used
              </span>
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
                          <td>
                            <Count value={item.steps} />
                          </td>
                          <td>
                            <Count value={total(item.tokens)} />
                          </td>
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
