//! Core CPU state types for the RH850G3MH simulator.

// ---- PSW bits ----
pub const PSW_UM: u32 = 1 << 31;
pub const PSW_CU2: u32 = 1 << 18;
pub const PSW_CU1: u32 = 1 << 17;
pub const PSW_CU0: u32 = 1 << 16;
pub const PSW_EBV: u32 = 1 << 15;
pub const PSW_EB: u32 = 1 << 14; // debug EB
pub const PSW_EP: u32 = 1 << 13; // debug EP
pub const PSW_PD: u32 = 1 << 12; // debug PD
pub const PSW_SAT: u32 = 1 << 11;
pub const PSW_ID: u32 = 1 << 6; // interrupt disable
pub const PSW_CY: u32 = 1 << 10;
pub const PSW_NZ: u32 = 1 << 9;
pub const PSW_S: u32 = 1 << 8;
pub const PSW_V: u32 = 1 << 7;
pub const PSW_RESET_VAL: u32 = 0x0020;

// ---- Exception cause codes (lower 16 bits) ----
pub const CAUSE_RESET: u16 = 0x00;
pub const CAUSE_SYSERR: u16 = 0x10;
pub const CAUSE_FETRAP: u16 = 0x31; // 31H-3FH range
pub const CAUSE_TRAP0: u16 = 0x40;
pub const CAUSE_TRAP1: u16 = 0x50;
pub const CAUSE_RIE: u16 = 0x60;
pub const CAUSE_FPINT: u16 = 0x71;
pub const CAUSE_UCPOP0: u16 = 0x80;
pub const CAUSE_UCPOP1: u16 = 0x81;
pub const CAUSE_UCPOP2: u16 = 0x82;
pub const CAUSE_MIP: u16 = 0x90;
pub const CAUSE_MDP: u16 = 0x91;
pub const CAUSE_PIE: u16 = 0xA0;
pub const CAUSE_DEBUG: u16 = 0xB0;
pub const CAUSE_MAE: u16 = 0xC0;
pub const CAUSE_FENMI: u16 = 0xE0;
pub const CAUSE_FEINT: u16 = 0xF0;

#[inline]
pub fn vector_offset(cause: u16) -> u32 {
    let b = cause & 0xFF;
    // EIINTn: priority p => 0x100 + p*0x10
    if cause >= 0x1000 && cause < 0x1200 {
        let p = ((cause - 0x1000) / 0x40) as u32; // placeholder; priority set separately
        0x100 + p * 0x10
    } else {
        match b {
            0x00 => 0x000,
            0x10 => 0x010, // SYSERR
            0x31 => 0x030, // FETRAP
            0x40..=0x5F => {
                // TRAP vectors: cause 0x40-0x5F → offset 0x040 + (cause-0x40)*0x10
                // But cause 0x40 is TRAP0, 0x50 is TRAP1, others are reserved
                // For simplicity: 0x40→0x040, 0x50→0x050, rest→0x040
                if b == 0x50 { 0x050 } else { 0x040 }
            }
            0x60 => 0x060, // RIE
            0x71 => 0x070, // FPINT
            0x80 | 0x81 | 0x82 => 0x080, // UCPOP
            0x90 | 0x91 => 0x090, // MIP/MDP
            0xA0 => 0x0A0, // PIE
            0xB0 => 0x0B0, // Debug
            0xC0 => 0x0C0, // MAE
            0xE0 => 0x0E0, // FENMI
            0xF0 => 0x0F0, // FEINT
            _ => 0x000,
        }
    }
}

/// Full CPU state.
#[derive(Clone)]
pub struct G3 {
    pub r: [u32; 32],
    pub pc: u32,
    pub psw: u32,
    // system registers
    pub rb: u32, // RBASE (SR2,1)
    pub ebase: u32, // EBASE (SR3,1)
    pub intbp: u32, // INTBP (SR4,1)
    pub pid: u32, // PID (SR6,1)
    // EI/FE status save
    pub eipc: u32,
    pub eipsw: u32,
    pub fepc: u32,
    pub fepsw: u32,
    pub eiic: u32,
    pub feic: u32,
    pub eiwr: u32,
    pub fewr: u32,
    // FPU: 16 register pairs F0..F15 (each holds 64-bit)
    pub fp: [u64; 16],
    pub fpsr: u32,
    pub fpepc: u32,
    pub fpcfg: u32,
    // CALLT registers
    pub ctpc: u32,
    pub ctpsw: u32,
    pub ctbp: u32,
    // SYSCALL registers
    pub sccfg: u32,
    pub scbp: u32,
    // control / debug
    pub stopped: bool,
    pub stop_reason: String,
    pub ins_count: u64,
    pub breakpoints: std::collections::HashSet<u32>,
    pub on_break: bool,
    /// HALT instruction state: CPU is parked until an external request
    /// (exception / watchpoint / debugger resume) releases it.
    pub halted: bool,
    /// Data watchpoints: addr (low bits) + size (1/2/4 bytes). Checked after
    /// each executed instruction against the write log.
    pub watchpoints: std::collections::HashMap<u32, u8>,
}

impl G3 {
    pub fn new() -> Self {
        G3 {
            r: [0; 32],
            pc: 0,
            psw: PSW_RESET_VAL,
            rb: 0,
            ebase: 0,
            intbp: 0,
            pid: 0,
            eipc: 0,
            eipsw: 0,
            fepc: 0,
            fepsw: 0,
            eiic: 0,
            feic: 0,
            eiwr: 0,
            fewr: 0,
            fp: [0; 16],
            fpsr: 0,
            fpepc: 0,
            fpcfg: 0,
            ctpc: 0,
            ctpsw: 0,
            ctbp: 0,
            sccfg: 0,
            scbp: 0,
            stopped: false,
            stop_reason: String::new(),
            ins_count: 0,
            breakpoints: std::collections::HashSet::new(),
            on_break: false,
            halted: false,
            watchpoints: std::collections::HashMap::new(),
        }
    }

    // ---- PSW helpers ----
    pub fn psw_flag(&self, mask: u32) -> bool {
        self.psw & mask != 0
    }
    pub fn set_psw(&mut self, mask: u32, v: bool) {
        if v {
            self.psw |= mask;
        } else {
            self.psw &= !mask;
        }
    }

    /// Evaluate a condition code (Table 7.4).
    /// NOTE: bit9 is the NZ flag — 1 means "Not Zero" (i.e. Z=0).
    pub fn cond(&self, cccc: u32) -> bool {
        let s = self.psw & PSW_S != 0;
        let v = self.psw & PSW_V != 0;
        let c = self.psw & PSW_CY != 0;
        let z = self.psw & PSW_NZ == 0; // Z flag = NOT NZ
        let sat = self.psw & PSW_SAT != 0;
        match cccc {
            0b0000 => v,
            0b1000 => !v,
            0b0001 => c,
            0b1001 => !c,
            0b0010 => z,
            0b1010 => !z,
            0b0011 => c || z,
            0b1011 => !(c || z),
            0b0100 => s,
            0b1100 => !s,
            0b0101 => true,
            0b1101 => sat,
            0b0110 => s ^ v,
            0b1110 => !(s ^ v),
            0b0111 => (s ^ v) || z,
            0b1111 => !((s ^ v) || z),
            _ => false,
        }
    }

    /// Set CY/OV/S/Z/SAT flags from an arithmetic result (no saturation).
    pub fn set_arith_flags(&mut self, result: u32, a: i32, b: i32, op: ArithOp) {
        self.set_psw(PSW_NZ, result != 0); // NZ=1 means non-zero (bit9)
        self.set_psw(PSW_S, result >> 31 == 1);
        self.set_psw(
            PSW_V,
            op == ArithOp::Add && (((a as u32) ^ (result as u32)) & ((b as u32) ^ (result as u32))) >> 31 == 1,
        );
        match op {
            ArithOp::Add => {
                let sum = (a as u64).wrapping_add(b as u64);
                self.set_psw(PSW_CY, sum >> 32 == 1);
            }
            ArithOp::Sub => self.set_psw(PSW_CY, (a as u32) <= (b as u32)),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ArithOp {
    Add,
    Sub,
}

/// CPU reset.
pub fn cpu_reset(g: &mut G3) {
    for i in 1..32 {
        g.r[i] = 0;
    }
    g.psw = PSW_RESET_VAL;
    g.pc = g.rb & !1;
    // NOT stopped: the CPU is ready to execute after reset. The user
    // controls execution via step/run buttons.
    g.stopped = false;
    g.stop_reason = String::new();
}

pub fn sign_extend(v: u32, bits: u32) -> i32 {
    if bits >= 32 {
        v as i32
    } else {
        ((v << (32 - bits)) as i32) >> (32 - bits)
    }
}
