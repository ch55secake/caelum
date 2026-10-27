#!/usr/bin/env python3
"""Render Caelum's Homebrew formula and cask into the tap checkout."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from string import Template


ROOT = Path(__file__).resolve().parents[2]
ASSETS = {
    "AARCH64_CLI_SHA256": "caelum-cli-v{version}-aarch64-apple-darwin.tar.gz",
    "X86_64_CLI_SHA256": "caelum-cli-v{version}-x86_64-apple-darwin.tar.gz",
    "AARCH64_DMG_SHA256": "Caelum-v{version}-aarch64.dmg",
    "X86_64_DMG_SHA256": "Caelum-v{version}-x86_64.dmg",
}


def read_checksums(path: Path) -> dict[str, str]:
    checksums: dict[str, str] = {}
    for line in path.read_text().splitlines():
        digest, filename = line.split(maxsplit=1)
        checksums[Path(filename.lstrip("* ")).name] = digest
    return checksums


def render(template: Path, substitutions: dict[str, str]) -> str:
    return Template(template.read_text()).substitute(substitutions)


def update_tap_tests(tap: Path) -> None:
    tests = tap / ".github/workflows/tests.yml"
    content = tests.read_text()
    formula_audit = "        run: brew audit --new --strict --online --tap ch55secake/tap\n"
    if formula_audit not in content:
        raise SystemExit(f"could not find formula audit command in {tests}")
    cask_audit = "brew audit --cask --strict --online caelum"
    if cask_audit not in content:
        content = content.replace(
            formula_audit,
            formula_audit
            + "      - name: Audit Caelum cask\n"
            + f"        run: {cask_audit}\n",
            1,
        )

    style_line = re.search(r"(?m)^(\s*run: brew style .*)$", content)
    if style_line is None:
        raise SystemExit(f"could not find brew style command in {tests}")
    updated_style = style_line.group(1)
    for path in ("Formula/c/caelum-cli.rb", "Casks/caelum.rb"):
        if path not in updated_style:
            updated_style += f" {path}"
    content = content[: style_line.start(1)] + updated_style + content[style_line.end(1) :]
    content = content.replace("Check formula style", "Check formula and cask style", 1)
    tests.write_text(content)


def main() -> None:
    if len(sys.argv) != 4:
        raise SystemExit(f"usage: {sys.argv[0]} VERSION SHA256SUMS TAP_CHECKOUT")

    version, checksum_file, tap_checkout = sys.argv[1:]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise SystemExit(f"invalid release version: {version}")

    checksums = read_checksums(Path(checksum_file))
    substitutions = {"VERSION": version}
    for key, asset_pattern in ASSETS.items():
        asset = asset_pattern.format(version=version)
        digest = checksums.get(asset)
        if digest is None or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise SystemExit(f"missing or invalid SHA-256 for release asset: {asset}")
        substitutions[key] = digest

    tap = Path(tap_checkout)
    formula = tap / "Formula/c/caelum-cli.rb"
    cask = tap / "Casks/caelum.rb"
    formula.parent.mkdir(parents=True, exist_ok=True)
    cask.parent.mkdir(parents=True, exist_ok=True)
    formula.write_text(render(ROOT / "packaging/homebrew/caelum-cli.rb.in", substitutions))
    cask.write_text(render(ROOT / "packaging/homebrew/caelum.rb.in", substitutions))
    update_tap_tests(tap)

    readme = tap / "README.md"
    start = "<!-- BEGIN CAELUM PACKAGES -->"
    end = "<!-- END CAELUM PACKAGES -->"
    section = f"""{start}
## Caelum

Install the desktop app and CLI separately:

```sh
brew install --cask ch55secake/tap/caelum
brew install ch55secake/tap/caelum-cli
```

{end}"""
    readme_text = readme.read_text()
    if start in readme_text and end in readme_text:
        before, remainder = readme_text.split(start, maxsplit=1)
        _, after = remainder.split(end, maxsplit=1)
        readme_text = before.rstrip() + "\n\n" + section + after
    else:
        readme_text = readme_text.rstrip() + "\n\n" + section + "\n"
    readme.write_text(readme_text)

    print(f"Rendered {formula} and {cask} for v{version}")


if __name__ == "__main__":
    main()
