import * as esbuild from "esbuild";
import path from "path";
import { fileURLToPath } from "url";
import { dirname } from "path";

// Convert ESM meta URL to file path
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

await esbuild.build({
  entryPoints: ["src/index.ts"],
  bundle: true,
  format: "esm",
  platform: "browser",
  target: ["esnext"],
  outdir: path.resolve(__dirname, "../static/dist"),
  sourcemap: true,
  minify: false,
});
