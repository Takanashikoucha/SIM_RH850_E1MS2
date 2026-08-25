//! RH850G3MH (RH850/E1M-S2 CPU1) instruction simulator, compiled to WASM.

pub mod exception;
pub mod exec;
pub mod fpu;
pub mod mem;
pub mod state;

pub use mem::Memory;
pub use state::G3;

use wasm_bindgen::prelude::*;

/// A JS-side handle to a running G3 simulation.
#[wasm_bindgen]
#[derive(Clone)]
pub struct Sim {
    g: G3,
    mem: Memory,
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new(ram_size: u32) -> Sim {
        Sim {
            g: G3::new(),
            mem: Memory::new(if ram_size == 0 { 0x40_0000 } else { ram_size }),
        }
    }

    /// Load the program ROM at a base address.
    #[wasm_bindgen(js_name = "loadRom")]
    pub fn load_rom(&mut self, data: &[u8], base: u32) {
        self.mem.load_rom(data, base);
    }

    /// Set the reset base address (RBASE). PC after reset = RBASE & ~1.
    #[wasm_bindgen(js_name = "setRbase")]
    pub fn set_rbase(&mut self, v: u32) {
        self.g.rb = v;
        self.g.ebase = v; // default EBASE = RBASE
    }

    #[wasm_bindgen(js_name = "setIntbp")]
    pub fn set_intbp(&mut self, v: u32) {
        self.g.intbp = v;
    }

    /// Reset the CPU.
    #[wasm_bindgen(js_name = "reset")]
    pub fn reset(&mut self) {
        state::cpu_reset(&mut self.g);
        self.g.halted = false;
        self.g.ins_count = 0;
        // Manual seek / new ROM: release watchpoint-triggered stops so the CPU
        // is runnable again. HALT-state stops stay parked (OCD semantics).
        if self.g.stop_reason.starts_with("watchpoint") {
            self.g.stopped = false;
            self.g.stop_reason.clear();
        }
    }

    /// Resume execution from a HALT-state stop (or any stop).
    /// A hardware HALT parks the core until released — this models the release
    /// request (e.g. an external event). No-op when not halted.
    #[wasm_bindgen(js_name = "resume")]
    pub fn resume(&mut self) {
        if self.g.halted {
            self.g.halted = false;
            self.g.stopped = false;
            self.g.stop_reason.clear();
        }
    }

    /// Execute one instruction. Returns true if the CPU is now stopped.
    #[wasm_bindgen(js_name = "stepOnce")]
    pub fn step_once(&mut self) -> bool {
        if self.g.stopped {
            return true;
        }
        // Check breakpoint at current PC
        if self.g.breakpoints.contains(&self.g.pc) {
            self.g.on_break = true;
            self.g.stopped = true;
            self.g.stop_reason = "breakpoint".into();
            return true;
        }
        exec::step(&mut self.g, &mut self.mem);
        check_watchpoints(&mut self.g, &mut self.mem);
        self.g.on_break = self.g.breakpoints.contains(&self.g.pc);
        self.g.stopped
    }

    /// Run until a breakpoint, halt, watchpoint, or `max_ins` instructions.
    #[wasm_bindgen(js_name = "runTo")]
    pub fn run_to(&mut self, max_ins: u32) -> u32 {
        let mut n = 0u32;
        while !self.g.stopped && n < max_ins {
            if self.g.breakpoints.contains(&self.g.pc) {
                self.g.on_break = true;
                self.g.stopped = true;
                self.g.stop_reason = "breakpoint".into();
                break;
            }
            exec::step(&mut self.g, &mut self.mem);
            check_watchpoints(&mut self.g, &mut self.mem);
            n += 1;
        }
        self.g.on_break = self.g.breakpoints.contains(&self.g.pc);
        n
    }

    #[wasm_bindgen(js_name = "addBreakpoint")]
    pub fn add_breakpoint(&mut self, addr: u32) {
        self.g.breakpoints.insert(addr);
    }
    #[wasm_bindgen(js_name = "removeBreakpoint")]
    pub fn remove_breakpoint(&mut self, addr: u32) {
        self.g.breakpoints.remove(&addr);
    }
    #[wasm_bindgen(js_name = "clearBreakpoints")]
    pub fn clear_breakpoints(&mut self) {
        self.g.breakpoints.clear();
    }

    // ---- data watchpoints (OCD "Event Detection" parity) ----
    /// Add a watchpoint: stop when a RAM write touches `addr`..`addr+size-1`.
    /// size must be 1, 2 or 4; it is rounded down to the nearest power of two.
    #[wasm_bindgen(js_name = "addWatchpoint")]
    pub fn add_watchpoint(&mut self, addr: u32, size: u32) {
        let size = match size {
            1 => 1,
            2 => 2,
            _ => 4,
        };
        self.g.watchpoints.insert(addr, size as u8);
    }
    #[wasm_bindgen(js_name = "removeWatchpoint")]
    pub fn remove_watchpoint(&mut self, addr: u32) {
        self.g.watchpoints.remove(&addr);
    }
    #[wasm_bindgen(js_name = "clearWatchpoints")]
    pub fn clear_watchpoints(&mut self) {
        self.g.watchpoints.clear();
    }
    #[wasm_bindgen(js_name = "getWatchpoints")]
    pub fn get_watchpoints(&self) -> Vec<u32> {
        // flat [addr, size, addr, size, ...]
        self.g
            .watchpoints
            .iter()
            .flat_map(|(a, s)| [*a, *s as u32])
            .collect()
    }

    // ---- state getters ----
    #[wasm_bindgen(js_name = "getReg")]
    pub fn get_reg(&self, n: u32) -> u32 {
        self.g.r[n as usize]
    }
    #[wasm_bindgen(js_name = "setReg")]
    pub fn set_reg(&mut self, n: u32, v: u32) {
        if n != 0 {
            self.g.r[n as usize] = v;
        }
    }
    #[wasm_bindgen(js_name = "getPc")]
    pub fn get_pc(&self) -> u32 {
        self.g.pc
    }
    #[wasm_bindgen(js_name = "setPc")]
    pub fn set_pc(&mut self, v: u32) {
        self.g.pc = v & !1;
    }
    #[wasm_bindgen(js_name = "getPsw")]
    pub fn get_psw(&self) -> u32 {
        self.g.psw
    }
    #[wasm_bindgen(js_name = "getFpReg")]
    pub fn get_fp_reg(&self, n: u32) -> u32 {
        (self.g.fp[n as usize] & 0xFFFF_FFFF) as u32
    }
    #[wasm_bindgen(js_name = "getFpRegHi")]
    pub fn get_fp_reg_hi(&self, n: u32) -> u32 {
        (self.g.fp[n as usize] >> 32) as u32
    }
    /// Set an FP register (for snapshot restore). lo/hi are the low/high 32 bits.
    #[wasm_bindgen(js_name = "setFpReg")]
    pub fn set_fp_reg(&mut self, n: u32, lo: u32, hi: u32) {
        self.g.fp[n as usize] = ((hi as u64) << 32) | (lo as u64);
    }
    #[wasm_bindgen(js_name = "getInsCount")]
    pub fn get_ins_count(&self) -> u32 {
        self.g.ins_count as u32
    }
    #[wasm_bindgen(js_name = "isStopped")]
    pub fn is_stopped(&self) -> bool {
        self.g.stopped
    }
    #[wasm_bindgen(js_name = "getStopReason")]
    pub fn get_stop_reason(&self) -> String {
        self.g.stop_reason.clone()
    }
    #[wasm_bindgen(js_name = "getMemByte")]
    pub fn get_mem_byte(&self, addr: u32) -> u32 {
        self.mem.read_byte(addr) as u32
    }
    #[wasm_bindgen(js_name = "getMemWord")]
    pub fn get_mem_word(&self, addr: u32) -> u32 {
        self.mem.read_word(addr)
    }
    #[wasm_bindgen(js_name = "setMemWord")]
    pub fn set_mem_word(&mut self, addr: u32, v: u32) {
        self.mem.write_word(addr, v);
    }
    /// Read a 32-bit word from memory (ROM or RAM).
    #[wasm_bindgen(js_name = "readWord")]
    pub fn read_word(&self, addr: u32) -> u32 {
        self.mem.read_word(addr)
    }

    /// Set PSW directly (for snapshot restore / stepping back).
    #[wasm_bindgen(js_name = "setPsw")]
    pub fn set_psw(&mut self, v: u32) {
        self.g.psw = v;
    }

    /// Check data watchpoints after a manual memory write (Watch entry edit).
    /// Returns true if the write hit a watchpoint (CPU now stopped).
    #[wasm_bindgen(js_name = "checkWatchpoints")]
    pub fn check_watchpoints(&mut self) -> bool {
        check_watchpoints(&mut self.g, &mut self.mem);
        self.g.stopped
    }

    /// Read-only: true if the CPU is parked in the HALT state.
    #[wasm_bindgen(js_name = "isHalted")]
    pub fn is_halted(&self) -> bool {
        self.g.halted
    }

    /// Drain the RAM write log since the last call: returns flat
    /// [addr0, oldByte0, addr1, oldByte1, ...].
    #[wasm_bindgen(js_name = "takeWriteLog")]
    pub fn take_write_log(&mut self) -> Vec<u32> {
        self.mem.take_write_log().into_iter().flat_map(|(a, b)| [a, b as u32]).collect()
    }

    /// Restore RAM from a flat write log [addr0, oldByte0, ...] (undo).
    #[wasm_bindgen(js_name = "restoreWriteLog")]
    pub fn restore_write_log(&mut self, log: &[u32]) {
        let pairs: Vec<(u32, u8)> = log
            .chunks_exact(2)
            .map(|c| (c[0], c[1] as u8))
            .collect();
        self.mem.restore_from_log(&pairs);
    }

    #[wasm_bindgen(js_name = "getRbase")]
    pub fn get_rbase(&self) -> u32 {
        self.g.rb
    }
    #[wasm_bindgen(js_name = "getEipc")]
    pub fn get_eipc(&self) -> u32 {
        self.g.eipc
    }
    #[wasm_bindgen(js_name = "getFepc")]
    pub fn get_fepc(&self) -> u32 {
        self.g.fepc
    }
    #[wasm_bindgen(js_name = "getRb")]
    pub fn get_rb(&self) -> u32 {
        self.g.rb
    }
    #[wasm_bindgen(js_name = "getRamSize")]
    pub fn get_ram_size(&self) -> u32 {
        self.mem.ram_size
    }
    /// Read a RAM/ROM slice into a returned Uint8Array.
    #[wasm_bindgen(js_name = "readSlice")]
    pub fn read_slice(&self, addr: u32, len: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len {
            out.push(self.mem.read_byte(addr + i));
        }
        out
    }
    #[wasm_bindgen(js_name = "writeSlice")]
    pub fn write_slice(&mut self, addr: u32, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            self.mem.write_byte(addr + i as u32, *b);
        }
    }
    #[wasm_bindgen(js_name = "getFpsr")]
    pub fn get_fpsr(&self) -> u32 {
        self.g.fpsr
    }
    #[wasm_bindgen(js_name = "getEiic")]
    pub fn get_eiic(&self) -> u32 {
        self.g.eiic
    }
    /// Report the most recent execution fault, if the current PC is outside the
    /// loaded ROM (i.e. the CPU fell off the end of code into a vector/0xFF area
    /// and is now taking RIEs there).
    /// Returns [lastRomPc, 0x60, 0] — lastRomPc = the last PC inside ROM, the
    /// instruction at which execution effectively ran off (typically a missing
    /// halt at end-of-loop, or an unknown instruction that raised RIE whose
    /// vector is not in the loaded image). cause = 0 when PC is inside ROM.
    #[wasm_bindgen(js_name = "getLastFault")]
    pub fn get_last_fault(&self) -> Vec<u32> {
        if self.mem.in_rom(self.g.pc) {
            return vec![self.g.pc, 0, 0];
        }
        // Walk back: try the previous 2-byte slots to find the last ROM PC.
        let mut p = self.g.pc;
        for _ in 0..4 {
            if self.mem.in_rom(p) {
                return vec![p, 0x60, 0];
            }
            p = p.wrapping_sub(2);
        }
        vec![self.g.pc, 0x60, 0]
    }
    #[wasm_bindgen(js_name = "getFeic")]
    pub fn get_feic(&self) -> u32 {
        self.g.feic
    }
}

/// Stop the CPU if the RAM writes since the last check touched a watchpoint.
/// Called after every executed instruction (stepOnce/runTo) and from JS after
/// manual memory writes (so watchpoints fire on debugger writes too).
///
/// The write log is only *drained* when it is inspected. If the CPU is already
/// stopped (e.g. a breakpoint hit at PC+2), the log is left intact so the JS
/// layer's takeWriteLog still sees the writes for its changed-addr tracking.
fn check_watchpoints(g: &mut G3, mem: &mut Memory) {
    if g.watchpoints.is_empty() {
        return;
    }
    if g.stopped {
        // A prior stop already owns this write batch — don't touch the log.
        return;
    }
    let log = mem.take_write_log();
    for (addr, _old) in &log {
        for (wp_addr, size) in &g.watchpoints {
            let s = *size as u32;
            if *addr >= *wp_addr && *addr < *wp_addr + s {
                g.stopped = true;
                g.stop_reason = format!("watchpoint 0x{:08X}", wp_addr);
                return;
            }
        }
    }
}

