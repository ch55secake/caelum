export function localDate(date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

// Calendar-day buckets include idle days. Bound the chart without dropping usage
// when an all-time query spans years; UTC arithmetic avoids DST-sized days.
export function activityBuckets(activity, range) {
  const day = 86_400_000;
  const first = Date.parse(`${localDate(new Date(range.from))}T00:00:00Z`);
  const last = Date.parse(
    `${localDate(new Date(Math.max(range.from, range.to - 1)))}T00:00:00Z`,
  );
  const days = Math.floor((last - first) / day) + 1;
  if (!Number.isFinite(days) || days < 1) return [];
  const size = Math.max(1, Math.ceil(days / 60));
  const date = (value) => new Date(value).toISOString().slice(0, 10);
  const buckets = Array.from(
    { length: Math.ceil(days / size) },
    (_, index) => ({
      from: date(first + index * size * day),
      to: date(Math.min(last, first + ((index + 1) * size - 1) * day)),
      steps: 0,
    }),
  );
  for (const item of activity) {
    const index = Math.floor(
      (Date.parse(`${item.date}T00:00:00Z`) - first) / day / size,
    );
    if (buckets[index] && item.date <= date(last))
      buckets[index].steps += item.steps;
  }
  return buckets;
}
