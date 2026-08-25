/* tslint:disable */
/* eslint-disable */

/**
 * A JS-side handle to a running G3 simulation.
 */
export class Sim {
    free(): void;
    [Symbol.dispose](): void;
    addBreakpoint(addr: number): void;
    /**
     * Add a watchpoint: stop when a RAM write touches `addr`..`addr+size-1`.
     * size must be 1, 2 or 4; it is rounded down to the nearest power of two.
     */
    addWatchpoint(addr: number, size: number): void;
    /**
     * Check data watchpoints after a manual memory write (Watch entry edit).
     * Returns true if the write hit a watchpoint (CPU now stopped).
     */
    checkWatchpoints(): boolean;
    clearBreakpoints(): void;
    clearWatchpoints(): void;
    getEiic(): number;
    getEipc(): number;
    getFeic(): number;
    getFepc(): number;
    getFpReg(n: number): number;
    getFpRegHi(n: number): number;
    getFpsr(): number;
    getInsCount(): number;
    /**
     * Report the most recent execution fault, if the current PC is outside the
     * loaded ROM (i.e. the CPU fell off the end of code into a vector/0xFF area
     * and is now taking RIEs there).
     * Returns [lastRomPc, 0x60, 0] — lastRomPc = the last PC inside ROM, the
     * instruction at which execution effectively ran off (typically a missing
     * halt at end-of-loop, or an unknown instruction that raised RIE whose
     * vector is not in the loaded image). cause = 0 when PC is inside ROM.
     */
    getLastFault(): Uint32Array;
    getMemByte(addr: number): number;
    getMemWord(addr: number): number;
    getPc(): number;
    getPsw(): number;
    getRamSize(): number;
    getRb(): number;
    getRbase(): number;
    getReg(n: number): number;
    getStopReason(): string;
    getWatchpoints(): Uint32Array;
    /**
     * Read-only: true if the CPU is parked in the HALT state.
     */
    isHalted(): boolean;
    isStopped(): boolean;
    /**
     * Load the program ROM at a base address.
     */
    loadRom(data: Uint8Array, base: number): void;
    constructor(ram_size: number);
    /**
     * Read a RAM/ROM slice into a returned Uint8Array.
     */
    readSlice(addr: number, len: number): Uint8Array;
    /**
     * Read a 32-bit word from memory (ROM or RAM).
     */
    readWord(addr: number): number;
    removeBreakpoint(addr: number): void;
    removeWatchpoint(addr: number): void;
    /**
     * Reset the CPU.
     */
    reset(): void;
    /**
     * Restore RAM from a flat write log [addr0, oldByte0, ...] (undo).
     */
    restoreWriteLog(log: Uint32Array): void;
    /**
     * Resume execution from a HALT-state stop (or any stop).
     * A hardware HALT parks the core until released — this models the release
     * request (e.g. an external event). No-op when not halted.
     */
    resume(): void;
    /**
     * Run until a breakpoint, halt, watchpoint, or `max_ins` instructions.
     */
    runTo(max_ins: number): number;
    /**
     * Set an FP register (for snapshot restore). lo/hi are the low/high 32 bits.
     */
    setFpReg(n: number, lo: number, hi: number): void;
    setIntbp(v: number): void;
    setMemWord(addr: number, v: number): void;
    setPc(v: number): void;
    /**
     * Set PSW directly (for snapshot restore / stepping back).
     */
    setPsw(v: number): void;
    /**
     * Set the reset base address (RBASE). PC after reset = RBASE & ~1.
     */
    setRbase(v: number): void;
    setReg(n: number, v: number): void;
    /**
     * Execute one instruction. Returns true if the CPU is now stopped.
     */
    stepOnce(): boolean;
    /**
     * Drain the RAM write log since the last call: returns flat
     * [addr0, oldByte0, addr1, oldByte1, ...].
     */
    takeWriteLog(): Uint32Array;
    writeSlice(addr: number, data: Uint8Array): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_sim_free: (a: number, b: number) => void;
    readonly sim_addBreakpoint: (a: number, b: number) => void;
    readonly sim_addWatchpoint: (a: number, b: number, c: number) => void;
    readonly sim_checkWatchpoints: (a: number) => number;
    readonly sim_clearBreakpoints: (a: number) => void;
    readonly sim_clearWatchpoints: (a: number) => void;
    readonly sim_getEiic: (a: number) => number;
    readonly sim_getEipc: (a: number) => number;
    readonly sim_getFeic: (a: number) => number;
    readonly sim_getFepc: (a: number) => number;
    readonly sim_getFpReg: (a: number, b: number) => number;
    readonly sim_getFpRegHi: (a: number, b: number) => number;
    readonly sim_getFpsr: (a: number) => number;
    readonly sim_getInsCount: (a: number) => number;
    readonly sim_getLastFault: (a: number) => [number, number];
    readonly sim_getMemByte: (a: number, b: number) => number;
    readonly sim_getMemWord: (a: number, b: number) => number;
    readonly sim_getPc: (a: number) => number;
    readonly sim_getPsw: (a: number) => number;
    readonly sim_getRamSize: (a: number) => number;
    readonly sim_getRb: (a: number) => number;
    readonly sim_getReg: (a: number, b: number) => number;
    readonly sim_getStopReason: (a: number) => [number, number];
    readonly sim_getWatchpoints: (a: number) => [number, number];
    readonly sim_isHalted: (a: number) => number;
    readonly sim_isStopped: (a: number) => number;
    readonly sim_loadRom: (a: number, b: number, c: number, d: number) => void;
    readonly sim_new: (a: number) => number;
    readonly sim_readSlice: (a: number, b: number, c: number) => [number, number];
    readonly sim_readWord: (a: number, b: number) => number;
    readonly sim_removeBreakpoint: (a: number, b: number) => void;
    readonly sim_removeWatchpoint: (a: number, b: number) => void;
    readonly sim_reset: (a: number) => void;
    readonly sim_restoreWriteLog: (a: number, b: number, c: number) => void;
    readonly sim_resume: (a: number) => void;
    readonly sim_runTo: (a: number, b: number) => number;
    readonly sim_setFpReg: (a: number, b: number, c: number, d: number) => void;
    readonly sim_setIntbp: (a: number, b: number) => void;
    readonly sim_setMemWord: (a: number, b: number, c: number) => void;
    readonly sim_setPc: (a: number, b: number) => void;
    readonly sim_setPsw: (a: number, b: number) => void;
    readonly sim_setRbase: (a: number, b: number) => void;
    readonly sim_setReg: (a: number, b: number, c: number) => void;
    readonly sim_stepOnce: (a: number) => number;
    readonly sim_takeWriteLog: (a: number) => [number, number];
    readonly sim_writeSlice: (a: number, b: number, c: number, d: number) => void;
    readonly sim_getRbase: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
