// Node smoke test: load the built WASM sim and run the test ROM.
// Validates the wasm artifact (the same file the browser loads from web/src/sim/).
// Usage: node testdata/smoke.mjs   (no args needed; paths resolved from this file)
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

const wasmBytes = readFileSync(join(root, "web/src/sim/sim_bg.wasm"));
const res = new Response(wasmBytes, { headers: { "Content-Type": "application/wasm" } });
const mod = await import(join(root, "web/src/sim/sim.js"));
await mod.default(res);
const { Sim } = mod;

const rom = readFileSync(join(root, "testdata/test.rom"));
const s = new Sim(0x400000);
s.loadRom(new Uint8Array(rom), 0x00000000);
s.setRbase(0x00000000);
s.reset();

console.log("after reset: PC =", (s.getPc() >>> 0).toString(16).padStart(8, "0"));

// Run to HALT (the test ROM covers all C syntax and ends with HALT).
const MAX = 100000;
for (let i = 0; i < MAX; i++) {
  if (s.stepOnce()) break;
}
console.log(`after run: INS=${s.getInsCount()} PC=0x${(s.getPc() >>> 0).toString(16).padStart(8, "0")}`);
console.log(`halted=${s.isHalted()} stopReason=${s.getStopReason()}`);

// Key global results (see testdata/mkrom.py for the C program semantics).
const words = [0x1000, 0x1004, 0x1008, 0x100c, 0x1010];
for (const a of words) {
  console.log(`mem[0x${a.toString(16).padStart(4, "0")}] = 0x${(s.getMemWord(a) >>> 0).toString(16).padStart(8, "0")}`);
}

if (s.isHalted() && s.getStopReason() === "halt") {
  console.log("SMOKE OK");
} else {
  console.error("SMOKE FAILED: expected HALT");
  process.exit(1);
}
