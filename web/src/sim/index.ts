// WASM simulator interface for the Vue UI.
// The wasm-bindgen artifacts (sim.js / sim.d.ts / sim_bg.wasm) live in this
// directory, produced by sim/build-wasm.sh.

export interface SimInstance {
  loadRom(data: Uint8Array, base: number): void;
  setRbase(v: number): void;
  setIntbp(v: number): void;
  reset(): void;
  stepOnce(): boolean;
  runTo(maxIns: number): number;
  addBreakpoint(addr: number): void;
  removeBreakpoint(addr: number): void;
  clearBreakpoints(): void;
  resume(): void;
  isHalted(): boolean;
  addWatchpoint(addr: number, size: number): void;
  removeWatchpoint(addr: number): void;
  clearWatchpoints(): void;
  getWatchpoints(): number[];
  checkWatchpoints(): boolean;
  getReg(n: number): number;
  setReg(n: number, v: number): void;
  getPc(): number;
  setPc(v: number): void;
  getPsw(): number;
  setPsw(v: number): void;
  getFpReg(n: number): number;
  getFpRegHi(n: number): number;
  setFpReg(n: number, lo: number, hi: number): void;
  getInsCount(): number;
  isStopped(): boolean;
  getStopReason(): string;
  getMemByte(addr: number): number;
  getMemWord(addr: number): number;
  readWord(addr: number): number;
  setMemWord(addr: number, v: number): void;
  getRbase(): number;
  getEipc(): number;
  getFepc(): number;
  getRb(): number;
  getRamSize(): number;
  readSlice(addr: number, len: number): Uint8Array;
  writeSlice(addr: number, data: Uint8Array): void;
  takeWriteLog(): Uint32Array;
  restoreWriteLog(log: Uint32Array): void;
  getFpsr(): number;
  getEiic(): number;
  getFeic(): number;
  getLastFault?(): number[];
}

// The WASM artifacts are committed alongside the source tree so Vite can
// resolve them directly (the prebuilt app ships without a Rust toolchain).
// @ts-ignore -- sim.js is a wasm-bindgen generated file; types come from sim.d.ts
import initSim, { Sim as SimClass } from "./sim.js";

let simPromise: Promise<SimInstance> | null = null;

export async function createSim(ramSize = 0x400000): Promise<SimInstance> {
  if (!simPromise) {
    simPromise = (async () => {
      await initSim();
      return new SimClass(ramSize) as unknown as SimInstance;
    })();
  }
  return simPromise;
}
