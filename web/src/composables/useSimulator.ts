import { ref, provide, inject, type Ref } from "vue";
import { createSim, type SimInstance } from "../sim";
import { disasm } from "../disasm";
import { parseListing, parseMap, type LstLine, type MapSymbol } from "../parsers";

export interface WatchEntry {
  addr: number;
  label: string;
  format: "hex" | "dec" | "u16" | "s16" | "f32" | "ascii";
}

export interface TraceEntry {
  ins: number;
  pc: number;
  disasm: string;
  changedRegs: number;
  changedAddrs: number;
}

export const SIM_KEY = Symbol("sim");

export function useSimulator() {
  const sim: Ref<SimInstance | null> = ref(null);
  const ready = ref(false);
  const romLoaded = ref(false);
  const romBase = ref(0);
  const romLen = ref(0);

  const pc = ref(0);
  const insCount = ref(0);
  const psw = ref(0);
  const running = ref(false);
  const stopped = ref(true);
  const stopReason = ref("");
  const halted = ref(false);
  const fault = ref<{ lastPc: number; cause: number } | null>(null);

  const breakpoints = ref<Set<number>>(new Set());
  const watch = ref<WatchEntry[]>([]);
  const trace: Ref<TraceEntry[]> = ref([]);

  // Listing + symbol state
  const lstLines = ref<LstLine[]>([]);
  const lstLoaded = ref(false);
  const symbols = ref<MapSymbol[]>([]);

  const regs = ref<number[]>(new Array(32).fill(0));
  const changedRegs = ref<Set<number>>(new Set());

  const rbaseInput = ref("00000000");
  const autoFollow = ref(true);
  const error = ref<string | null>(null);
  // Bumped on every loadRom/reset so memory panels re-read (their computed
  // caches by dependency; sim identity and pc don't change on load).
  const memVersion = ref(0);
  let errTimer: ReturnType<typeof setTimeout> | null = null;
  function showError(msg: string) {
    error.value = msg;
    if (errTimer) clearTimeout(errTimer);
    errTimer = setTimeout(() => {
      error.value = null;
    }, 8000);
  }

  // --- init ---
  async function init() {
    sim.value = await createSim();
    ready.value = true;
    refreshState();
  }

  function refreshState() {
    const s = sim.value;
    if (!s) return;
    pc.value = s.getPc();
    insCount.value = s.getInsCount();
    psw.value = s.getPsw();
    running.value = false;
    stopped.value = s.isStopped();
    stopReason.value = s.getStopReason();
    halted.value = s.isHalted();
    for (let i = 0; i < 32; i++) regs.value[i] = s.getReg(i);
    if (s.getLastFault) {
      const r = s.getLastFault();
      if (r[1] !== 0) {
        fault.value = { lastPc: r[0], cause: r[1] };
      } else {
        fault.value = null;
      }
    }
  }

  // --- ROM loading ---
  function loadRom(data: Uint8Array, base: number) {
    const s = sim.value;
    if (!s) return;
    s.loadRom(data, base);
    romBase.value = base;
    romLen.value = data.length;
    romLoaded.value = true;
    memVersion.value++;
    doReset();
  }

  // --- control ---
  function doReset() {
    const s = sim.value;
    if (!s) return;
    s.reset();
    trace.value = [];
    fault.value = null;
    changedRegs.value = new Set();
    memVersion.value++;
    refreshState();
  }

  function applyRbase() {
    const s = sim.value;
    if (!s) return;
    const v = parseInt(rbaseInput.value, 16) >>> 0;
    s.setRbase(v);
    s.setIntbp(v + 0x100);
    doReset();
  }

  function doStep(): boolean {
    const s = sim.value;
    if (!s) return false;
    // Already parked (breakpoint / HALT / watchpoint): report why, don't pretend
    // to have stepped.
    if (s.isStopped()) {
      const reason = s.getStopReason();
      if (s.isHalted()) {
        showError(`CPU is HALTED (parked at PC=0x${s.getPc().toString(16).padStart(8, "0")}) — no stepping possible`);
      } else if (reason) {
        showError(`CPU is stopped (${reason}) — clear/remove the breakpoint or reset to continue stepping`);
      }
      return false;
    }
    const prevPc = s.getPc();
    const prevIns = s.getInsCount();
    const prevRegs = regs.value.slice();
    const stoppedNow = s.stepOnce();
    if (stoppedNow) {
      // Legitimate stop: breakpoint / HALT / watchpoint / fault handler.
      refreshState();
      const changed = new Set<number>();
      for (let i = 0; i < 32; i++) {
        if (regs.value[i] !== prevRegs[i]) changed.add(i);
      }
      changedRegs.value = changed;
      trace.value.push({
        ins: s.getInsCount(),
        pc: prevPc,
        disasm: "",
        changedRegs: changed.size,
        changedAddrs: 0,
      });
      if (trace.value.length > 200) trace.value.shift();
      return true;
    }
    // Not stopped after step: verify the instruction actually advanced.
    const newPc = s.getPc();
    if (newPc === prevPc) {
      refreshState();
      showError(`Step failed at PC=0x${prevPc.toString(16).padStart(8, "0")} — PC did not advance (unknown/reserved instruction or missing RIE vector)`);
      return false;
    }
    refreshState();
    const changed = new Set<number>();
    for (let i = 0; i < 32; i++) {
      if (regs.value[i] !== prevRegs[i]) changed.add(i);
    }
    changedRegs.value = changed;
    const writeLog = s.takeWriteLog();
    const changedAddrs = writeLog.length > 0 ? writeLog.length / 2 : 0;
    // Decode the executed instruction for the trace
    let disasmText = "";
    try {
      const buf = new Uint8Array(4);
      for (let i = 0; i < 4; i++) buf[i] = s.getMemByte((prevPc + i) >>> 0) & 0xff;
      disasmText = disasm(buf, 0).text;
    } catch {
      disasmText = "??";
    }
    trace.value.push({
      ins: prevIns + 1,
      pc: prevPc,
      disasm: disasmText,
      changedRegs: changed.size,
      changedAddrs,
    });
    if (trace.value.length > 200) trace.value.shift();
    return true;
  }

  function doRun(maxIns = 10_000_000) {
    const s = sim.value;
    if (!s || running.value) return;
    running.value = true;
    const CHUNK = 100_000;
    const step = () => {
      if (!running.value) return;
      s.runTo(CHUNK);
      refreshState();
      if (stopped.value || insCount.value >= maxIns) {
        running.value = false;
        return;
      }
      requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  function doStop() {
    running.value = false;
    refreshState();
  }

  function doStepOver() {
    const s = sim.value;
    if (!s) return;
    // Read the first halfword at PC
    const h0 = s.getMemWord(pc.value) & 0xffff;
    const op6 = (h0 >> 5) & 0x3f;
    const reg2 = (h0 >> 11) & 0x1f;

    // JARL disp22: op6=0b011110(30), reg2≠0 → PC+4
    // JR disp22:    op6=0b011110(30), reg2==0 → PC+4 (not a call, but still jumps)
    // 16-bit call op6=0b000011(3) → PC+2
    const isCall = op6 === 30 && reg2 !== 0;
    const isJump = op6 === 30 && reg2 === 0;
    const is16bitCall = op6 === 3;

    if (isCall) {
      // JARL disp22: step until PC returns to PC+4
      const retAddr = (pc.value + 4) >>> 0;
      for (let i = 0; i < 10000; i++) {
        s.stepOnce();
        if (s.isStopped()) break;
        if (s.getPc() === retAddr) break;
      }
    } else if (isJump) {
      // JR: just step once (it's a jump, not a call)
      doStep();
    } else if (is16bitCall) {
      // 16-bit call: step until PC returns to PC+2
      const retAddr = (pc.value + 2) >>> 0;
      for (let i = 0; i < 10000; i++) {
        s.stepOnce();
        if (s.isStopped()) break;
        if (s.getPc() === retAddr) break;
      }
    } else {
      // Not a call/jump: just step once
      doStep();
    }
    refreshState();
  }

  function seek(addr: number) {
    const s = sim.value;
    if (!s) return;
    s.setPc(addr >>> 0);
    refreshState();
  }

  // --- breakpoints ---
  function toggleBp(addr: number) {
    const s = sim.value;
    if (!s) return;
    const a = addr >>> 0;
    if (breakpoints.value.has(a)) {
      breakpoints.value.delete(a);
      s.removeBreakpoint(a);
    } else {
      breakpoints.value.add(a);
      s.addBreakpoint(a);
    }
    // In-place mutation: Vue 3 tracks Set add/delete per-key,
    // so only the affected line re-renders (no full re-key of 60 lines).
  }

  function clearBps() {
    const s = sim.value;
    if (!s) return;
    breakpoints.value = new Set();
    s.clearBreakpoints();
  }

  // --- watch ---
  function addWatch(addr: number, label?: string) {
    watch.value.push({
      addr: addr >>> 0,
      label: label || `0x${(addr >>> 0).toString(16).padStart(8, "0")}`,
      format: "hex",
    });
  }

  function removeWatch(idx: number) {
    watch.value.splice(idx, 1);
  }

  // --- listing / symbols ---
  function loadListing(text: string) {
    lstLines.value = parseListing(text);
    lstLoaded.value = lstLines.value.length > 0;
    memVersion.value++;
  }

  function loadMap(text: string) {
    symbols.value = parseMap(text);
    memVersion.value++;
  }

  function symbolAt(addr: number): MapSymbol | null {
    // Find the symbol whose range contains addr
    for (const s of symbols.value) {
      if (s.size > 0 && addr >= s.addr && addr < s.addr + s.size) return s;
    }
    // Fallback: exact address match
    for (const s of symbols.value) {
      if (addr === s.addr) return s;
    }
    return null;
  }

  function watchSymbol(name: string, addr: number) {
    addWatch(addr, name);
  }

  // --- helpers ---
  function fmtAddr(a: number): string {
    return a.toString(16).padStart(8, "0");
  }

  function pswFlags(): { name: string; bit: number; set: boolean }[] {
    const p = psw.value;
    return [
      { name: "UM", bit: 31, set: ((p >> 31) & 1) === 1 },
      { name: "CU2", bit: 18, set: ((p >> 18) & 1) === 1 },
      { name: "CU1", bit: 17, set: ((p >> 17) & 1) === 1 },
      { name: "CU0", bit: 16, set: ((p >> 16) & 1) === 1 },
      { name: "EBV", bit: 15, set: ((p >> 15) & 1) === 1 },
      { name: "SAT", bit: 11, set: ((p >> 11) & 1) === 1 },
      { name: "CY", bit: 10, set: ((p >> 10) & 1) === 1 },
      { name: "NZ", bit: 9, set: ((p >> 9) & 1) === 1 },
      { name: "S", bit: 8, set: ((p >> 8) & 1) === 1 },
      { name: "V", bit: 7, set: ((p >> 7) & 1) === 1 },
    ];
  }

  const api = {
    sim,
    ready,
    romLoaded,
    romBase,
    romLen,
    pc,
    insCount,
    psw,
    running,
    stopped,
    stopReason,
    halted,
    fault,
    breakpoints,
    watch,
    trace,
    lstLines,
    lstLoaded,
    symbols,
    regs,
    changedRegs,
    rbaseInput,
    autoFollow,
    error,
    memVersion,
    init,
    loadRom,
    doReset,
    applyRbase,
    doStep,
    doRun,
    doStop,
    doStepOver,
    seek,
    toggleBp,
    clearBps,
    addWatch,
    removeWatch,
    loadListing,
    loadMap,
    symbolAt,
    watchSymbol,
    fmtAddr,
    pswFlags,
    refreshState,
  };

  // Provide to child components
  provide(SIM_KEY, api);

  return api;
}

export type Simulator = ReturnType<typeof useSimulator>;

// Inject the shared simulator instance in child components
export function injectSim(): Simulator {
  const sim = inject(SIM_KEY);
  if (!sim) throw new Error("Simulator not initialized in parent");
  return sim as Simulator;
}
