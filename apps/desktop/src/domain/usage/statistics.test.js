import assert from "node:assert/strict";
import test from "node:test";
import { activityBuckets, localDate, statsCount } from "./statistics.js";

test("large statistics counts use M and B without abbreviating thousands", () => {
  for (const value of [0, 42, 1000, 999999])
    assert.equal(statsCount(value), value.toLocaleString());
  assert.equal(statsCount(1_000_000), "1M");
  assert.equal(statsCount(1_234_567), `${(1.2).toLocaleString()}M`);
  assert.equal(statsCount(999_940_000), `${(999.9).toLocaleString()}M`);
  assert.equal(statsCount(999_999_999), "1B");
  assert.equal(statsCount(1_000_000_000), "1B");
  assert.equal(statsCount(2_450_000_000), `${(2.5).toLocaleString()}B`);
});

test("activity includes idle calendar days and keeps local date bounds through DST", () => {
  const range = {
    from: new Date(2026, 10, 1).getTime(),
    to: new Date(2026, 10, 4).getTime(),
  };
  assert.equal(localDate(new Date(range.from)), "2026-11-01");
  assert.deepEqual(
    activityBuckets(
      [
        { date: "2026-11-01", steps: 3 },
        { date: "2026-11-03", steps: 7 },
        { date: "2026-11-04", steps: 99 },
      ],
      range,
    ),
    [
      { from: "2026-11-01", to: "2026-11-01", steps: 3 },
      { from: "2026-11-02", to: "2026-11-02", steps: 0 },
      { from: "2026-11-03", to: "2026-11-03", steps: 7 },
    ],
  );
});

test("multi-year activity is bounded without losing model steps", () => {
  const activity = [
    { date: "2020-01-01", steps: 1 },
    { date: "2023-07-18", steps: 12 },
    { date: "2026-12-31", steps: 5 },
  ];
  const buckets = activityBuckets(activity, {
    from: new Date(2020, 0, 1).getTime(),
    to: new Date(2027, 0, 1).getTime(),
  });
  assert.ok(buckets.length <= 60);
  assert.equal(
    buckets.reduce((sum, item) => sum + item.steps, 0),
    18,
  );
  assert.equal(buckets[0].from, "2020-01-01");
  assert.equal(buckets.at(-1).to, "2026-12-31");
});
