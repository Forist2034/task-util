import fs from "node:fs";
import path from "node:path";
import esbuild from "esbuild";
import url from "node:url";

const root = path.join(import.meta.dirname, "..");
const dest = path.join(root, "dist");

async function build() {
  if (fs.existsSync(dest)) {
    fs.rmSync(dest, { recursive: true });
  }
  fs.mkdirSync(dest);

  await esbuild.build({
    entryPoints: [path.join(root, "src/plugin.ts")],
    bundle: true,
    outfile: path.join(dest, "plugin.js"),
    platform: "node",
    target: "es2020",
    format: "cjs",
    minify: false,
  });
  fs.copyFileSync(
    path.join(root, "manifest.json"),
    path.join(dest, "manifest.json"),
  );

  console.log("build complete");
}

build().catch((err) => {
  console.error("Build failed:", err);
  process.exit(1);
});
