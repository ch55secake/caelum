import { readdirSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const tests = readdirSync("src", { recursive: true })
  .filter((file) => file.endsWith(".test.js"))
  .sort()
  .map((file) => join("src", file));
if (!tests.length) throw new Error("No frontend unit tests found");
const result = spawnSync(process.execPath, ["--test", ...tests], {
  stdio: "inherit",
});
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
