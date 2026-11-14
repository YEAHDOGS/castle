// End-to-end check of the MCP server: lists tools, calls status, and (with --generate) renders a test image.
//   node smoke.mjs [--generate] [--edit <image>]
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const args = process.argv.slice(2);
const client = new Client({ name: "smoke", version: "0" });
await client.connect(new StdioClientTransport({ command: process.execPath, args: ["server.mjs"], stderr: "inherit" }));

const show = (r) => console.log(r.isError ? "ERROR:" : "ok:", r.content.map((c) => c.text).join("\n"));
console.log("tools:", (await client.listTools()).tools.map((t) => t.name).join(", "));
show(await client.callTool({ name: "status", arguments: {} }));

if (args.includes("--generate")) {
  show(await client.callTool({ name: "generate_image", arguments: {
    prompt: "Studio photograph of a golden retriever puppy wearing a tiny red bandana, sitting on a wooden floor, soft window light from frame-left, shallow background of a cream wall.",
    width: 768, height: 768, steps: 8, seed: 42, filename_prefix: "smoke_t2i",
  } }, undefined, { timeout: 900000 }));
}
const e = args.indexOf("--edit");
if (e >= 0) {
  show(await client.callTool({ name: "edit_image", arguments: {
    prompt: "Change the bandana on the puppy in <ref_image_0> to a blue bandana with white polka dots. Keep everything else unchanged.",
    images: [args[e + 1]], steps: 8, seed: 7, filename_prefix: "smoke_edit",
  } }, undefined, { timeout: 900000 }));
}
await client.close();
