export type Tokens = {
  input: number;
  output: number;
  reasoning: number;
  cache: { read: number; write: number };
};
export type Stats = {
  range: { from: number; to: number };
  sessions: number;
  subagents: number;
  prompts: number;
  steps: number;
  tokens: Tokens;
  cost: number;
  activeDays: number;
  streak: number;
  activity: { date: string; steps: number }[];
  models: {
    model: { id: string; providerID: string; variant?: string };
    steps: number;
    tokens: Tokens;
    cost: number;
  }[];
};
export function localDate(date: Date): string;
export function statsCount(value: number): string;
export function activityBuckets(
  activity: Stats["activity"],
  range: Stats["range"],
): { from: string; to: string; steps: number }[];
