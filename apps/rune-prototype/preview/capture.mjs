import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const manifest = resolve(root, "apps/rune-prototype/Cargo.toml");
const output = resolve(process.argv[2] ?? resolve(root, "apps/desktop/preview-results"));
mkdirSync(output, { recursive: true });

for (const theme of ["dark", "light"]) {
  for (const scene of ["", "inspector", "compact"]) {
    const name = `rune-${theme}${scene ? `-${scene}` : ""}`;
    execFileSync("cargo", [
      "run", "--locked", "--manifest-path", manifest, "--",
      "--theme", theme, "--scale", "1", "--at", "1",
      "--size", scene === "compact" ? "800x620" : "1280x1000",
      ...(scene ? ["--scene", scene] : []),
      "--snap", resolve(output, `${name}.png`),
      "--tree", resolve(output, `${name}.json`),
    ], { stdio: "inherit" });
  }
}
