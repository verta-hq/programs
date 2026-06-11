const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const idlPath = path.join(root, "target", "idl", "verta_core.json");
const outputPath = path.resolve(
  root,
  "..",
  "mobile",
  "lib",
  "core",
  "solana",
  "verta_core_idl.g.dart"
);

function lowerCamel(name) {
  return name.replace(/_([a-z])/g, (_, char) => char.toUpperCase());
}

const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));
const instructions = idl.instructions
  .map((instruction) => ({
    name: lowerCamel(instruction.name),
    discriminator: instruction.discriminator,
  }))
  .sort((a, b) => a.name.localeCompare(b.name));

const lines = [
  "// GENERATED CODE - DO NOT MODIFY BY HAND.",
  "// Run `yarn generate:mobile-idl` from programs/ after `anchor build`.",
  "",
  "class VertaCoreInstructions {",
  "  const VertaCoreInstructions._();",
  "",
];

for (const instruction of instructions) {
  lines.push(
    `  static const List<int> ${
      instruction.name
    } = [${instruction.discriminator.join(", ")}];`
  );
}

lines.push("}", "");

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, lines.join("\n"));
console.log(`Generated ${path.relative(root, outputPath)}`);
