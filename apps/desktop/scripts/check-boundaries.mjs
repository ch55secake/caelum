import { readdirSync, readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = fileURLToPath(new URL("../src/", import.meta.url));
const allowed = {
  app: ["app", "ui", "application", "domain", "platform"],
  ui: ["ui", "application", "domain", "platform"],
  application: ["application", "domain", "platform"],
  platform: ["platform", "domain"],
  domain: ["domain"],
};
function* files(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) yield* files(path);
    else if (/\.(js|ts|tsx)$/.test(entry.name)) yield path;
  }
}
const errors = [];
for (const path of files(root)) {
  const source = relative(root, path);
  const layer = source.split("/")[0];
  if (!allowed[layer]) continue;
  const file = ts.createSourceFile(
    path,
    readFileSync(path, "utf8"),
    ts.ScriptTarget.Latest,
    true,
  );
  function visit(node) {
    const specifier =
      ts.isImportDeclaration(node) || ts.isExportDeclaration(node)
        ? node.moduleSpecifier
        : ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)
          ? node.argument.literal
          : ts.isCallExpression(node) &&
              node.expression.kind === ts.SyntaxKind.ImportKeyword
            ? node.arguments[0]
            : undefined;
    if (specifier && ts.isStringLiteral(specifier)) {
      const target = specifier.text;
      if (target.startsWith(".")) {
        const dependency = relative(root, resolve(dirname(path), target)).split(
          "/",
        )[0];
        if (!allowed[layer].includes(dependency))
          errors.push(`${source}: ${layer} cannot import ${target}`);
      } else if (
        layer === "domain" &&
        !(source.endsWith(".test.js") && target.startsWith("node:"))
      ) {
        errors.push(`${source}: domain code cannot import ${target}`);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(file);
}
if (errors.length) throw new Error(errors.join("\n"));
console.log("Desktop dependency boundaries passed");
