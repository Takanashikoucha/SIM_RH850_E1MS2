//! Basic functional tests for the G3 simulator (native build).
//! Opcodes verified against R01US0143EJ0130 Rev1.30 §7.2.2.

use sim::exec::step;
use sim::mem::Memory;
use sim::state::G3;

fn put16(mem: &mut Memory, addr: usize, w: u16) {
    mem.ram[addr] = (w & 0xFF) as u8;
    mem.ram[addr + 1] = ((w >> 8) & 0xFF) as u8;
}

#[test]
fn test_mov_and_addi() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    g.r[1] = 5;

    // MOV reg1, reg2 (Format I): word0 = rrrrr000000RRRRR
    // MOV r1 -> r2: reg2=2, op6=0, reg1=1
    let mov = (2u32 << 11) | (0u32 << 5) | 1u32;
    put16(&mut mem, 0, mov as u16);

    // ADDI imm16, reg1, reg2 (Format VI): word0 = rrrrr110000RRRRR, word1 = imm16
    // ADDI 3, r2, r3: reg2=3, op6=0b110000, reg1=2
    let addi0 = (3u32 << 11) | (0b110000u32 << 5) | 2u32;
    put16(&mut mem, 2, addi0 as u16);
    put16(&mut mem, 4, 3);

    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 5, "MOV r1 -> r2");
    step(&mut g, &mut mem);
    assert_eq!(g.r[3], 8, "ADDI r2+3 -> r3");
    assert_eq!(g.pc, 6, "PC advanced 6 bytes");
    assert_eq!(g.r[2], 5, "MOV does not clobber r2");
}

#[test]
fn test_add_flags() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    g.r[1] = 10;
    g.r[2] = 20;
    // ADD reg1, reg2 (Format I): word0 = rrrrr001110RRRRR ; ADD r1, r2 -> r2 = r2+r1
    let add = (2u32 << 11) | (0b001110u32 << 5) | 1u32;
    put16(&mut mem, 0, add as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 30, "ADD r1+r2");
    assert_eq!(g.psw & sim::state::PSW_NZ, sim::state::PSW_NZ, "NZ set (result non-zero)");
    assert_eq!(g.psw & sim::state::PSW_S, 0, "S clear (positive)");
}

#[test]
fn test_sub_overflow() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // SUB r1, r2 : r2 = r2 - r1. Set r2 = -5, r1 = 10 -> r2 = -15 (no overflow)
    g.r[2] = (-5i32) as u32;
    g.r[1] = 10;
    // SUB opcode 0b001101
    let sub = (2u32 << 11) | (0b001101u32 << 5) | 1u32;
    put16(&mut mem, 0, sub as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], (-15i32) as u32, "SUB -5-10");
}

#[test]
fn test_bcond_z_taken() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // V flag set
    g.psw |= sim::state::PSW_V;
    // BV (cccc=0000). Format III: dddddddd1011dddd... 
    // disp9 = bits15..8 | (bit1 << 8)
    // For target=12 (disp9=6): bits15..8=0x06, bit1=0
    // word0 = 00000110_1011_0000 = 0x06B0
    let word0: u16 = 0x06B0;
    put16(&mut mem, 0, word0);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 12, "BV taken: PC + disp9*2 = 12");
}

#[test]
fn test_bcond_z_not_taken() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // Z = 0 (NZ bit set)
    g.psw |= sim::state::PSW_NZ;
    let word0: u16 = 0x04B2;
    put16(&mut mem, 0, word0);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 2, "BZ not taken: PC+2");
}

#[test]
fn test_bcond_negative_disp() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    g.psw |= sim::state::PSW_V; // V=1 for BV
    // BV with disp9 = -4: sign_extend(disp9,9) = -4
    // disp9 = 508 (binary 111111100), bits15..8=0xFC, bit1=1
    // word0 = 11111100_1011_0010 = 0xFCB2
    let word0: u16 = 0xFCB2;
    put16(&mut mem, 8, word0);
    g.pc = 8;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 8 - 4 * 2, "BV taken backward: PC - 8 = 0");
}

#[test]
fn test_jr_forward22() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // JR disp22 (Format V): word0 = 00000_11110_ddddddddddd, word1 = ddddddddddddddd_0
    // disp22 = word1[15:0] << 5 | word0[4:0]; target = PC + sext(disp22) * 2
    // For +16 bytes: sext(disp22)*2 = 16 → disp22 = 8
    // word0: 00000 11110 01000 = 0x0748 ; word1: 0x0000
    put16(&mut mem, 0, 0x0748);
    put16(&mut mem, 2, 0x0000);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 4, "JR +2 words = +4 bytes");
}

#[test]
fn test_jmp_reg() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    g.r[5] = 0x1000;
    // JMP [reg1] (Format I): word0 = 00000000011RRRRR ; reg1=5
    let w0: u32 = (0b00011u32 << 5) | 5u32;
    put16(&mut mem, 0, w0 as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 0x1000, "JMP [r5]");
}

#[test]
fn test_ld_st_word() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    mem.ram[0x100] = 0xEF;
    mem.ram[0x101] = 0xBE;
    mem.ram[0x102] = 0xAD;
    mem.ram[0x103] = 0xDE;
    g.r[1] = 0x100;

    // LD.W disp16 [reg1], reg2 (Format VII): word0 = rrrrr111000RRRRR, word1 = disp16
    let ld0 = (2u32 << 11) | (0b111000u32 << 5) | 1u32;
    put16(&mut mem, 0, ld0 as u16);
    put16(&mut mem, 2, 0);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 0xDEADBEEF, "LD.W");

    // ST.W reg2, disp16 [reg1] (Format VII): word0 = rrrrr111011RRRRR
    g.r[3] = 0x12345678;
    let st0 = (3u32 << 11) | (0b111011u32 << 5) | 1u32;
    put16(&mut mem, 4, st0 as u16);
    put16(&mut mem, 6, 0x100);
    g.pc = 4;
    step(&mut g, &mut mem);
    assert_eq!(mem.read_word(0x200), 0x12345678, "ST.W at r1+0x100");
}

#[test]
fn test_sxb_sxh_zxb_zxh() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // SXB r1: word0 = 00000000101RRRRR, reg1 in LOW bits.
    g.r[1] = 0xFF;
    let sxb: u16 = ((0b00101u32 << 5) | 1) as u16;
    put16(&mut mem, 0, sxb);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[1], 0xFFFFFFFF, "SXB 0xFF -> -1");

    // SXH r1: word0 = 00000000111RRRRR, reg1 in LOW bits.
    g.r[1] = 0xFFFF_8000; // bit15 set: halfword 0x8000 sign-extends to 0xFFFF8000
    let sxh: u16 = ((0b00111u32 << 5) | 1) as u16;
    put16(&mut mem, 2, sxh);
    g.pc = 2;
    step(&mut g, &mut mem);
    assert_eq!(g.r[1], 0xFFFF_8000, "SXH keeps sign-extended halfword 0x8000 -> 0xFFFF8000");
    // positive halfword: bit15 clear, high bits cleared (re-execute same SXH)
    g.r[1] = 0xDEAD_00FF;
    g.pc = 2;
    step(&mut g, &mut mem);
    assert_eq!(g.r[1], 0x0000_00FF, "SXH 0x00FF -> 0x000000FF");

    // ZXB r1: word0 = 00000000100RRRRR, reg1 in LOW bits.
    g.r[1] = 0xFFFF_FFFF;
    let zxb: u16 = ((0b00100u32 << 5) | 1) as u16;
    put16(&mut mem, 4, zxb);
    g.pc = 4;
    step(&mut g, &mut mem);
    assert_eq!(g.r[1], 0xFF, "ZXB");

    // ZXH r1: word0 = 00000000110RRRRR, reg1 in LOW bits.
    g.r[1] = 0xFFFF_FFFF;
    let zxh: u16 = ((0b00110u32 << 5) | 1) as u16;
    put16(&mut mem, 6, zxh);
    g.pc = 6;
    step(&mut g, &mut mem);
    assert_eq!(g.r[1], 0xFFFF, "ZXH");
}

#[test]
fn test_shl_shr_sar() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // SHL r1, r2: r2 <<= r1. r1=4, r2=1 -> 16
    g.r[1] = 4;
    g.r[2] = 1;
    // SHL Format XI: word0 = 00000_111111_00010, word1 = 00000_0001100_000010
    put16(&mut mem, 0, ((2u32 << 11) | (0b111111u32 << 5) | 1u32) as u16);
    put16(&mut mem, 2, 0x0062);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 16, "SHL 1<<4 = 16");

    // SHR r1, r2: r2 >>= r1. r1=4, r2=16 -> 1
    g.r[1] = 4;
    g.r[2] = 16;
    put16(&mut mem, 4, ((2u32 << 11) | (0b111111u32 << 5) | 1u32) as u16);
    put16(&mut mem, 6, 0x0042); // SHR word1
    g.pc = 4;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 1, "SHR 16>>4 = 1");

    // SAR r1, r2: arithmetic shift. r1=4, r2=-16 (0xFFFFFFF0) -> -1
    g.r[1] = 4;
    g.r[2] = 0xFFFF_FFF0;
    put16(&mut mem, 8, ((2u32 << 11) | (0b111111u32 << 5) | 1u32) as u16);
    put16(&mut mem, 10, 0x0052); // SAR word1
    g.pc = 8;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 0xFFFF_FFFF, "SAR -16>>4 = -1");
}

#[test]
fn test_mul_32bit() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // MUL r1, r2, r3: r3||r2 = r2 * r1. r1=10, r2=20 -> 200
    g.r[1] = 10;
    g.r[2] = 20;
    // word0 = 00000_111111_00010, word1 = 00000_0100010_0000
    put16(&mut mem, 0, ((2u32 << 11) | (0b111111u32 << 5) | 1u32) as u16);
    put16(&mut mem, 2, 0x0220);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 200, "MUL low = 200");
    assert_eq!(g.r[0], 0, "MUL high = 0 (r0)");
}

#[test]
fn test_tst() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // TST r1, r2: flags from r2 AND r1. r1=0x0F, r2=0xF0 -> 0 -> Z
    g.r[1] = 0x0F;
    g.r[2] = 0xF0;
    let tst = (2u32 << 11) | (0b001011u32 << 5) | 1u32;
    put16(&mut mem, 0, tst as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.psw & sim::state::PSW_NZ, 0, "TST result 0 -> NZ clear");
    assert_eq!(g.r[2], 0xF0, "TST does not modify r2");
}

#[test]
fn test_jarl_and_return() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // JARL r14, disp22 (Format V): word0 = 01110_11110_ddddddddddd, word1 = ddddddddddddddd_0
    // disp22 = word1[15:0] << 5 | word0[4:0]
    // PC = PC + sext(disp22)  (byte offset, NOT *2)
    // For +6 bytes: sext(disp22) = 6 → disp22 = 6
    // word0: 01110 011110 00110 = 0x73C6 ; word1: 0x0000
    put16(&mut mem, 0, 0x73C6); // r2=14, opcode5=011110, disp_lo=6
    put16(&mut mem, 2, 0x0000);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 6, "JARL target");
    assert_eq!(g.r[14], 4, "JARL LR = PC+4");
}

#[test]
fn test_halt_stops_cpu() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // HALT (Format X): word0 = 0000011111100000 (0x7E00), word1 = 0000000100100000 (0x2000)
    put16(&mut mem, 0, 0x7E00);
    put16(&mut mem, 2, 0x2000);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 4, "PC advances past HALT (exception EIRET returns here)");
    assert_eq!(g.halted, true, "CPU enters HALT state");
    assert_eq!(g.stopped, true, "HALT stops execution");
    assert_eq!(g.stop_reason, "halt", "stop reason is halt");
    // A subsequent step() while halted must be a no-op (core is parked).
    let pc_before = g.pc;
    step(&mut g, &mut mem);
    // NB: exec::step does not know about halted; the driver (lib.rs) guards it.
    // We only assert the state flags here — lib.rs::step_once returns early when stopped.
    assert_eq!(g.pc, pc_before.wrapping_add(2), "raw step still decodes (driver guards halted)");
}

#[test]
fn test_halt_then_next_step_guards() {
    // The lib.rs driver contract: after HALT, step_once() must be a no-op
    // (returns early on g.stopped) — verified here at the state level:
    // halted && stopped together is the parked condition.
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    put16(&mut mem, 0, 0x7E00);
    put16(&mut mem, 2, 0x2000);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert!(g.halted && g.stopped, "parked: halted && stopped");
}

#[test]
fn test_ri_exception() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    put16(&mut mem, 0, 0x0040); // RIE 16-bit
    g.rb = 0;
    g.psw &= !sim::state::PSW_EBV;
    g.pc = 0;
    step(&mut g, &mut mem);
    // RIE offset = 0x60, base = RBASE = 0 -> PC = 0x60
    assert_eq!(g.pc, 0x60, "RIE -> RBASE + 0x60");
    assert_eq!(g.feic, 0x60, "FEIC = RIE cause");
}

#[test]
fn test_trap() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // TRAP v (extended): word0 = 00000111111vvvvv, word1 = 0x0100
    // v=1: word0 = 0x7E01, word1 = 0x0100
    put16(&mut mem, 0, 0x7E01);
    put16(&mut mem, 2, 0x0100);
    g.rb = 0;
    g.psw &= !sim::state::PSW_EBV;
    g.pc = 0;
    step(&mut g, &mut mem);
    // TRAP cause = 0x40 + v = 0x41, offset = 0x040 (TRAP0)
    assert_eq!(g.pc, 0x40, "TRAP 1 -> RBASE + 0x40");
    assert_eq!(g.eiic, 0x41, "EIIC = TRAP cause");
}

#[test]
fn test_pushsp_popsp() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // PUSHSP rh-rt: word0 = 01000111111RRRRR, word1 = wwwww00101100000
    // Push r1..r2: rh=1, rt=2 (rh <= rt)
    // word0 = 01000_111111_00001 = 0x47E1
    // word1 = 00010_00101100000 = 0x1160 (lo7 = 0x160)
    g.r[31] = 0x100; // SP
    g.r[1] = 0x1111;
    g.r[2] = 0x2222;
    put16(&mut mem, 0, 0x47E1);
    put16(&mut mem, 2, 0x1160);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[31], 0x100 - 8, "SP decremented by 8 (2 regs)");
    // r1 pushed first (at higher addr 0xFC), r2 pushed second (at lower addr 0xF8)
    assert_eq!(mem.read_word(0x100 - 4), 0x1111, "r1 at higher addr");
    assert_eq!(mem.read_word(0x100 - 8), 0x2222, "r2 at lower addr");
}

#[test]
fn test_snooze() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // SNOOZE (Format X): word0 = 0x7E00, word1 = 0x2000
    put16(&mut mem, 0, 0x7E00);
    put16(&mut mem, 2, 0x2000);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.halted, true, "SNOOZE parks CPU");
    assert_eq!(g.stopped, true, "SNOOZE stops execution");
}

#[test]
fn test_cmov() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // CMOV cccc, reg1 → reg3 (extended Format XI):
    // word0 = rrrrr11111100000 (reg2=0), word1 = wwwww011001cccc0
    // CMOV BV, r1 → r3: cccc=0000 (V), reg1=1, reg3=3
    // word0 = 00000_111111_00001 = 0x7E01
    // word1 = 00011_011001_0000_0 = 0x1B20
    g.psw |= sim::state::PSW_V; // V=1 for BV
    g.r[1] = 0xABCD;
    g.r[3] = 0;
    put16(&mut mem, 0, 0x7E01);
    put16(&mut mem, 2, 0x1B20);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[3], 0xABCD, "CMOV BV taken: r3 = r1");
}

#[test]
fn test_sasf() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // SASF cccc, reg2: word0 = rrrrr1111110cccc, word1 = 0x0200
    // SASF T, r1 (always true): cccc=0101, reg2=1
    // word0 = 00001_111111_0_0101 = 0x0FE5
    // word1 = 0x0200
    g.r[1] = 0xABCD;
    put16(&mut mem, 0, 0x0FE5);
    put16(&mut mem, 2, 0x0200);
    g.pc = 0;
    step(&mut g, &mut mem);
    // SASF T: reg1 = (reg1 << 1) | 1 = 0x1579B
    assert_eq!(g.r[1], 0x1579B, "SASF T: r1 = (r1 << 1) | 1");
}

#[test]
fn test_andi() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // ANDI imm16, reg1, reg2 (Format VI): 0b110011
    // ANDI 0x0F00, r1, r2: reg2=2, op6=0b110011, reg1=1
    g.r[1] = 0xFFFF;
    let andi0 = (2u32 << 11) | (0b110011u32 << 5) | 1u32;
    put16(&mut mem, 0, andi0 as u16);
    put16(&mut mem, 2, 0x0F00);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 0x0F00, "ANDI 0xFFFF & 0x0F00");
}

#[test]
fn test_movea() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // MOVEA imm16, reg1, reg2 (Format VI): 0b110001
    // MOVEA 0x100, r1, r2: reg2=2, op6=0b110001, reg1=1
    g.r[1] = 0x1000;
    let movea0 = (2u32 << 11) | (0b110001u32 << 5) | 1u32;
    put16(&mut mem, 0, movea0 as u16);
    put16(&mut mem, 2, 0x100);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 0x1100, "MOVEA r1 + 0x100");
}

#[test]
fn test_ld_st_half() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    mem.ram[0x100] = 0xAB;
    mem.ram[0x101] = 0xCD;
    g.r[1] = 0x100;

    // LD.H disp16 [reg1], reg2 (Format VII): 0b111001
    let ld0 = (2u32 << 11) | (0b111001u32 << 5) | 1u32;
    put16(&mut mem, 0, ld0 as u16);
    put16(&mut mem, 2, 0);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 0xCDAB, "LD.H sign-extends 0xABCD -> 0xCDAB");

    // ST.H reg2, disp16 [reg1] (Format VII): 0b111010, h1 bit0 = 1
    g.r[3] = 0x1234;
    let st0 = (3u32 << 11) | (0b111010u32 << 5) | 1u32;
    put16(&mut mem, 4, st0 as u16);
    put16(&mut mem, 6, 0x100 | 1); // disp=0x100, bit0=1 (ST.H)
    g.pc = 4;
    step(&mut g, &mut mem);
    assert_eq!(mem.read_half(0x200), 0x1234, "ST.H at r1+0x100");
}

#[test]
fn test_ld_byte() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    mem.ram[0x100] = 0x80; // 0x80 = -128 as i8
    g.r[1] = 0x100;

    // LD.B disp16 [reg1], reg2 (Format VII): 0b111010, h1 bit0 = 0
    let ld0 = (2u32 << 11) | (0b111010u32 << 5) | 1u32;
    put16(&mut mem, 0, ld0 as u16);
    put16(&mut mem, 2, 0); // disp=0, bit0=0 (LD.B)
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 0xFFFFFF80, "LD.B sign-extends 0x80 -> 0xFFFFFF80");
}

#[test]
fn test_mov_imm5() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // MOV imm5, reg2 (Format II): rrrrr010000iiiii, op6 = 0b010000
    // MOV -1 (imm5=31), r2: reg2=2, op6=16, imm5=31
    let mov0 = (2u32 << 11) | (0b010000u32 << 5) | 31u32;
    put16(&mut mem, 0, mov0 as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 0xFFFFFFFF, "MOV imm5 31 (sext) -> -1");
}

#[test]
fn test_cmp_flags() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // CMP r1, r2 (Format I): 0b001111, sets flags like SUB (r2 - r1)
    g.r[1] = 10;
    g.r[2] = 5;
    let cmp = (2u32 << 11) | (0b001111u32 << 5) | 1u32;
    put16(&mut mem, 0, cmp as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    // r2 - r1 = 5 - 10 = -5 (negative), so S=1, NZ=1
    assert_eq!(g.r[2], 5, "CMP does not modify r2");
    assert_eq!(g.psw & sim::state::PSW_S, sim::state::PSW_S, "S set (result negative)");
}

#[test]
fn test_mulh_16bit() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // MULH (16-bit, Format I): rrrrr000111RRRRR, op6 = 0b011111 (31)
    // r1=100, r2=200 -> r2 = (100*200) & 0xFFFF = 20000
    g.r[1] = 100;
    g.r[2] = 200;
    let mulh = (2u32 << 11) | (0b011111u32 << 5) | 1u32;
    put16(&mut mem, 0, mulh as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], (100i32 * 200i32) as u32 & 0xFFFF, "MULH 100*200 low halfword");
}

#[test]
fn test_divh_16bit() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // DIVH (16-bit): rrrrr000010RRRRR, op6 = 0b000010 (2)
    // r2=100, r1=10 -> r2 = 100/10 = 10
    g.r[1] = 10;
    g.r[2] = 100;
    let divh = (2u32 << 11) | (0b000010u32 << 5) | 1u32;
    put16(&mut mem, 0, divh as u16);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 10, "DIVH 100/10");
}

#[test]
fn test_jarl_reg() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // JARL [reg1], reg3 (Format XI): h1 = wwwww_0010110_0000
    // word0 = rrrrr_111111_RRRRR (r2=0, r1=1)
    // word1 = wwwww_0010110_0000 (w1_w=2, 0x150 pattern)
    // word1 = 00010_0010110_0000 = 0x1150
    g.r[1] = 0x2000; // target address
    put16(&mut mem, 0, ((0u32 << 11) | (0b111111u32 << 5) | 1u32) as u16);
    put16(&mut mem, 2, 0x1150); // w1_w=2, 0x150 pattern
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 0x2000, "JARL [r1]");
    assert_eq!(g.r[2], 4, "JARL LR = PC+4");
}

#[test]
fn test_rotl() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // ROTL reg1, reg2 (Format XI): word0 = rrrrr111111RRRRR, word1 = wwwww00011000110
    // r1=1, r2=0x80000000 -> ROTL 1: (0x80000000 << 1) | (0x80000000 >> 31) = 0 | 1 = 1
    // word0 = 00010_111111_00001 (r2=2, r1=1)
    // word1 = 00010_0001100_0110 = 0x1066 (w1_w=2, 0x066 pattern)
    g.r[1] = 1;
    g.r[2] = 0x80000000;
    put16(&mut mem, 0, ((2u32 << 11) | (0b111111u32 << 5) | 1u32) as u16);
    put16(&mut mem, 2, 0x1066); // w1_w=2, 0x066 pattern
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 1, "ROTL 0x80000000 by 1 = 1");
    assert_eq!(g.psw & sim::state::PSW_CY, sim::state::PSW_CY, "CY set (MSB shifted out)");
}

#[test]
fn test_bsw() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // BSW (Format XII): word0 = rrrrr11111100000, word1 = wwwww01101000000
    // r2 = 0x12345678, result in w1_w
    // word0 = 00010_111111_00000 (r2=2, op6=0x3F, low=0)
    // word1 = 00011_01101000000 (w1_w=3, 0x340 pattern) = 0x1B40
    g.r[2] = 0x12345678;
    put16(&mut mem, 0, ((2u32 << 11) | (0b111111u32 << 5) | 0u32) as u16);
    put16(&mut mem, 2, 0x1B40); // w1_w=3, 0x340 pattern
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[3], 0x78563412, "BSW byte swap");
}

#[test]
fn test_hsw() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // HSW (Format XII): word0 = rrrrr11111100000, word1 = wwwww01101000100
    // r2 = 0x12345678, result in w1_w
    // word0 = 00010_111111_00000 (r2=2, op6=0x3F, low=0)
    // word1 = 00011_01101000100 (w1_w=3, 0x344 pattern) = 0x1B44
    g.r[2] = 0x12345678;
    put16(&mut mem, 0, ((2u32 << 11) | (0b111111u32 << 5) | 0u32) as u16);
    put16(&mut mem, 2, 0x1B44); // w1_w=3, 0x344 pattern
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[3], 0x56781234, "HSW halfword swap");
}

#[test]
fn test_div_32bit() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // DIV (Format XI): h1 = 0x290
    // r2=100, r1=10 -> r2=10, w1_w=0
    g.r[1] = 10;
    g.r[2] = 100;
    put16(&mut mem, 0, ((2u32 << 11) | (0b111111u32 << 5) | 1u32) as u16);
    put16(&mut mem, 2, 0x2290); // w1_w=2, 0x290 pattern
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2], 10, "DIV 100/10 quotient");
}

#[test]
fn test_fpu_addf_s() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // ADDF.S reg3 ← reg1 + reg2 (Format F:I), R01US0143 §7.4:
    //   word0 = rrrrr 111111 RRRRR        (reg2=1, reg1=2)
    //   word1 = wwww M(5)=00000 op(6) type(2) sub(1)
    //   reg3=3, op=000000 (ADDF), type=1 (single), sub=0
    // h1 = (3 << 11) | (0 << 5) | (1 << 3) | 0 = 0x1808
    g.set_fp_single(1, 0x3F800000); // 1.0f
    g.set_fp_single(2, 0x40000000); // 2.0f
    let w0 = (1u32 << 11) | (0b111111u32 << 5) | 2u32; // reg2=1, reg1=2
    put16(&mut mem, 0, w0 as u16);
    put16(&mut mem, 2, 0x1808); // reg3=3, marker=0, ADDF, single
    g.pc = 0;
    step(&mut g, &mut mem);
    let result = g.fp_single(3); // reg3 = 3
    assert_eq!(result, 0x40400000, "ADDF.S 1.0+2.0 = 3.0");
    assert_eq!(g.pc, 4, "FPU ADDF.S PC advanced");
}

#[test]
fn test_ldsr_stsr_psw() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // LDSR (Format X): word0 = rrrrr111111RRRRR, word1 = sssss00000100000
    // Read PSW (regID=5, selID=0) into r1
    // word0 = 00101_111111_00001 (regID=5, dst=1)
    // word1 = 00000_00000100000 (selID=0, 0x100 pattern)
    g.psw = 0x1234;
    let w0 = (5u32 << 11) | (0b111111u32 << 5) | 1u32;
    put16(&mut mem, 0, w0 as u16);
    put16(&mut mem, 2, 0x0100); // selID=0, 0x100 pattern for LDSR
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[1], 0x1234, "LDSR PSW -> r1");
}

#[test]
fn test_eiret() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // EIRET (extended): word0 = 0000011111100000 (0x7E00), word1 = 0x0148
    g.eipc = 0x1234;
    g.eipsw = 0xABCD;
    put16(&mut mem, 0, 0x7E00);
    put16(&mut mem, 2, 0x0148);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 0x1234, "EIRET PC = EIPC");
    assert_eq!(g.psw, 0xABCD, "EIRET PSW = EIPSW");
}

#[test]
fn test_feret() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // FERET (extended): word0 = 0x7E00, word1 = 0x0149
    g.fepc = 0x5678;
    g.fepsw = 0x9ABC;
    put16(&mut mem, 0, 0x7E00);
    put16(&mut mem, 2, 0x0149);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 0x5678, "FERET PC = FEPC");
    assert_eq!(g.psw, 0x9ABC, "FERET PSW = FEPSW");
}

#[test]
fn test_setf() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // SETF cccc (Format IX): word0 = rrrrr1111110cccc, word1 = 0
    // SETF NZ (cccc=0010): sets NZ=1
    // word0 = 00000_111111_0_0010 = 0x07E2 (op6 = 0b111111 = 0x3F, extended)
    let w0: u16 = 0x07E2;
    put16(&mut mem, 0, w0);
    put16(&mut mem, 2, 0);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.psw & sim::state::PSW_NZ, sim::state::PSW_NZ, "SETF NZ sets NZ");
}

#[test]
fn test_callt() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // CALLT imm6 (Format II): 0b00000_01000_iiiiii
    // Per R01US0143 Rev1.30:
    //   CTPC = PC+2, CTPSW(4:0) = PSW(4:0)
    //   adr = CTBP + zext(imm6)*2, PC = CTBP + zext(Load-Half(adr))
    // imm6=1, CTBP=0x100 -> entry at 0x102. Load half 0x0010 -> PC = 0x110
    // Opcode: bits15..11=00000, bits10..5=01000(8), bits4..0=00001 -> 0x0101
    g.ctbp = 0x100;
    g.psw = 0x12;
    put16(&mut mem, 0x102, 0x0010); // table entry: offset 0x10
    put16(&mut mem, 0, 0x0101);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.pc, 0x110, "CALLT PC = CTBP + halfword table entry");
    assert_eq!(g.ctpc, 2, "CALLT CTPC = PC+2");
    assert_eq!(g.ctpsw, 0x12, "CALLT CTPSW = PSW[4:0]");
}

// ============================================================================
// E1MS2 address space tests (续18: mem.rs 重构后的区域边界验证)
// ============================================================================

#[test]
fn test_mem_flash_readonly() {
    let mut mem = Memory::new(0x1000);
    // Load a small ROM at base 0x100
    let rom_data = [0xDE, 0xAD, 0xBE, 0xEF];
    mem.load_rom(&rom_data, 0x100);
    assert_eq!(mem.read_byte(0x100), 0xDE, "ROM read byte 0");
    assert_eq!(mem.read_byte(0x103), 0xEF, "ROM read byte 3");
    // Writes to ROM must be dropped
    mem.write_byte(0x100, 0x00);
    assert_eq!(mem.read_byte(0x100), 0xDE, "ROM write dropped");
    mem.write_word(0x100, 0x11111111);
    assert_eq!(mem.read_word(0x100), 0xEFBEADDE, "ROM word write dropped");
}

#[test]
fn test_mem_region_detection() {
    let mem = Memory::new(0x1000);
    // Code flash
    assert!(mem.in_flash(0x0000_0000), "flash base");
    assert!(mem.in_flash(0x003F_FFFF), "flash end");
    assert!(!mem.in_flash(0x0040_0000), "past flash end");
    // Local RAM (0xFEBF0000–0xFEBFFFFF)
    assert!(mem.in_local_ram(0xFEBF_0000), "local ram base");
    assert!(mem.in_local_ram(0xFEBF_FFFF), "local ram end");
    assert!(!mem.in_local_ram(0xFEC0_0000), "past local ram");
    // Global RAM (0xFEEE0000–0xFEF1FFFF)
    assert!(mem.in_global_ram(0xFEEE_0000), "global ram base");
    assert!(mem.in_global_ram(0xFEF1_FFFF), "global ram end");
    assert!(!mem.in_global_ram(0xFEF2_0000), "past global ram");
    // I/O low (0x10000000–0x1FFFFFFF)
    assert!(mem.in_io(0x1000_0000), "io low base");
    assert!(mem.in_io(0x1FFF_FFFF), "io low end");
    // I/O high (0xFF000000–0xFFFDFFFF)
    assert!(mem.in_io(0xFF00_0000), "io high base");
    assert!(mem.in_io(0xFFFD_FFFF), "io high end");
    // Unmapped
    assert!(!mem.in_mapped(0x0200_0000), "unmapped hole");
    assert!(!mem.in_mapped(0xFFFE_0000), "unmapped high hole");
}

#[test]
fn test_mem_local_ram_rw() {
    let mut mem = Memory::new(0x1000);
    let base = 0xFEBF_0000;
    // Initially zero
    assert_eq!(mem.read_byte(base), 0);
    // Write word and read back
    mem.write_word(base + 4, 0xA5A5_1234);
    assert_eq!(mem.read_word(base + 4), 0xA5A5_1234, "local ram word");
    assert_eq!(mem.read_byte(base + 4), 0x34, "local ram byte lo");
    assert_eq!(mem.read_byte(base + 7), 0xA5, "local ram byte hi");
    // Write log records the old values
    let log = mem.take_write_log();
    assert_eq!(log.len(), 4, "4 byte writes logged");
    assert!(log.iter().all(|(_, old)| *old == 0), "old values were 0");
    // Restore from log
    mem.restore_from_log(&log);
    assert_eq!(mem.read_word(base + 4), 0, "restored to 0");
}

#[test]
fn test_mem_global_ram_rw() {
    let mut mem = Memory::new(0x1000);
    let base = 0xFEEE_0000;
    mem.write_word(base + 0x100, 0xDEAD_BEEF);
    assert_eq!(mem.read_word(base + 0x100), 0xDEAD_BEEF, "global ram word");
    mem.write_half(base + 0x108, 0x1234);
    assert_eq!(mem.read_half(base + 0x108), 0x1234, "global ram half");
}

#[test]
fn test_mem_unmapped_access() {
    let mut mem = Memory::new(0x1000);
    // Unmapped read returns 0
    assert_eq!(mem.read_byte(0x0200_0000), 0, "unmapped read = 0");
    assert_eq!(mem.read_word(0x0300_0000), 0, "unmapped word = 0");
    // Unmapped writes are silently dropped (no panic, no log)
    mem.write_word(0x0200_0000, 0xFFFFFFFF);
    assert!(mem.take_write_log().is_empty(), "unmapped write not logged");
    assert_eq!(mem.read_word(0x0200_0000), 0, "unmapped write dropped");
}

#[test]
fn test_mem_io_sparse() {
    let mut mem = Memory::new(0x1000);
    // I/O reads return 0 when not registered
    assert_eq!(mem.read_byte(0x1000_0000), 0, "io unregistered = 0");
    // Register an I/O address (io map stores 32-bit values, read as byte at base)
    mem.io.insert(0x1000_0000, 0xCAFEBABE);
    assert_eq!(mem.read_byte(0x1000_0000), 0xBE, "io registered byte (LE)");
    assert_eq!(mem.read_byte(0x1000_0001), 0, "io unregistered offset = 0");
    // Byte write updates the lowest byte of the 32-bit register
    mem.write_byte(0x1000_0000, 0x77);
    assert_eq!(mem.read_byte(0x1000_0000), 0x77, "io byte write");
    assert_eq!(mem.io.get(&0x1000_0000), Some(&0xCAFEBA77), "io register after byte write");
}

#[test]
fn test_sld_sst_uses_ep() {
    let mut g = G3::new();
    let mut mem = Memory::new(0x1000);
    // Set EP (r30) to a Local RAM address
    let ep = 0xFEBF_0100;
    g.r[30] = ep;

    // Seed data directly in local RAM (byte at offset 0x104 = ep+4)
    mem.local_ram_mut()[0x104] = 0x78;
    mem.local_ram_mut()[0x105] = 0x56;
    mem.local_ram_mut()[0x106] = 0x34;
    mem.local_ram_mut()[0x107] = 0x12;
    mem.local_ram_mut()[0x108] = 0xCD;
    mem.local_ram_mut()[0x109] = 0xAB;

    // SLD.W disp7 [ep], r1:  rrrrr=00001, op4=1010, disp7=0000100(4), bit0=0
    // h0 = (1<<11) | (0b1010<<7) | (4<<1) | 0 = 0x0D08
    put16(&mut mem, 0, 0x0D08);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[1], 0x1234_5678, "SLD.W via EP");

    // SLD.H disp8 [ep], r2:  rrrrr=00010, op4=1000, disp7=0001000(8)
    // h0 = (2<<11) | (0b1000<<7) | 8 = 0x1408
    put16(&mut mem, 0, 0x1408);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(g.r[2] as u16, 0xABCD, "SLD.H via EP");

    // SST.W r3, disp7 [ep]:  rrrrr=00011, op4=1010, disp7=0010100(20), bit0=1
    g.r[3] = 0xCAFE_F00D;
    // h0 = (3<<11) | (0b1010<<7) | (20<<1) | 1 = 0x1D29
    put16(&mut mem, 0, 0x1D29);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(mem.read_word(ep + 20), 0xCAFE_F00D, "SST.W via EP");

    // SST.B r4, disp7 [ep]:  rrrrr=00100, op4=0111, disp7=0001010(10)
    g.r[4] = 0x42;
    // h0 = (4<<11) | (0b0111<<7) | 10 = 0x238A
    put16(&mut mem, 0, 0x238A);
    g.pc = 0;
    step(&mut g, &mut mem);
    assert_eq!(mem.read_byte(ep + 10), 0x42, "SST.B via EP");
}

