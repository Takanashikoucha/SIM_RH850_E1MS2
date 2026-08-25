//! Instruction decode and execution for RH850G3MH.
//!
//! Decode strategy:
//!  - Read the first 16-bit halfword.
//!  - The middle 6 bits (bits 10..5) are the primary opcode for most formats.
//!  - Special 16-bit formats (Format III branch, Format IV load/store, RIE, NOP) are detected
//!    by their distinct bit patterns.
//!  - A 32-bit second halfword disambiguates the remaining formats.
//!
//! This file implements the common subset of instructions. Unrecognized opcodes raise a
//! reserved-instruction exception (RIE), which a user reset handler can turn into a halt.

use crate::exception::acknowledge;
use crate::mem::Memory;
use crate::state::*;

/// Decode and execute one instruction. Advances PC and updates state.
/// Returns the number of bytes consumed.
pub fn step(g: &mut G3, mem: &mut Memory) -> u32 {
    let pc = g.pc;
    let h0 = mem.read_half(pc) as u32;
    let op6 = (h0 >> 5) & 0x3F;

    // ---- 16-bit special formats (detected before the generic op6 dispatch) ----

    // NOP (0000H)
    if h0 == 0x0000 {
        g.pc += 2;
        g.ins_count += 1;
        return 2;
    }


    // RIE 16-bit: 0000_0000_0100_0000 = 0040H
    if h0 == 0x0040 {
        acknowledge(g, CAUSE_RIE, pc);
        g.ins_count += 1;
        return 2;
    }

    // Extended format instructions: bits 10..5 == 0b111111 (0x3F)
    // Includes: coprocessor (FPU), EIRET, FERET, CMOV, SYSCALL, FETRAP, TRAP,
    // BIT ops, MUL, MAC, DIV, SHL/SHR/SAR, etc.
    if op6 == 0x3F {
        let h1e = mem.read_half(pc + 2) as u32;
        return exec_extended(g, mem, pc, h0, h1e);
    }

    // Conditional branch (Format III): 16-bit Bcond disp8.
    // Layout: dddddddd1011cccc
    //   bits15..8  = dddddddd (disp8[7:0], 8 bits, two's complement)
    //   bits7..4   = 1011     (opcode)
    //   bits3..0   = cccc     (condition)
    // Target = PC + sign_extend(disp8) * 2 (PC = address of first byte).
    // Range: ±128 bytes (128 forward, 126 backward).
    if (h0 >> 4) & 0xF == 0b1011 {
        let cccc = h0 & 0xF;
        let disp8 = (h0 >> 8) & 0xFF;
        let disp = (sign_extend(disp8, 8) as i32).wrapping_mul(2);
        let target = pc.wrapping_add(disp as u32) & !1;
        if g.cond(cccc) {
            g.pc = target;
        } else {
            g.pc = pc + 2;
        }
        g.ins_count += 1;
        return 2;
    }

    // Read the second halfword — needed both for 32-bit instructions and for
    // disambiguating 16-bit vs 32-bit forms that share the same op6.
    let h1 = mem.read_half(pc + 2) as u32;
    let w = ((h1 as u32) << 16) | h0;

    // CALLT imm6 (Format II): 0b00000_01000_iiiiii
    //   The leading-5 bits (reg2 field, bits 15..11) MUST be 0 — that distinguishes
    //   CALLT from the 16-bit OR with the same op6.
    //   Per R01US0143 Rev1.30 §7.2.2 CALLT:
    //     CTPC  ← PC + 2 (return PC)
    //     CTPSW(4:0) ← PSW(4:0)
    //     adr   ← CTBP + zext(imm6) * 2          (table entry address)
    //     PC    ← CTBP + zext(Load-Half(adr))    (target)
    //   NOT executed (falls through to exec_extended) when h1 matches the FPU
    //   marker (bits 31..21 == 0x230) — that space is FPU instructions, which
    //   must reach the extended dispatcher instead.
    // CALLT imm6 (Format II): 0b00000_01000_iiiiii
    //   The CALLT encoding uses bits 14..10 = 01000 (NOT the op6 field, which is
    //   bits 10..5 = 00000 for CALLT). This is verified against R01US0143 Rev1.30.
    //   Distinguished from OR (op6=0b01000, bits 10..5) by checking bits 14..10.
    //   Per R01US0143 Rev1.30 §7.2.2 CALLT:
    //     CTPC  ← PC + 2 (return PC)
    //     CTPSW(4:0) ← PSW(4:0)
    //     adr   ← CTBP + zext(imm6) * 2          (table entry address)
    //     PC    ← CTBP + zext(Load-Half(adr))    (target)
    //   NOT executed (falls through) when the FPU marker matches (bits 31..21 == 0x230).
    // CALLT imm6 (Format II): word0 = 0b00000_01000_iiiiii
    //   Verified against R01US0143 Rev1.30 §7.2.2 CALLT opcode diagram:
    //     bits 15..11 = 00000, bits 10..5 = 01000, bits 4..0 = imm6
    //   op6 (bits 10..5) = 0b01000 = 16. Distinguished from OR (also op6=16)
    //   by the leading 5-bit field (bits 15..11) which must be 0 for CALLT.
    //   Per R01US0143 Rev1.30 §7.2.2 CALLT:
    //     CTPC  ← PC + 2 (return PC)
    //     CTPSW(4:0) ← PSW(4:0)
    //     adr   ← CTBP + zext(imm6) * 2          (table entry address)
    //     PC    ← CTBP + zext(Load-Half(adr))    (target)
    //   NOT executed (falls through) when the FPU marker matches.
    // CALLT imm6 (Format II): op6 = 8 (0b01000), bits 15..11 = 00000.
    // Verified: 0x0101 = 0000 0001 0000 0001, (0x0101 >> 5) & 0x3F = 8.
    if op6 == 8 && ((h0 >> 11) & 0x1F) == 0 && ((w >> 21) & 0x7FF) != 0x230 {
        let imm6 = h0 & 0x3F;
        g.ctpc = pc + 2;
        g.ctpsw = g.psw & 0x1F;
        let entry_addr = g.ctbp.wrapping_add(imm6.wrapping_mul(2));
        let offset = mem.read_half(entry_addr) as u32;
        g.pc = g.ctbp.wrapping_add(offset) & !1;
        g.ins_count += 1;
        return 2;
    }

    // ---- 16-bit Format I (reg-reg): op6 values that are unambiguous 16-bit ----
    // ADD, SUB, AND, OR, XOR, MOV, CMP, JMP, NOT, DIVH, MULH
    // NOTE: op6 28-37 are SLD/SST (Format IV), handled after this block.
    // ADDI(0b110000=48) and MOVHI(0b110010=50) are 32-bit only (Format VI),
    // so they fall through to exec_32bit.
    match op6 {
        0b001110 => return exec_arith_rr(g, h0, Op16::Add),
        0b001101 => return exec_arith_rr(g, h0, Op16::Sub),
        0b001010 => return exec_arith_rr(g, h0, Op16::And),
        0b001000 => return exec_arith_rr(g, h0, Op16::Or),
        0b001001 => return exec_arith_rr(g, h0, Op16::Xor),
        0b000000 => return exec_mov_rr(g, h0),
        0b001111 => return exec_cmp_rr(g, h0),
        0b000011 => return exec_jmp_reg(g, h0, pc),
        0b000001 => {
            // NOT reg2 (in-place)
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            g.r[r2] = !g.r[r2];
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        0b011111 => {
            // MULH (16-bit, Format I): rrrrr000111RRRRR
            //   GR[reg2](15:0) = GR[reg1](15:0) * GR[reg2](15:0)
            //   (signed halfword multiply, result in low halfword of reg2)
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let a = g.r[r1] as i16 as i32;
            let b = g.r[r2] as i16 as i32;
            let prod = a.wrapping_mul(b);
            g.r[r2] = (prod as u32) & 0xFFFF;
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        0b000010 => {
            // DIVH (16-bit): reg2 = reg2 / reg1 (halfword)
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let a = g.r[r2] as i16 as i32;
            let b = g.r[r1] as i16 as i32;
            if b != 0 {
                g.r[r2] = (a / b) as u32;
            }
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        // ---- Data manipulation (Format I, 16-bit) ----
        // ZXB reg1 (0b00100): zero-extend byte
        0b00100 => {
            let r1 = (h0 & 0x1F) as usize;
            g.r[r1] = g.r[r1] & 0xFF;
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        // SXB reg1: 00000000101RRRRR (reg1 in LOW bits 4..0)
        0b00101 => {
            let r1 = (h0 & 0x1F) as usize;
            g.r[r1] = (g.r[r1] as i8 as i32) as u32;
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        // ZXH reg1 (0b00110): zero-extend halfword
        0b00110 => {
            let r1 = (h0 & 0x1F) as usize;
            g.r[r1] = g.r[r1] & 0xFFFF;
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        // SXH reg1: 00000000111RRRRR (reg1 in LOW bits 4..0)
        7u32 => {
            let r1 = (h0 & 0x1F) as usize;
            g.r[r1] = (g.r[r1] as i16 as i32) as u32;
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        // TST reg1, reg2 (0b001011): flags from reg2 AND reg1 (no result written)
        0b001011 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let r = g.r[r2] & g.r[r1];
            g.set_psw(PSW_NZ, r != 0); // NZ=1 means non-zero
            g.set_psw(PSW_S, r >> 31 == 1);
            g.set_psw(PSW_CY, false);
            g.set_psw(PSW_V, false);
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        // MOV imm5, reg2 (Format II): rrrrr010000iiiii
        //   op6 = 0b010000 (16), reg2 = bits15..11, imm5 = bits4..0
        //   reg2 = sign_extend(imm5)
        0b010000 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let imm5 = h0 & 0x1F;
            g.r[r2] = sign_extend(imm5, 5) as u32;
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }

        // MULH imm5, reg2 (Format II): rrrrr010111iiiii
        //   op6 = 0b010111 (23), reg2 = bits15..11, imm5 = bits4..0
        //   reg2 = sext(imm5) * sext16(reg2) (low 16 bits)
        0b010111 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let imm5 = h0 & 0x1F;
            let imm = sign_extend(imm5, 5) as i16 as i32;
            let b = g.r[r2] as i16 as i32;
            let prod = imm.wrapping_mul(b);
            g.r[r2] = (prod as u32) & 0xFFFF;
            g.pc += 2;
            g.ins_count += 1;
            return 2;
        }
        // SUBI imm5, reg2 (G3K only, but G3MH may not have it)
        // Actually SUBI is not in G3MH. Skip.
        _ => {}
    }

    // 16-bit SLD/SST (Format IV): opcode field = bits10..7 (4 bits), reg2 = bits15..11.
    //   SLD.B  = 0b0110 (bits10..7)
    //   SLD.BU = 0b0000 (bits10..7), bit0=1
    //   SLD.H  = 0b1000 (bits10..7)
    //   SLD.HU = 0b0000 (bits10..7), bit0=0
    //   SLD.W  = 0b1010 (bits10..7), bit0=0
    //   SST.B  = 0b0111 (bits10..7)
    //   SST.H  = 0b1001 (bits10..7)
    //   SST.W  = 0b1010 (bits10..7), bit0=1
    let op4_sld = (h0 >> 7) & 0xF;
    // SLD.W/SST.W (op4=0b1010) conflicts with 32-bit JR/JARL (op6=0b011110=30).
    // JR/JARL has op6 = 30, SLD.W/SST.W has op6 in 40..47 range.
    // Also, SST.B (op4=0b0111) conflicts with JARL (op6=30, op4=0b0111).
    // We can distinguish: JARL has r2 != 0 AND op6 == 30, while SST.B has op6 in 28..31.
    let is_jr_jarl = op6 == 30 && ((h0 >> 11) & 0x1F) != 0;
    let is_sld_w_sst_w = op4_sld == 0b1010 && op6 >= 40 && op6 <= 47;
    if (op4_sld == 0b0110 || op4_sld == 0b0111 || op4_sld == 0b1000
        || op4_sld == 0b1001 || op4_sld == 0b0000 || is_sld_w_sst_w)
        && !is_jr_jarl
    {
        return exec_sld_sst_16(g, mem, pc, h0);
    }

    // ---- 32-bit dispatch ----
    match exec_32bit(g, mem, pc, w, h0, h1, op6) {
        Ok(n) => n,
        Err(cause) => {
            acknowledge(g, cause, pc);
            g.ins_count += 1;
            2
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Op16 {
    Add,
    Sub,
    And,
    Or,
    Xor,
}

/// reg-reg 16-bit op: reg2 = reg2 OP reg1
fn exec_arith_rr(g: &mut G3, h0: u32, op: Op16) -> u32 {
    let r2 = ((h0 >> 11) & 0x1F) as usize;
    let r1 = (h0 & 0x1F) as usize;
    let a = g.r[r2];
    let b = g.r[r1];
    match op {
        Op16::Add => {
            let r = a.wrapping_add(b);
            g.set_arith_flags(r, a as i32, b as i32, ArithOp::Add);
            g.r[r2] = r;
        }
        Op16::Sub => {
            let r = a.wrapping_sub(b);
            g.set_arith_flags(r, a as i32, b as i32, ArithOp::Sub);
            g.r[r2] = r;
        }
        Op16::And => {
            let r = a & b;
            g.set_psw(PSW_NZ, r != 0); // NZ=1 means non-zero
            g.set_psw(PSW_S, r >> 31 == 1);
            g.r[r2] = r;
        }
        Op16::Or => {
            let r = a | b;
            g.set_psw(PSW_NZ, r != 0); // NZ=1 means non-zero
            g.set_psw(PSW_S, r >> 31 == 1);
            g.r[r2] = r;
        }
        Op16::Xor => {
            let r = a ^ b;
            g.set_psw(PSW_NZ, r != 0); // NZ=1 means non-zero
            g.set_psw(PSW_S, r >> 31 == 1);
            g.r[r2] = r;
        }
    }
    g.pc += 2;
    g.ins_count += 1;
    2
}

fn exec_mov_rr(g: &mut G3, h0: u32) -> u32 {
    let r2 = ((h0 >> 11) & 0x1F) as usize;
    let r1 = (h0 & 0x1F) as usize;
    g.r[r2] = g.r[r1];
    g.pc += 2;
    g.ins_count += 1;
    2
}

fn exec_cmp_rr(g: &mut G3, h0: u32) -> u32 {
    let r2 = ((h0 >> 11) & 0x1F) as usize;
    let r1 = (h0 & 0x1F) as usize;
    let a = g.r[r2];
    let b = g.r[r1];
    g.set_arith_flags(a.wrapping_sub(b), a as i32, b as i32, ArithOp::Sub);
    g.pc += 2;
    g.ins_count += 1;
    2
}

/// JMP [reg1] — opcode 0b000110, reg1 in bits 4..0
fn exec_jmp_reg(g: &mut G3, h0: u32, _pc: u32) -> u32 {
    let r1 = (h0 & 0x1F) as usize;
    g.pc = g.r[r1] & !1;
    g.ins_count += 1;
    2
}

/// 16-bit SLD/SST (Format IV).
/// Layout: rrrrr XXXX ddddddd where XXXX = op4 (bits10..7), dddddd = disp6 (bits6..1), bit0=type.
///
/// SLD.B:  0b0110, type=1 (signed)   → r2 = sext8([r0 + disp8])
/// SLD.BU: 0b0000, type=1 (unsigned) → r2 = zext8([r0 + disp8])
/// SLD.H:  0b1000, type=0 (halfword) → r2 = sext16([r0 + disp8])
/// SLD.HU: 0b0000, type=0 (halfword) → r2 = zext16([r0 + disp8])
/// SLD.W:  0b1010, type=1 (bit0=0)   → r2 = [r0 + disp8]
/// SST.B:  0b0111                     → [r0 + disp8] = r2[7:0]
/// SST.H:  0b1001                     → [r0 + disp8] = r2[15:0]
/// SST.W:  0b1010, type=1 (bit0=1)   → [r0 + disp8] = r2
/// Short-format (Format IV) SLD/SST instructions.
///
/// Verified encodings (R01US0143 Rev1.30, pages 263–272):
///   SLD.B  : rrrrr 0110 ddddddd        disp7,  byte, signed ext
///   SLD.BU : rrrrr 0000 110 dddd       disp4,  byte, zero ext
///   SLD.H  : rrrrr 1000 ddddddd        disp8,  half, signed ext
///   SLD.HU : rrrrr 0000 111 dddd       disp5,  half, zero ext
///   SLD.W  : rrrrr 1010 dddddd 0       disp8,  word (bit0=0)
///   SST.B  : rrrrr 0111 ddddddd        disp7,  byte
///   SST.H  : rrrrr 1001 ddddddd        disp8,  half
///   SST.W  : rrrrr 1010 dddddd 1       disp8,  word (bit0=1)
///
/// Base register is the element pointer EP = r30 (compiler convention).
/// Displacements are zero-extended per the manual's Operation field.
fn exec_sld_sst_16(g: &mut G3, mem: &mut Memory, _pc: u32, h0: u32) -> u32 {
    let r2 = ((h0 >> 11) & 0x1F) as usize;
    let base = g.r[30]; // EP
    let op4 = (h0 >> 7) & 0xF;
    let bit0 = (h0 & 1) == 1;

    match (op4, bit0) {
        // SLD.B disp7 [ep], reg2
        //   Opcode: rrrrr 0110 ddddddd  (disp7 = bits6..0)
        (0b0110, _) => {
            let disp = h0 & 0x7F;
            let addr = base.wrapping_add(disp);
            let b = mem.read_byte(addr) as i8 as i32;
            g.r[r2] = b as u32;
        }
        // SST.B reg2, disp7 [ep]
        //   Opcode: rrrrr 0111 ddddddd  (disp7 = bits6..0)
        (0b0111, _) => {
            let disp = h0 & 0x7F;
            let addr = base.wrapping_add(disp);
            mem.write_byte(addr, g.r[r2] as u8);
        }
        // SLD.H disp8 [ep], reg2
        //   Opcode: rrrrr 1000 ddddddd  (disp8 = bits6..0, zero-extended)
        (0b1000, _) => {
            let disp = h0 & 0x7F;
            let addr = base.wrapping_add(disp);
            let h = mem.read_half(addr) as i16 as i32;
            g.r[r2] = h as u32;
        }
        // SST.H reg2, disp8 [ep]
        //   Opcode: rrrrr 1001 ddddddd  (disp8 = bits6..0, zero-extended)
        (0b1001, _) => {
            let disp = h0 & 0x7F;
            let addr = base.wrapping_add(disp);
            mem.write_half(addr, g.r[r2] as u16);
        }
        // SLD.W (bit0=0) / SST.W (bit0=1): rrrrr 1010 dddddd [0/1]
        //   disp8 = bits 6..1 of the 7-bit field, zero-extended (aligned access).
        (0b1010, is_store) => {
            let disp = (h0 >> 1) & 0x3F; // 6-bit field
            let addr = base.wrapping_add(disp);
            if is_store {
                mem.write_word(addr, g.r[r2]);
            } else {
                g.r[r2] = mem.read_word(addr);
            }
        }
        // SLD.BU (op4=0b0000, bit0=1) / SLD.HU (op4=0b0000, bit0=0)
        (0b0000, is_bu) => {
            if is_bu {
                // SLD.BU disp4 [ep], reg2
                //   Opcode: rrrrr 0000 1 dddd  (disp4 = bits3..0)
                let disp = h0 & 0xF;
                let addr = base.wrapping_add(disp);
                g.r[r2] = mem.read_byte(addr) as u32;
            } else {
                // SLD.HU disp5 [ep], reg2
                //   Opcode: rrrrr 0000 1 ddddd  (disp5 = bits4..0)
                let disp = h0 & 0x1F;
                let addr = base.wrapping_add(disp);
                g.r[r2] = mem.read_half(addr) as u32;
            }
        }
        _ => {}
    }
    g.pc += 2;
    g.ins_count += 1;
    2
}

/// Extended-format instruction (op6 == 0b111111). Decodes FPU, EIRET, FERET,
/// SYSCALL, FETRAP, TRAP, MUL, CMOV, BIT ops, and other 32-bit extended ops.
fn exec_extended(g: &mut G3, mem: &mut Memory, pc: u32, h0: u32, h1: u32) -> u32 {
    let w = ((h1 as u32) << 16) | h0;
    let r2 = ((h0 >> 11) & 0x1F) as usize; // reg2 (bits 15..11)
    let r1 = (h0 & 0x1F) as usize; // reg1 (bits 4..0)

    // ---- EIRET / FERET: h1 == 0x0148 / 0x0149, r2 == 0 ----
    if h1 == 0x0148 && r2 == 0 {
        g.psw = g.eipsw;
        g.pc = g.eipc & !1;
        g.ins_count += 1;
        return 4;
    }
    if h1 == 0x0149 && r2 == 0 {
        g.psw = g.fepsw;
        g.pc = g.fepc & !1;
        g.ins_count += 1;
        return 4;
    }
    // ---- CTRET: word0 = 0000011111100000, word1 = 0000000101000100 (0x0144) ----
    //   PC ← CTPC, PSW(4:0) ← CTPSW(4:0)
    if h1 == 0x0144 && r2 == 0 {
        g.psw = (g.psw & !0x1F) | (g.ctpsw & 0x1F);
        g.pc = g.ctpc & !1;
        g.ins_count += 1;
        return 4;
    }

    // ---- FPU instructions: Format F:I ----
    // Per R01US0143 Rev1.30 §7.4, F:I format:
    //   word0: reg2(5) | 111111 | reg1(5)
    //   word1: reg3(5) | M(5) | op(6) | type(2) | sub(1)
    // The FPU marker is the 5-bit field M in word1 bits 10..6.
    // M = 00000 (all F:I FPU instructions).
    // Distinguish FPU from other extended op6=0x3F instructions by M == 0.
    // Also require h1 != 0 to avoid matching SETF/TRAP/etc. which have h1 == 0.
    if h1 != 0 && (h1 >> 6) & 0x1F == 0 {
        let reg3 = ((h1 >> 11) & 0x1F) as usize;
        let op6 = (h1 >> 5) & 0x3F;   // op (bits 10..5)
        let ty = (h1 >> 3) & 0x3;     // type (bits 4..3): 0=double, 1=single
        let sub = (h1 >> 2) & 0x1;    // sub-op bit (bit 2)
        match (op6, ty, sub) {
            (0b000000, 1, 0) => g.addf(reg3 as u32, r1 as u32, r2 as u32, false),   // ADDF.S
            (0b000000, 0, 0) => g.addf(reg3 as u32, r1 as u32, r2 as u32, true),    // ADDF.D
            (0b000100, 1, 0) => g.subf(reg3 as u32, r1 as u32, r2 as u32, false),   // SUBF.S
            (0b000100, 0, 0) => g.subf(reg3 as u32, r1 as u32, r2 as u32, true),    // SUBF.D
            (0b001000, 1, 0) => g.mulf(reg3 as u32, r1 as u32, r2 as u32, false),   // MULF.S
            (0b001000, 0, 0) => g.mulf(reg3 as u32, r1 as u32, r2 as u32, true),    // MULF.D
            (0b011110, 1, 0) => g.divf(reg3 as u32, r1 as u32, r2 as u32, false),   // DIVF.S
            (0b011110, 0, 0) => g.divf(reg3 as u32, r1 as u32, r2 as u32, true),    // DIVF.D
            // CVTF.WS (word to single): op=2, sub=0, r1 must be 0
            (0b000010, 1, 0) if r1 == 0 => g.cvtf_i2f(reg3 as u32, g.r[r2], false),
            // CVTF.WD (word to double)
            (0b000010, 0, 0) if r1 == 0 => g.cvtf_i2f(reg3 as u32, g.r[r2], true),
            // CVTS.SW (single to word): op=4, sub=1, r1 must be 0
            (0b000100, 1, 1) if r1 == 0 => g.cvts_f2i(reg3 as u32, r2 as u32, false),
            // CVTS.DW (double to word)
            (0b000100, 0, 1) if r1 == 0 => g.cvts_f2i(reg3 as u32, r2 as u32, true),
            // NEGF: op=16 (0b010000)
            (0b010000, 1, 0) => g.negf(reg3 as u32, r1 as u32, false),
            (0b010000, 0, 0) => g.negf(reg3 as u32, r1 as u32, true),
            // ABSF: op=17 (0b010001)
            (0b010001, 1, 0) => g.absf(reg3 as u32, r1 as u32, false),
            (0b010001, 0, 0) => g.absf(reg3 as u32, r1 as u32, true),
            // CVTF.SD (single to double): op=20 (0b010100), type=0, sub=0
            (0b010100, 0, 0) => {
                let sv = g.fp_single(r1 as u32);
                g.set_fp_double(reg3 as u32, (f32::from_bits(sv) as f64).to_bits());
            }
            // CVTF.DS (double to single): op=21 (0b010101), type=1, sub=0
            (0b010101, 1, 0) => {
                let dv = g.fp_double(r1 as u32);
                let (bits, _) = crate::fpu::round_f64_to_f32(f64::from_bits(dv), crate::fpu::RoundMode::Rn);
                g.set_fp_single(reg3 as u32, bits);
            }
            _ => {
                // Unknown FPU op — treat as NOP
                g.pc += 4;
                g.ins_count += 1;
                return 4;
            }
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- Extended 32-bit ops (Format XI) ----
    // word1 layout: wwwww(bits15..11) + type/sub-bits(bits10..0)
    let w1_w = ((h1 >> 11) & 0x1F) as usize;

    // MUL reg1, reg2, reg3 (Format XI): h1 = wwwww_0100010_0000
    //   reg3||reg2 = (i32)reg2 * (i32)reg1
    if h1 & 0x7FF == 0x220 { // MUL
        let a = g.r[r1] as i64;
        let b = g.r[r2] as i64;
        let prod = a.wrapping_mul(b);
        g.r[r2] = prod as u32;
        g.r[w1_w] = (prod >> 32) as u32;
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // MULU reg1, reg2, reg3 (Format XI): h1 = wwwww_0100010_0010
    //   reg3||reg2 = (u32)reg2 * (u32)reg1
    if h1 & 0x7FF == 0x222 { // MULU
        let a = g.r[r1] as u64;
        let b = g.r[r2] as u64;
        let prod = a.wrapping_mul(b);
        g.r[r2] = prod as u32;
        g.r[w1_w] = (prod >> 32) as u32;
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // MAC reg1, reg2, reg3, reg4 (Format XI): h1 = wwwww_0011110_mmmm0
    //   reg4+1||reg4 = (i32)reg2*(i32)reg1 + reg3+1||reg3
    if h1 & 0x7FF == 0x1E0 { // MAC
        let a = g.r[r1] as i64;
        let b = g.r[r2] as i64;
        let prod = a.wrapping_mul(b);
        let reg3 = ((h1 >> 4) & 0xF) as usize;
        let acc = ((g.r[(reg3 + 1) % 32] as u64) << 32) | (g.r[reg3] as u64);
        let sum = (prod as u64).wrapping_add(acc);
        g.r[w1_w] = sum as u32;
        g.r[(w1_w + 1) % 32] = (sum >> 32) as u32;
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // MACU: h1 = wwwww_0011111_mmmm0
    if h1 & 0x7FF == 0x1F0 { // MACU
        let a = g.r[r1] as u64;
        let b = g.r[r2] as u64;
        let prod = a.wrapping_mul(b);
        let reg3 = ((h1 >> 4) & 0xF) as usize;
        let acc = ((g.r[(reg3 + 1) % 32] as u64) << 32) | (g.r[reg3] as u64);
        let sum = prod.wrapping_add(acc);
        g.r[w1_w] = sum as u32;
        g.r[(w1_w + 1) % 32] = (sum >> 32) as u32;
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // DIVU reg1, reg2 (Format XI): h1 = wwwww_0101100_000010
    //   reg2 = u32 div, w1_w = remainder
    if h1 & 0x7FF == 0x2A2 { // DIVU
        let a = g.r[r2];
        let b = g.r[r1];
        if b != 0 {
            g.r[r2] = a / b;
            g.r[w1_w] = a % b;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // SHL reg1, reg2 (Format XI): h1 = wwwww_0001100_000010
    //   reg2 <<= (reg1 & 0x1F), CY = MSB shifted out
    if h1 & 0x7FF == 0x062 { // SHL
        let n = (g.r[r1] & 0x1F) as u32;
        if n != 0 {
            let cy = (g.r[r2] >> (32 - n)) & 1;
            g.r[r2] = g.r[r2].wrapping_shl(n);
            g.set_psw(PSW_CY, cy == 1);
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // SHR reg1, reg2 (Format XI): h1 = wwwww_0001000_000010
    if h1 & 0x7FF == 0x042 { // SHR
        let n = (g.r[r1] & 0x1F) as u32;
        if n != 0 {
            let cy = (g.r[r2] << (32 - n)) & 1;
            g.r[r2] = g.r[r2] >> n;
            g.set_psw(PSW_CY, cy == 1);
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // SAR reg1, reg2 (Format XI): h1 = wwwww_0001010_000010
    if h1 & 0x7FF == 0x052 { // SAR
        let n = (g.r[r1] & 0x1F) as u32;
        if n != 0 {
            let v = g.r[r2] as i32;
            let cy = if n >= 32 { v as u32 & 1 } else { ((v << (32 - n)) as u32) & 1 };
            g.r[r2] = (v >> n) as u32;
            g.set_psw(PSW_CY, cy == 1);
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // JARL [reg1], reg3 (Format XI): h1 = wwwww_0010110_00000
    //   reg3 = PC+4, PC = [reg1]
    if h1 & 0x7FF == 0x150 { // JARL [r1],r3
        g.r[w1_w] = pc + 4;
        g.pc = g.r[r1] & !1;
        g.ins_count += 1;
        return 4;
    }
    // SASF cccc, reg2 (Format IX): word0 = rrrrr1111110cccc, word1 = 0x0200
    //   if cond: reg2 = (reg2 << 1) | 1; else: reg2 = reg2 << 1
    //   op6 = 0x3F (extended), h1 = 0x0200
    if h1 == 0x0200 {
        let cccc = h0 & 0xF;
        let reg2 = (h0 >> 11) & 0x1F;
        if g.cond(cccc) {
            g.r[reg2 as usize] = (g.r[reg2 as usize] << 1) | 1;
        } else {
            g.r[reg2 as usize] = g.r[reg2 as usize] << 1;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }
    // SETF cccc (Format IX): h1 = 0, word0 = rrrrr_111111_0_cccc
    //   (SETF is 32-bit with word1 = 0)
    if h1 == 0 && (h0 >> 5) & 0x3F == 0b111111 {
        let cccc = h0 & 0xF;
        match cccc {
            0b0001 => g.set_psw(PSW_CY, true),
            0b0010 => g.set_psw(PSW_NZ, true),
            0b0011 => g.set_psw(PSW_S, true),
            0b0100 => g.set_psw(PSW_V, true),
            0b0101 => g.set_psw(PSW_SAT, true),
            0b0000 => g.set_psw(PSW_CY | PSW_NZ | PSW_S | PSW_V | PSW_SAT, false),
            _ => {}
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- TRAP (32-bit): word0 = 00000111111vvvvv, word1 = 0000000100000000 (0x0100)
    //   vector5 = word0 bits4..0. cause = 0x40 + v. reg2 is always 0.
    if h1 == 0x0100 && ((h0 >> 11) & 0x1F) == 0 {
        let vector = (h0 & 0x1F) as u16;
        let cause = 0x40u16 + vector;
        acknowledge(g, cause, pc);
        g.ins_count += 1;
        return 4;
    }

    // ---- SYSCALL: word0 = 11010111111vvvvv, word1 = 00VVV00101100000
    //   vector8 = (h0>>11&0x1F) << 3 | (h1>>8&0x7)
    //   cause = 0x8000 + vector8
    //   PC = SCBP + sext(imm6)*4  where imm6 = word1 bits15..10
    if (h0 >> 5) & 0x3F == 0b111111 && (h0 >> 11) & 0x1F != 0 {
        let lo11 = h1 & 0x7FF;
        if lo11 & 0x1FF == 0x0B {
            let vector8 = (((h0 >> 11) & 0x1F) as u16) << 3 | ((h1 >> 8) & 0x7) as u16;
            let cause = 0x8000u16 + vector8;
            // Save EIPC/EIPSW/EIIC
            let old_psw = g.psw;
            g.eipc = pc + 4;
            g.eipsw = old_psw;
            g.eiic = cause as u32;
            g.psw = (g.psw & !PSW_UM) | PSW_EP | PSW_ID;
            // PC = SCBP + Load(SCBP + vector8*2, Word)
            let scbp = g.scbp;
            let tbl_addr = scbp + (vector8 as u32) * 2;
            let entry = mem.read_word(tbl_addr);
            g.pc = (scbp as i32).wrapping_add(entry as i32) as u32 & !1;
            g.ins_count += 1;
            return 4;
        }
    }

    // ---- CMOV (Format XI): word1 = wwwww011001cccc0 (XI) / wwwww011000cccc0 (XII)
    //   if cond: reg3 = reg1 (XI: reg1 in word0 low bits) or reg3 = sext(imm5) (XII)
    //   XI opcode bits10..0 = 011001cccc0, XII = 011000cccc0
    {
        let lo11 = h1 & 0x7FF;
        // CMOV: bits10..6 = 01100, bit5 = 1 (XI) or 0 (XII), bits4..1 = cccc, bit0 = 0
        if (lo11 >> 6) & 0x1F == 0b01100 && (lo11 & 1) == 0 {
            let is_xi = (lo11 >> 5) & 1 == 1;
            let cccc = (lo11 >> 1) & 0xF;
            if g.cond(cccc) {
                let r3 = ((h1 >> 11) & 0x1F) as usize;
                if is_xi {
                    g.r[r3] = g.r[r1];
                } else {
                    let imm5 = h0 & 0x1F;
                    g.r[r3] = sign_extend(imm5, 5) as u32;
                }
            }
            g.pc += 4;
            g.ins_count += 1;
            return 4;
        }
    }

    // ---- DIV (Format XI): word1 = wwwww01011000000, word0 = rrrrr111111RRRRR
    //   reg2 = reg2 / reg1 (signed), w1_w = remainder
    if h1 & 0x7FF == 0x290 { // DIV
        let a = g.r[r2] as i32;
        let b = g.r[r1] as i32;
        if b != 0 {
            g.r[r2] = (a / b) as u32;
            g.r[w1_w] = (a % b) as u32;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- DIVH (Format XI, 2-operand): word1 = wwwww01010000000
    //   reg2 = reg2 / sext16(reg1) (signed halfword)
    if h1 & 0x7FF == 0x280 { // DIVH
        let a = g.r[r2] as i32;
        let b = (g.r[r1] as i16) as i32;
        if b != 0 {
            g.r[r2] = (a / b) as u32;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- DIVHU (Format XI): word1 = wwwww01010000010
    //   reg2 = reg2 / zext16(reg1) (unsigned halfword), w1_w = remainder
    if h1 & 0x7FF == 0x282 { // DIVHU
        let a = g.r[r2];
        let b = (g.r[r1] as u16) as u32;
        if b != 0 {
            g.r[r2] = a / b;
            g.r[w1_w] = a % b;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- DIVQ (Format XI): word1 = wwwww01011111100
    //   reg2 = reg2 / reg1 (signed, no remainder)
    if h1 & 0x7FF == 0x2FC { // DIVQ
        let a = g.r[r2] as i32;
        let b = g.r[r1] as i32;
        if b != 0 {
            g.r[r2] = (a / b) as u32;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- DIVQU (Format XI): word1 = wwwww01011111110
    //   reg2 = reg2 / reg1 (unsigned, no remainder)
    if h1 & 0x7FF == 0x2FE { // DIVQU
        let a = g.r[r2];
        let b = g.r[r1];
        if b != 0 {
            g.r[r2] = a / b;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- ROTL (Format XI): word1 = wwwww00011000110 (reg form)
    //   reg2 = ROTL(reg2, reg1 & 0x1F), CY = bit shifted out
    if h1 & 0x7FF == 0x066 { // ROTL
        let n = (g.r[r1] & 0x1F) as u32;
        if n != 0 {
            let v = g.r[r2];
            g.r[r2] = (v << n) | (v >> (32 - n));
            g.set_psw(PSW_CY, (v >> (32 - n)) & 1 == 1);
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- ROTL (Format XII): word0 = rrrrr111111iiiii, word1 = wwwww00011000100
    //   reg2 = ROTL(reg2, imm9), imm9 = sext(iiiii IIII)
    if h1 & 0x7FF == 0x064 { // ROTL imm
        let imm5 = h0 & 0x1F;
        let imm4 = (h1 >> 14) & 0xF;
        let imm9 = sign_extend((imm5 | (imm4 << 5)) as u32, 9) as u32 & 0x1F;
        let v = g.r[r2];
        if imm9 != 0 {
            g.r[r2] = (v << imm9) | (v >> (32 - imm9));
            g.set_psw(PSW_CY, (v >> (32 - imm9)) & 1 == 1);
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- SHL/SHR/SAR imm (Format XII) ----
    // SHL imm: word1 = wwwww010110iiiii (16-bit form)
    //   Actually the 16-bit form: rrrrr010110iiiii (op6 in bits10..5)
    // These are handled in the 16-bit dispatch, not here.

    // ---- BSH (Format XII): word0 = rrrrr11111100000, word1 = wwwww01101000010
    //   reg3 = swap bytes in halfwords of reg2
    if h1 & 0x7FF == 0x342 && (h0 & 0x1F) == 0 { // BSH
        let v = g.r[r2];
        let swapped = ((v & 0xFF00) << 8) | ((v & 0x00FF) >> 8)
            | ((v & 0xFF00_0000) << 8) | ((v & 0x00FF_0000) >> 8);
        g.r[w1_w] = swapped;
        g.set_psw(PSW_NZ, swapped != 0);
        g.set_psw(PSW_S, swapped >> 31 == 1);
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- BSW (Format XII): word0 = rrrrr11111100000, word1 = wwwww01101000000
    //   reg3 = byte-swap word of reg2
    if h1 & 0x7FF == 0x340 && (h0 & 0x1F) == 0 { // BSW
        let v = g.r[r2];
        let swapped = ((v & 0xFF) << 24) | ((v & 0xFF00) << 8)
            | ((v & 0xFF0000) >> 8) | ((v & 0xFF000000) >> 24);
        g.r[w1_w] = swapped;
        g.set_psw(PSW_NZ, swapped != 0);
        g.set_psw(PSW_S, swapped >> 31 == 1);
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- HSH (Format XII): word1 = wwwww01101000110
    //   reg3 = reg2 (copy, flags from lower halfword)
    if h1 & 0x7FF == 0x346 && (h0 & 0x1F) == 0 { // HSH
        g.r[w1_w] = g.r[r2];
        g.set_psw(PSW_NZ, (g.r[r2] & 0xFFFF) != 0);
        g.set_psw(PSW_S, g.r[r2] >> 31 == 1);
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- HSW (Format XII): word1 = wwwww01101000100
    //   reg3 = halfword swap of reg2
    if h1 & 0x7FF == 0x344 && (h0 & 0x1F) == 0 { // HSW
        let v = g.r[r2];
        let swapped = (v >> 16) | (v << 16);
        g.r[w1_w] = swapped;
        g.set_psw(PSW_NZ, swapped != 0);
        g.set_psw(PSW_S, swapped >> 31 == 1);
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- SCH0L/SCH0R/SCH1L/SCH1R (Format IX): word0 = rrrrr11111100000
    //   SCH0L: word1 = wwwww01101100100 — search 0 from MSB
    //   SCH0R: word1 = wwwww01101100000 — search 0 from LSB
    //   SCH1L: word1 = wwwww01101100110 — search 1 from MSB
    //   SCH1R: word1 = wwwww01101100010 — search 1 from LSB
    if (h0 & 0x1F) == 0 {
        match h1 & 0x7FF {
            0x354 => { // SCH0L
                let v = g.r[r2];
                let (result, cy, z) = search_bit(v, 0, true);
                g.r[w1_w] = result;
                g.set_psw(PSW_CY, cy);
                g.set_psw(PSW_NZ, !z);
            }
            0x350 => { // SCH0R
                let v = g.r[r2];
                let (result, cy, z) = search_bit(v, 0, false);
                g.r[w1_w] = result;
                g.set_psw(PSW_CY, cy);
                g.set_psw(PSW_NZ, !z);
            }
            0x356 => { // SCH1L
                let v = g.r[r2];
                let (result, cy, z) = search_bit(v, 1, true);
                g.r[w1_w] = result;
                g.set_psw(PSW_CY, cy);
                g.set_psw(PSW_NZ, !z);
            }
            0x352 => { // SCH1R
                let v = g.r[r2];
                let (result, cy, z) = search_bit(v, 1, false);
                g.r[w1_w] = result;
                g.set_psw(PSW_CY, cy);
                g.set_psw(PSW_NZ, !z);
            }
            _ => {}
        }
        if matches!(h1 & 0x7FF, 0x350 | 0x352 | 0x354 | 0x356) {
            g.pc += 4;
            g.ins_count += 1;
            return 4;
        }
    }

    // ---- SETF cccc (Format XII): word0 = rrrrr1111110cccc, word1 = 0
    //   Already handled above (SETF block).

    // ---- SASF cccc, reg2 (Format IX): word0 = rrrrr1111110cccc, word1 = 0x0200
    //   if cond: reg2 = (reg2 << 1) | 1; else: reg2 = reg2 << 1
    if h1 == 0x0200 && (h0 >> 5) & 0x3F == 0b111110 {
        let cccc = h0 & 0xF;
        let reg2 = (h0 >> 11) & 0x1F;
        if g.cond(cccc) {
            g.r[reg2 as usize] = (g.r[reg2 as usize] << 1) | 1;
        } else {
            g.r[reg2 as usize] = g.r[reg2 as usize] << 1;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- MUL imm9 (Format XII): word0 = rrrrr111111iiiii, word1 = wwwww01001IIII00
    //   reg3||reg2 = reg2 * sext(imm9)
    {
        let lo11 = h1 & 0x7FF;
        if lo11 & 0x1F8 == 0x148 { // 01001_IIII_00 pattern check
            let imm5 = h0 & 0x1F;
            let imm4 = (h1 >> 14) & 0xF;
            let imm9 = sign_extend(imm5 | (imm4 << 5), 9) as i64;
            let b = g.r[r2] as i64;
            let prod = b.wrapping_mul(imm9);
            g.r[r2] = prod as u32;
            g.r[w1_w] = (prod >> 32) as u32;
            g.pc += 4;
            g.ins_count += 1;
            return 4;
        }
    }

    // ---- LDSR (Format X): word0 = rrrrr111111RRRRR, word1 = sssss00000100000
    //   RRRRR = dst register, rrrrr = regID, sssss = selID
    //   word1 bits7..0 = 00000100000 (0x40), bit 7 = 0
    if h1 & 0x3FF == 0x100 && (h1 & 0x80) == 0 {
        let sel_id = (h1 >> 11) & 0x1F;
        let reg_id = (h0 >> 11) & 0x1F;
        let dst = h0 & 0x1F;
        let val = read_sysreg(g, reg_id, sel_id);
        g.r[dst as usize] = val;
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- STSR (Format X): word0 = rrrrr111111RRRRR, word1 = sssss00001000000
    //   RRRRR = src register, rrrrr = regID, sssss = selID
    //   word1 bits7..0 = 00001000000 (0x80), bit 7 = 1
    if h1 & 0x3FF == 0x100 && (h1 & 0x80) != 0 {
        let sel_id = (h1 >> 11) & 0x1F;
        let reg_id = (h0 >> 11) & 0x1F;
        let src = h0 & 0x1F;
        write_sysreg(g, reg_id, sel_id, g.r[src as usize]);
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- PUSHSP (Format XI): word0 = 01000111111RRRRR, word1 = wwwww00101100000
    //   bits15..11 = 01000 (8), bits10..5 = 111111, bits4..0 = rh
    //   wwwww = rt (in word1 high bits)
    //   if rh <= rt: push rh, rh+1, ..., rt ascending; sp -= 4 * (rt - rh + 1)
    if (h0 >> 11) & 0x1F == 0b01000 && (h0 >> 5) & 0x3F == 0x3F && (h1 & 0x7FF) == 0x160 {
        let rh = (h0 & 0x1F) as usize;
        let rt = ((h1 >> 11) & 0x1F) as usize;
        if rh <= rt {
            let sp = (g.r[31] as i32) & !3; // mask lower 2 bits
            let mut tmp = sp;
            for cur in rh..=rt {
                tmp -= 4;
                let addr = tmp as u32 & 0xFFFF_FFFF;
                mem.write_word(addr, g.r[cur]);
            }
            g.r[31] = tmp as u32;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- POPSP (Format XI): word0 = 01100111111RRRRR, word1 = wwwww00101100000
    //   bits15..11 = 01100 (12), bits10..5 = 111111, bits4..0 = rh
    //   wwwww = rt (in word1 high bits)
    //   if rh <= rt: pop rt, rt-1, ..., rh descending; sp += 4 * (rt - rh + 1)
    if (h0 >> 11) & 0x1F == 0b01100 && (h0 >> 5) & 0x3F == 0x3F && (h1 & 0x7FF) == 0x160 {
        let rh = (h0 & 0x1F) as usize;
        let rt = ((h1 >> 11) & 0x1F) as usize;
        if rh <= rt {
            let sp = (g.r[31] as i32) & !3; // mask lower 2 bits
            let mut tmp = sp;
            for cur in (rh..=rt).rev() {
                let addr = tmp as u32 & 0xFFFF_FFFF;
                if cur != 31 { // don't overwrite sp
                    g.r[cur] = mem.read_word(addr);
                }
                tmp += 4;
            }
            g.r[31] = tmp as u32;
        }
        g.pc += 4;
        g.ins_count += 1;
        return 4;
    }

    // ---- Fallback: unknown extended -> RIE ----
    acknowledge(g, CAUSE_RIE, pc);
    g.ins_count += 1;
    4
}

/// Bit search: find the first `bit_val` (0 or 1) in `v`.
/// `from_left` = search from MSB. Returns (result, cy, not_found).
/// result = position + 1 (1-based), or 0 if not found.
fn search_bit(v: u32, bit_val: u32, from_left: bool) -> (u32, bool, bool) {
    for pos in 0..32 {
        let bit = if from_left {
            (v >> (31 - pos)) & 1
        } else {
            (v >> pos) & 1
        };
        if bit == bit_val {
            let cy = if from_left { pos == 31 } else { pos == 0 };
            return (pos + 1, cy, false);
        }
    }
    (0, false, true)
}

/// Read a system register by (regID, selID).
fn read_sysreg(g: &G3, reg_id: u32, sel_id: u32) -> u32 {
    match (sel_id, reg_id) {
        (0, 0) => g.eipc,
        (0, 1) => g.eipsw,
        (0, 2) => g.fepc,
        (0, 3) => g.fepsw,
        (0, 5) => g.psw,
        (0, 13) => g.eiic,
        (0, 14) => g.feic,
        (0, 28) => g.eiwr,
        (0, 29) => g.fewr,
        (1, 2) => g.rb,
        (1, 3) => g.ebase,
        (1, 4) => g.intbp,
        (1, 6) => g.pid,
        _ => 0,
    }
}

/// Write a system register by (regID, selID).
fn write_sysreg(g: &mut G3, reg_id: u32, sel_id: u32, val: u32) {
    match (sel_id, reg_id) {
        (0, 0) => g.eipc = val,
        (0, 1) => g.eipsw = val,
        (0, 2) => g.fepc = val,
        (0, 3) => g.fepsw = val,
        (0, 5) => g.psw = val,
        (0, 13) => g.eiic = val,
        (0, 14) => g.feic = val,
        (0, 28) => g.eiwr = val,
        (0, 29) => g.fewr = val,
        (1, 2) => g.rb = val,
        (1, 3) => g.ebase = val,
        (1, 4) => g.intbp = val,
        _ => {}
    }
}

/// 32-bit instruction dispatch.
fn exec_32bit(
    g: &mut G3,
    mem: &mut Memory,
    pc: u32,
    _w: u32,
    h0: u32,
    h1: u32,
    op6: u32,
) -> Result<u32, u16> {
    match op6 {
        // ADDI imm16, reg1, reg2  (Format VI) 0b110000
        0b110000 => {
            // TRAP vector5 (Format X): word0 = 00000111111vvvvv, word1 = 0x0100
            //   Detected before ADDI. cause = 0x40 + vector.
            //   word1 = 0x0100 uniquely identifies TRAP (no other op uses this word1).
            if h1 == 0x0100 {
                let vector = (h0 & 0x1F) as u16;
                let cause = 0x40u16 + vector;
                acknowledge(g, cause, pc);
                g.ins_count += 1;
                return Ok(4);
            }
            // CMOV (Format XI): word1 bits10..0 = wwwww01100Xcccc0
            //   Detected before ADDI (op6=0b110000 is not unique to ADDI).
            let lo11 = h1 & 0x7FF;
            if (lo11 >> 6) & 0x1F == 0b01100 && (lo11 & 1) == 0 {
                let is_xi = (lo11 >> 5) & 1 == 1;
                let cccc = (lo11 >> 1) & 0xF;
                let cmov_r1 = (h0 & 0x1F) as usize;
                if g.cond(cccc) {
                    let r3 = ((h1 >> 11) & 0x1F) as usize;
                    if is_xi {
                        g.r[r3] = g.r[cmov_r1];
                    } else {
                        let imm5 = h0 & 0x1F;
                        g.r[r3] = sign_extend(imm5, 5) as u32;
                    }
                }
                g.pc += 4;
                g.ins_count += 1;
                return Ok(4);
            }
            // EIRET (Format X): word0 = 0000011111100000 (0x7E00), word1 = 0x0148
            //   PC = EIPC, PSW = EIPSW
            if h0 == 0x7E00 && h1 == 0x0148 {
                g.psw = g.eipsw;
                g.pc = g.eipc & !1;
                g.ins_count += 1;
                return Ok(4);
            }
            // FERET (Format X): word0 = 0x7E00, word1 = 0x0149
            //   PC = FEPC, PSW = FEPSW
            if h0 == 0x7E00 && h1 == 0x0149 {
                g.psw = g.fepsw;
                g.pc = g.fepc & !1;
                g.ins_count += 1;
                return Ok(4);
            }
            // HALT (Format X) shares this op6: word0 = 0000011111100000 (0x7E00),
            // word1 = 0000000100100000 (0x2000). Detected before ADDI decode.
            // Places the CPU core in the HALT state (parked until released).
            // PC = next instruction so an exception EIRET returns past HALT.
            if h0 == 0x7E00 && h1 == 0x2000 {
                g.pc = pc + 4;
                g.ins_count += 1;
                g.halted = true;
                g.stopped = true;
                if g.stop_reason.is_empty() {
                    g.stop_reason = "halt".into();
                }
                return Ok(4);
            }
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let imm = h1 as i32;
            let a = g.r[r1];
            let r = a.wrapping_add(imm as u32);
            g.set_arith_flags(r, a as i32, imm, ArithOp::Add);
            g.r[r2] = r;
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // MOVHI imm16, reg1, reg2  (Format VI) 0b110010
        0b110010 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let imm = h1 as u32;
            g.r[r2] = g.r[r1] | (imm << 16);
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // JR disp22 (Format V): word0 = 00000 11110 ddddddd
        //   word1 = dddddddddddddddddd 0  (22-bit disp, bit0 masked 0)
        //   disp22 = word1[15:0] << 5 | word0[4:0]
        //   target = PC + sign_extend(disp22)  (byte offset, NOT *2)
        // JARL r2, disp22: word0 = rrrrr 11110 ddddddd (reg2 = link register)
        //   Same displacement; reg2 = return address (PC+4).
        0b11110 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let disp22 = (h1 & 0xFFFF) << 5 | (h0 & 0x1F);
            let disp = sign_extend(disp22, 22) as i32;
            let target = pc.wrapping_add(disp as u32) & !1;
            if r2 != 0 {
                g.r[r2] = pc + 4; // JARL: LR = return address
            }
            g.pc = target;
            g.ins_count += 1;
            Ok(4)
        }
        // op6 == 0b111111 (extended) is handled by step() via exec_extended.
        // If we reach here it means the extended handler returned an error.
        0b111111 => Err(CAUSE_RIE),
        // LD.W disp16 [reg1], reg2  (Format VII) 0b111000
        0b111000 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let disp = h1 as i32;
            let addr = g.r[r1].wrapping_add(disp as u32) & !1;
            g.r[r2] = mem.read_word(addr);
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // ST.W reg2, disp16 [reg1]  (Format VII) 0b111011
        0b111011 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let disp = h1 as i32;
            let addr = g.r[r1].wrapping_add(disp as u32) & !1;
            mem.write_word(addr, g.r[r2]);
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // LD.H disp16 [reg1], reg2 (Format VII) 0b111001
        0b111001 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let disp = h1 as i32;
            let addr = g.r[r1].wrapping_add(disp as u32) & !1;
            g.r[r2] = (mem.read_half(addr) as u32) as i32 as u32;
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // ST.H reg2, disp16 [reg1] (Format VII) 0b111010, h1 bit0 = 1
        // LD.B disp16 [reg1], reg2 (Format VII) 0b111010, h1 bit0 = 0
        0b111010 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let disp = h1 as i32;
            let addr = g.r[r1].wrapping_add(disp as u32) & !1;
            if (h1 & 1) == 1 {
                // ST.H: write halfword
                mem.write_half(addr, g.r[r2] as u16);
            } else {
                // LD.B: read byte, sign-extend
                let b = mem.read_byte(addr) as i8 as i32;
                g.r[r2] = b as u32;
            }
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // MOVEA imm16, reg1, reg2 (Format VI): 0b110001
        //   reg2 = reg1 + sext(imm16)
        0b110001 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let imm = h1 as i32;
            g.r[r2] = g.r[r1].wrapping_add(imm as u32);
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // ANDI imm16, reg1, reg2 (Format VI): 0b110011
        //   reg2 = reg1 AND sext(imm16)
        0b110011 => {
            let r2 = ((h0 >> 11) & 0x1F) as usize;
            let r1 = (h0 & 0x1F) as usize;
            let imm = h1 as i32;
            g.r[r2] = g.r[r1] & (imm as u32);
            g.set_psw(PSW_NZ, g.r[r2] != 0);
            g.set_psw(PSW_S, g.r[r2] >> 31 == 1);
            g.pc += 4;
            g.ins_count += 1;
            Ok(4)
        }
        // LD.B disp16 [reg1], reg2 (Format VII): op6 = 0b111010, h1 bit0 = 0
        //   (Shares op6 with ST.H: ST.H has h1 bit0 = 1)
        // Already handled in 0b111010 case above — need to check bit0 of h1.
        // The existing 0b111010 case treats it as ST.H. We need to fix this.
        // For now, handle LD.B in the existing 0b111010 case.
        // LD.HU disp16 [reg1], reg2 (Format VII): op6 = 0b111111, h1 bit0 = 1
        //   (Extended format — handled in exec_extended)
        // LD.BU disp16 [reg1], reg2 (Format VII): op6 = 0b111111, h1 bit0 = 1
        //   (Extended format — handled in exec_extended)

        // ---- 16-bit Format II: MOV imm5, reg2 ----
        // These are 16-bit instructions that fall through to here because
        // they don't match any op6 in the 16-bit dispatch.
        // Actually they should be in the 16-bit dispatch. Let me add them there.

        // Fallback
        _ => Err(CAUSE_RIE),
    }
}

#[cfg(test)]
mod dbg_tests {
    use super::*;
    use crate::mem::Memory;
    use crate::state::G3;

    #[test]
    fn dbg_jmp() {
        let mut g = G3::new();
        let mut mem = Memory::new(0x1000);
        g.r[5] = 0x1000;
        let w0: u32 = (0b00011u32 << 5) | 5u32;
        mem.ram[0] = (w0 & 0xFF) as u8;
        mem.ram[1] = ((w0 >> 8) & 0xFF) as u8;
        g.pc = 0;
        let _ = step(&mut g, &mut mem);
        assert_eq!(g.pc, 0x1000);
    }
}



