//! IEEE-754 FPU for RH850G3MH (single + double precision).
//!
//! The G3 FPU keeps 16 register pairs (F0..F15), each 64 bits. Single-precision ops use the
//! low 32 bits; double-precision ops use the full 64 bits. Rounding modes come from FPCFG
//! (RN/RZ/RP/RM). We implement ops via f64 arithmetic and round to the target format.

use crate::state::G3;

#[derive(Clone, Copy)]
pub enum RoundMode {
    Rn, // nearest, ties to even
    Rz, // toward zero
    Rp, // toward +inf
    Rm, // toward -inf
}

impl RoundMode {
    pub fn from_cfg(fpcfg: u32) -> Self {
        match (fpcfg >> 9) & 0b11 {
            0b01 => RoundMode::Rz,
            0b10 => RoundMode::Rp,
            0b11 => RoundMode::Rm,
            _ => RoundMode::Rn,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FpStatus {
    Normal,
    Invalid,
    DivideByZero,
    Inexact,
    Overflow,
    Underflow,
    NaN,
}

/// Convert FPCFG rounding mode and an f64 to a 32-bit single.
pub fn round_f64_to_f32(x: f64, _m: RoundMode) -> (u32, FpStatus) {
    if x.is_nan() {
        return (0x7FC0_0000, FpStatus::NaN);
    }
    if x == f64::INFINITY {
        return (0x7F80_0000, FpStatus::Overflow);
    }
    if x == f64::NEG_INFINITY {
        return (0xFF80_0000, FpStatus::Overflow);
    }
    let bits = x as f32; // this performs rounding to f32 (round-to-nearest-even by default)
    // Note: for exact Rz/Rp/Rm we rely on f32 cast which is RNE; for most debug purposes
    // RNE is acceptable. We record Inexact if precision was lost.
    let exact = (bits as f64) == x;
    let inexact = if exact { FpStatus::Normal } else { FpStatus::Inexact };
    (bits.to_bits(), inexact)
}

fn round_f64_to_f64(x: f64, _m: RoundMode) -> (u64, FpStatus) {
    if x.is_nan() {
        return (0x7FF8_0000_0000_0000u64, FpStatus::NaN);
    }
    (x.to_bits(), FpStatus::Normal)
}

fn f32_to_f64(bits: u32) -> f64 {
    f32::from_bits(bits) as f64
}

fn f64_to_f64(bits: u64) -> f64 {
    f64::from_bits(bits)
}

impl G3 {
    pub fn fp_round_mode(&self) -> RoundMode {
        RoundMode::from_cfg(self.fpcfg)
    }

    /// Read single-precision FPr (low 32 bits of pair pr).
    pub fn fp_single(&self, pr: u32) -> u32 {
        (self.fp[pr as usize] & 0xFFFF_FFFF) as u32
    }
    /// Read double-precision FPr (full 64 bits).
    pub fn fp_double(&self, pr: u32) -> u64 {
        self.fp[pr as usize]
    }
    /// Write single-precision to FPr (low 32, high cleared).
    pub fn set_fp_single(&mut self, pr: u32, v: u32) {
        self.fp[pr as usize] = v as u64;
    }
    /// Write double-precision to FPr.
    pub fn set_fp_double(&mut self, pr: u32, v: u64) {
        self.fp[pr as usize] = v;
    }

    // ---- basic arithmetic ----
    pub fn addf(&mut self, dst: u32, a: u32, b: u32, dbl: bool) {
        let (x, y) = if dbl {
            (f64_to_f64(self.fp_double(a)), f64_to_f64(self.fp_double(b)))
        } else {
            (f32_to_f64(self.fp_single(a)), f32_to_f64(self.fp_single(b)))
        };
        let r = x + y;
        if dbl {
            let (bits, st) = round_f64_to_f64(r, self.fp_round_mode());
            self.set_fp_double(dst, bits);
            self.flag_fp(st);
        } else {
            let (bits, st) = round_f64_to_f32(r, self.fp_round_mode());
            self.set_fp_single(dst, bits);
            self.flag_fp(st);
        }
    }

    pub fn subf(&mut self, dst: u32, a: u32, b: u32, dbl: bool) {
        let (x, y) = if dbl {
            (f64_to_f64(self.fp_double(a)), f64_to_f64(self.fp_double(b)))
        } else {
            (f32_to_f64(self.fp_single(a)), f32_to_f64(self.fp_single(b)))
        };
        let r = x - y;
        self.store_fp_result(dst, r, dbl);
    }

    pub fn mulf(&mut self, dst: u32, a: u32, b: u32, dbl: bool) {
        let (x, y) = if dbl {
            (f64_to_f64(self.fp_double(a)), f64_to_f64(self.fp_double(b)))
        } else {
            (f32_to_f64(self.fp_single(a)), f32_to_f64(self.fp_single(b)))
        };
        let r = x * y;
        self.store_fp_result(dst, r, dbl);
    }

    pub fn divf(&mut self, dst: u32, a: u32, b: u32, dbl: bool) {
        let (x, y) = if dbl {
            (f64_to_f64(self.fp_double(a)), f64_to_f64(self.fp_double(b)))
        } else {
            (f32_to_f64(self.fp_single(a)), f32_to_f64(self.fp_single(b)))
        };
        let r = x / y;
        // track divide-by-zero / invalid
        if y == 0.0 {
            self.flag_fp(if x == 0.0 { FpStatus::Invalid } else { FpStatus::DivideByZero });
        }
        self.store_fp_result(dst, r, dbl);
    }

    fn store_fp_result(&mut self, dst: u32, r: f64, dbl: bool) {
        if dbl {
            let (bits, st) = round_f64_to_f64(r, self.fp_round_mode());
            self.set_fp_double(dst, bits);
            self.flag_fp(st);
        } else {
            let (bits, st) = round_f64_to_f32(r, self.fp_round_mode());
            self.set_fp_single(dst, bits);
            self.flag_fp(st);
        }
    }

    pub fn negf(&mut self, dst: u32, a: u32, dbl: bool) {
        if dbl {
            let v = self.fp_double(a);
            self.set_fp_double(dst, v ^ 0x8000_0000_0000_0000);
        } else {
            let v = self.fp_single(a);
            self.set_fp_single(dst, v ^ 0x8000_0000);
        }
    }

    pub fn absf(&mut self, dst: u32, a: u32, dbl: bool) {
        if dbl {
            let v = self.fp_double(a);
            self.set_fp_double(dst, v & !0x8000_0000_0000_0000);
        } else {
            let v = self.fp_single(a);
            self.set_fp_single(dst, v & !0x8000_0000);
        }
    }

    // ---- conversions ----
    /// CVTF: convert integer (r) to float dst.
    pub fn cvtf_i2f(&mut self, dst: u32, r: u32, dbl: bool) {
        let x = r as i32 as f64;
        self.store_fp_result(dst, x, dbl);
    }
    /// CVTS: convert float to integer (round to int, saturate).
    pub fn cvts_f2i(&mut self, dst: u32, src: u32, dbl: bool) {
        let x = if dbl {
            f64_to_f64(self.fp_double(src))
        } else {
            f32_to_f64(self.fp_single(src))
        };
        let iv = if x.is_nan() {
            0
        } else {
            let clamped = x.max(-2147483648.0).min(2147483647.0);
            let rounded = match self.fp_round_mode() {
                RoundMode::Rn => clamped.round(),
                RoundMode::Rz => clamped.trunc(),
                RoundMode::Rp => clamped.ceil(),
                RoundMode::Rm => clamped.floor(),
            };
            rounded as i32
        };
        self.r[dst as usize] = iv as u32;
    }
}

impl G3 {
    /// Flag FPU status (simplified FPSR). We keep a small set of sticky bits.
    fn flag_fp(&mut self, st: FpStatus) {
        // FPSR bits (simplified): 0=VF, 1=DF, 2=UF, 3=OF, 4=NF, 5=IF (inexact)
        match st {
            FpStatus::Invalid => self.fpsr |= 1 << 0,
            FpStatus::DivideByZero => self.fpsr |= 1 << 1,
            FpStatus::Underflow => self.fpsr |= 1 << 2,
            FpStatus::Overflow => self.fpsr |= 1 << 3,
            FpStatus::NaN => self.fpsr |= 1 << 4,
            FpStatus::Inexact => self.fpsr |= 1 << 5,
            FpStatus::Normal => {}
        }
    }
}
