/* @ts-self-types="./sim.d.ts" */

/**
 * A JS-side handle to a running G3 simulation.
 */
export class Sim {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SimFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sim_free(ptr, 0);
    }
    /**
     * @param {number} addr
     */
    addBreakpoint(addr) {
        wasm.sim_addBreakpoint(this.__wbg_ptr, addr);
    }
    /**
     * Add a watchpoint: stop when a RAM write touches `addr`..`addr+size-1`.
     * size must be 1, 2 or 4; it is rounded down to the nearest power of two.
     * @param {number} addr
     * @param {number} size
     */
    addWatchpoint(addr, size) {
        wasm.sim_addWatchpoint(this.__wbg_ptr, addr, size);
    }
    /**
     * Check data watchpoints after a manual memory write (Watch entry edit).
     * Returns true if the write hit a watchpoint (CPU now stopped).
     * @returns {boolean}
     */
    checkWatchpoints() {
        const ret = wasm.sim_checkWatchpoints(this.__wbg_ptr);
        return ret !== 0;
    }
    clearBreakpoints() {
        wasm.sim_clearBreakpoints(this.__wbg_ptr);
    }
    clearWatchpoints() {
        wasm.sim_clearWatchpoints(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    getEiic() {
        const ret = wasm.sim_getEiic(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getEipc() {
        const ret = wasm.sim_getEipc(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getFeic() {
        const ret = wasm.sim_getFeic(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getFepc() {
        const ret = wasm.sim_getFepc(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {number} n
     * @returns {number}
     */
    getFpReg(n) {
        const ret = wasm.sim_getFpReg(this.__wbg_ptr, n);
        return ret >>> 0;
    }
    /**
     * @param {number} n
     * @returns {number}
     */
    getFpRegHi(n) {
        const ret = wasm.sim_getFpRegHi(this.__wbg_ptr, n);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getFpsr() {
        const ret = wasm.sim_getFpsr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getInsCount() {
        const ret = wasm.sim_getInsCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Report the most recent execution fault, if the current PC is outside the
     * loaded ROM (i.e. the CPU fell off the end of code into a vector/0xFF area
     * and is now taking RIEs there).
     * Returns [lastRomPc, 0x60, 0] — lastRomPc = the last PC inside ROM, the
     * instruction at which execution effectively ran off (typically a missing
     * halt at end-of-loop, or an unknown instruction that raised RIE whose
     * vector is not in the loaded image). cause = 0 when PC is inside ROM.
     * @returns {Uint32Array}
     */
    getLastFault() {
        const ret = wasm.sim_getLastFault(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @param {number} addr
     * @returns {number}
     */
    getMemByte(addr) {
        const ret = wasm.sim_getMemByte(this.__wbg_ptr, addr);
        return ret >>> 0;
    }
    /**
     * @param {number} addr
     * @returns {number}
     */
    getMemWord(addr) {
        const ret = wasm.sim_getMemWord(this.__wbg_ptr, addr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getPc() {
        const ret = wasm.sim_getPc(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getPsw() {
        const ret = wasm.sim_getPsw(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getRamSize() {
        const ret = wasm.sim_getRamSize(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getRb() {
        const ret = wasm.sim_getRb(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getRbase() {
        const ret = wasm.sim_getRbase(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {number} n
     * @returns {number}
     */
    getReg(n) {
        const ret = wasm.sim_getReg(this.__wbg_ptr, n);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    getStopReason() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sim_getStopReason(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Uint32Array}
     */
    getWatchpoints() {
        const ret = wasm.sim_getWatchpoints(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Read-only: true if the CPU is parked in the HALT state.
     * @returns {boolean}
     */
    isHalted() {
        const ret = wasm.sim_isHalted(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    isStopped() {
        const ret = wasm.sim_isStopped(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Load the program ROM at a base address.
     * @param {Uint8Array} data
     * @param {number} base
     */
    loadRom(data, base) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.sim_loadRom(this.__wbg_ptr, ptr0, len0, base);
    }
    /**
     * @param {number} ram_size
     */
    constructor(ram_size) {
        const ret = wasm.sim_new(ram_size);
        this.__wbg_ptr = ret;
        SimFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Read a RAM/ROM slice into a returned Uint8Array.
     * @param {number} addr
     * @param {number} len
     * @returns {Uint8Array}
     */
    readSlice(addr, len) {
        const ret = wasm.sim_readSlice(this.__wbg_ptr, addr, len);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Read a 32-bit word from memory (ROM or RAM).
     * @param {number} addr
     * @returns {number}
     */
    readWord(addr) {
        const ret = wasm.sim_readWord(this.__wbg_ptr, addr);
        return ret >>> 0;
    }
    /**
     * @param {number} addr
     */
    removeBreakpoint(addr) {
        wasm.sim_removeBreakpoint(this.__wbg_ptr, addr);
    }
    /**
     * @param {number} addr
     */
    removeWatchpoint(addr) {
        wasm.sim_removeWatchpoint(this.__wbg_ptr, addr);
    }
    /**
     * Reset the CPU.
     */
    reset() {
        wasm.sim_reset(this.__wbg_ptr);
    }
    /**
     * Restore RAM from a flat write log [addr0, oldByte0, ...] (undo).
     * @param {Uint32Array} log
     */
    restoreWriteLog(log) {
        const ptr0 = passArray32ToWasm0(log, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.sim_restoreWriteLog(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Resume execution from a HALT-state stop (or any stop).
     * A hardware HALT parks the core until released — this models the release
     * request (e.g. an external event). No-op when not halted.
     */
    resume() {
        wasm.sim_resume(this.__wbg_ptr);
    }
    /**
     * Run until a breakpoint, halt, watchpoint, or `max_ins` instructions.
     * @param {number} max_ins
     * @returns {number}
     */
    runTo(max_ins) {
        const ret = wasm.sim_runTo(this.__wbg_ptr, max_ins);
        return ret >>> 0;
    }
    /**
     * Set an FP register (for snapshot restore). lo/hi are the low/high 32 bits.
     * @param {number} n
     * @param {number} lo
     * @param {number} hi
     */
    setFpReg(n, lo, hi) {
        wasm.sim_setFpReg(this.__wbg_ptr, n, lo, hi);
    }
    /**
     * @param {number} v
     */
    setIntbp(v) {
        wasm.sim_setIntbp(this.__wbg_ptr, v);
    }
    /**
     * @param {number} addr
     * @param {number} v
     */
    setMemWord(addr, v) {
        wasm.sim_setMemWord(this.__wbg_ptr, addr, v);
    }
    /**
     * @param {number} v
     */
    setPc(v) {
        wasm.sim_setPc(this.__wbg_ptr, v);
    }
    /**
     * Set PSW directly (for snapshot restore / stepping back).
     * @param {number} v
     */
    setPsw(v) {
        wasm.sim_setPsw(this.__wbg_ptr, v);
    }
    /**
     * Set the reset base address (RBASE). PC after reset = RBASE & ~1.
     * @param {number} v
     */
    setRbase(v) {
        wasm.sim_setRbase(this.__wbg_ptr, v);
    }
    /**
     * @param {number} n
     * @param {number} v
     */
    setReg(n, v) {
        wasm.sim_setReg(this.__wbg_ptr, n, v);
    }
    /**
     * Execute one instruction. Returns true if the CPU is now stopped.
     * @returns {boolean}
     */
    stepOnce() {
        const ret = wasm.sim_stepOnce(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Drain the RAM write log since the last call: returns flat
     * [addr0, oldByte0, addr1, oldByte1, ...].
     * @returns {Uint32Array}
     */
    takeWriteLog() {
        const ret = wasm.sim_takeWriteLog(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @param {number} addr
     * @param {Uint8Array} data
     */
    writeSlice(addr, data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.sim_writeSlice(this.__wbg_ptr, addr, ptr0, len0);
    }
}
if (Symbol.dispose) Sim.prototype[Symbol.dispose] = Sim.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_bb96b2010945f0bc: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./sim_bg.js": import0,
    };
}

const SimFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sim_free(ptr, 1));

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('sim_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
