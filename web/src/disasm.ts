/**
 * Disassembler for RH850G3MH (E1M-S2 CPU1).
 *
 * JS-side decoder using verified opcode layouts from the G3MH software manual.
 * Returns instruction text, byte length, and a kind tag (for color-coding /
 * step-over call detection).
 *
 * NOTE: opcode encodings are mirrored from sim/src/exec.rs — keep both in
 * sync when changing any encoding.
 */

export interface DisasmResult {
  text: string;
  bytes: number;
  kind: "arith" | "logic" | "load" | "store" | "branch" | "call" | "ret" | "system" | "fpu" | "data" | "unknown";
}

const COND: Record<number, string> = {
  0x0: "BV", 0x1: "BC", 0x2: "BZ", 0x3: "BCZ",
  0x4: "BS", 0x5: "BT", 0x6: "BLE", 0x7: "BLS",
  0x8: "BNV", 0x9: "BNL", 0xa: "BNZ", 0xb: "BNCZ",
  0xc: "BNS", 0xd: "BSAT", 0xe: "BGT", 0xf: "BGS",
};

function sext(v: number, bits: number): number {
  const mask = 1 << bits;
  if (v & (mask >> 1)) return v - mask;
  return v;
}

function r(n: number): string {
  return "r" + n;
}

function hex(n: number): string {
  return n.toString(16).padStart(8, "0") + "h";
}

/**
 * Decode one instruction at `off` (byte offset into the buffer).
 * `buf` is a Uint8Array view of the code region.
 */
export function disasm(buf: Uint8Array, off: number): DisasmResult {
  const h0 = buf[off] | (buf[off + 1] << 8);
  const h1 = buf[off + 2] | (buf[off + 3] << 8);
  const op6 = (h0 >> 5) & 0x3f;
  const reg2 = (h0 >> 11) & 0x1f;
  const reg1 = h0 & 0x1f;

  // --- 16-bit special formats ---
  if (h0 === 0x0000) return { text: "NOP", bytes: 2, kind: "system" };
  if (h0 === 0x0040) return { text: "RIE", bytes: 2, kind: "system" };

  // Extended (op6 = 0b111111)
  if (op6 === 0x3f) return disasmExtended(h0, h1);

  // Bcond (Format III): ddddd1011dddcccc
  // disp9 = bits15..8 (8 bits) | bit1 (1 bit) → 9-bit, bit0 masked 0
  if (((h0 >> 4) & 0xf) === 0xb) {
    const cond = h0 & 0xf;
    const disp9 = ((h0 >> 8) & 0xff) | (((h0 >> 1) & 1) << 8);
    const disp = sext(disp9, 9) * 2;
    // Buffer offset must be kept non-negative (Math.max), then wrapped to a
    // u32 target — the offset is the address relative to buffer start, while
    // the hardware target is PC + disp (an absolute u32 arithmetic value).
    const tgt = (Math.max(0, off + disp)) >>> 0;
    const name = COND[cond] ?? "B??" + cond.toString(16);
    return { text: `${name} ${hex(tgt)}`, bytes: 2, kind: "branch" };
  }

  // --- 16-bit Format I (reg-reg / data) ---
  switch (op6) {
    case 0b001110: return { text: `ADD ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "arith" };
    case 0b001101: return { text: `SUB ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "arith" };
    case 0b001010: return { text: `AND ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "logic" };
    case 0b001000: return { text: `OR ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "logic" };
    case 0b001001: return { text: `XOR ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "logic" };
    case 0b000000: return { text: `MOV ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "data" };
    case 0b001111: return { text: `CMP ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "arith" };
    case 0b000011: return { text: `JMP [${r(reg1)}]`, bytes: 2, kind: "branch" };
    case 0b000001: return { text: `NOT ${r(reg2)}`, bytes: 2, kind: "logic" };
    case 0b011111: return { text: `MULH ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "arith" };
    case 0b000010: return { text: `DIVH ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "arith" };
    case 0b00100:  return { text: `ZXB ${r(reg1)}`, bytes: 2, kind: "data" };
    case 0b00101:  return { text: `SXB ${r(reg1)}`, bytes: 2, kind: "data" };
    case 0b00110:  return { text: `ZXH ${r(reg1)}`, bytes: 2, kind: "data" };
    case 0b00111:  return { text: `SXH ${r(reg1)}`, bytes: 2, kind: "data" };
    case 0b001011: return { text: `TST ${r(reg2)}, ${r(reg1)}`, bytes: 2, kind: "logic" };
    // MOV imm5, reg2 (Format II): rrrrr010000iiiii
    // CALLT imm6 (Format II): 00000010000iiiiii
    case 0b010000: {
      if (((h0 >> 6) & 0x3F) === 0) {
        const imm6 = sext(h0 & 0x3F, 6);
        return { text: `CALLT ${hex(imm6)}`, bytes: 2, kind: "branch" };
      }
      const imm5 = sext(reg1, 5);
      return { text: `MOV ${r(reg2)}, ${imm5}`, bytes: 2, kind: "data" };
    }
    // MULH imm5, reg2 (Format II): rrrrr010111iiiii
    case 0b010111: {
      const imm5 = sext(reg1, 5);
      return { text: `MULH ${r(reg2)}, ${imm5}`, bytes: 2, kind: "arith" };
    }
  }

  // --- SLD/SST 16-bit (Format IV): op6 in 20..27 ---
  if (op6 >= 20 && op6 <= 27) {
    const op4 = (h0 >> 7) & 0xf;
    const disp7 = (h0 >> 1) & 0x7f;
    const disp = sext(disp7, 7);
    switch (op4) {
      case 0b0110: return { text: `SLD.B ${disp}, [${r(0)}] → ${r(reg2)}`, bytes: 2, kind: "load" };
      case 0b0111: return { text: `SST.B ${r(reg2)}, ${disp}, [${r(0)}]`, bytes: 2, kind: "store" };
      case 0b1000: return { text: `SLD.H ${disp}, [${r(0)}] → ${r(reg2)}`, bytes: 2, kind: "load" };
      case 0b1001: return { text: `SST.H ${r(reg2)}, ${disp}, [${r(0)}]`, bytes: 2, kind: "store" };
      case 0b1010:
        if ((h0 & 1) === 0) return { text: `SLD.W ${disp}, [${r(0)}] → ${r(reg2)}`, bytes: 2, kind: "load" };
        else return { text: `SST.W ${r(reg2)}, ${disp}, [${r(0)}]`, bytes: 2, kind: "store" };
      case 0b0000:
        if ((h0 & 1) === 1) return { text: `SLD.BU ${disp}, [${r(0)}] → ${r(reg2)}`, bytes: 2, kind: "load" };
        else return { text: `SLD.HU ${disp}, [${r(0)}] → ${r(reg2)}`, bytes: 2, kind: "load" };
    }
  }

  // --- 32-bit instructions ---
  switch (op6) {
    case 0b110000: {
      // HALT (Format X) shares op6 with ADDI
      if (h0 === 0x7e00 && h1 === 0x2000) return { text: "HALT", bytes: 4, kind: "system" };
      const imm = sext(h1, 16);
      return { text: `ADDI ${imm}, ${r(reg1)}, ${r(reg2)}`, bytes: 4, kind: "arith" };
    }
    case 0b110010: {
      const imm = sext(h1, 16);
      return { text: `MOVHI ${imm}, ${r(reg1)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    }
    case 0b110001: {
      const imm = sext(h1, 16);
      return { text: `MOVEA ${imm}, ${r(reg1)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    }
    case 0b110011: {
      const imm = sext(h1, 16);
      return { text: `ANDI ${imm}, ${r(reg1)}, ${r(reg2)}`, bytes: 4, kind: "logic" };
    }
    // JR / JARL disp22 (Format V)
    case 0b11110: {
      const disp22 = ((h1 & 0xffff) << 5) | (h0 & 0x1f);
      const disp = sext(disp22, 22) * 2;
      // off is a buffer-relative offset (≥0); disp may be large and negative.
      // Clamp the offset to ≥0 to keep the index sane, then treat the target
      // as a u32 absolute address (PC + disp in hardware u32 arithmetic).
      const tgt = (Math.max(0, off + disp)) >>> 0;
      if (reg2 !== 0) return { text: `JARL ${hex(tgt)}, ${r(reg2)}`, bytes: 4, kind: "call" };
      return { text: `JR ${hex(tgt)}`, bytes: 4, kind: "branch" };
    }
    case 0b111000: {
      const disp = sext(h1, 16);
      return { text: `LD.W ${disp}, [${r(reg1)}] → ${r(reg2)}`, bytes: 4, kind: "load" };
    }
    case 0b111011: {
      const disp = sext(h1, 16);
      return { text: `ST.W ${r(reg2)}, ${disp}, [${r(reg1)}]`, bytes: 4, kind: "store" };
    }
    case 0b111001: {
      const disp = sext(h1, 16);
      return { text: `LD.H ${disp}, [${r(reg1)}] → ${r(reg2)}`, bytes: 4, kind: "load" };
    }
    case 0b111010: {
      const disp = sext(h1, 16);
      if ((h1 & 1) === 1) return { text: `ST.H ${r(reg2)}, ${disp}, [${r(reg1)}]`, bytes: 4, kind: "store" };
      return { text: `LD.B ${disp}, [${r(reg1)}] → ${r(reg2)}`, bytes: 4, kind: "load" };
    }
  }

  return { text: `?? ${h0.toString(16).padStart(4, "0")} ${h1.toString(16).padStart(4, "0")}`, bytes: 2, kind: "unknown" };
}

/** Decode extended (op6=0b111111) instructions. */
function disasmExtended(h0: number, h1: number): DisasmResult {
  const reg2 = (h0 >> 11) & 0x1f;
  const reg1 = h0 & 0x1f;
  const lo11 = h1 & 0x7ff;
  const w3 = (h1 >> 11) & 0x1f;

  if (lo11 === 0x148 && reg2 === 0 && (h0 & 0x1f) === 0) return { text: "EIRET", bytes: 4, kind: "ret" };
  if (lo11 === 0x14a && reg2 === 0 && (h0 & 0x1f) === 0) return { text: "FERET", bytes: 4, kind: "ret" };
  if (lo11 === 0x144 && reg2 === 0 && (h0 & 0x1f) === 0) return { text: "CTRET", bytes: 4, kind: "ret" };

  if (h0 === 0x8e00 && lo11 === 0x160) return { text: "EI", bytes: 4, kind: "system" };
  if (h0 === 0x7e00 && lo11 === 0x160) return { text: "EID", bytes: 4, kind: "system" };

  if (lo11 === 0x100 && reg2 !== 0) {
    return { text: `TRAP ${reg2}`, bytes: 4, kind: "system" };
  }

  if ((lo11 & 0x1ff) === 0x0b && reg2 !== 0 && ((h0 >> 11) & 0x1f) !== 0) {
    const vec8 = (h0 >> 8) & 0xff;
    return { text: `SYSCALL ${vec8}`, bytes: 4, kind: "system" };
  }

  if (h1 === 0 && ((h0 >> 5) & 0x3f) === 0b111110) {
    const cccc = h0 & 0xf;
    return { text: `SETF ${COND[cccc] ?? "c" + cccc.toString(16)}`, bytes: 4, kind: "system" };
  }

  if (h1 === 0x1000 && ((h0 >> 5) & 0x3f) === 0b111110) {
    const cccc = h0 & 0xf;
    return { text: `SASF ${COND[cccc] ?? "c" + cccc.toString(16)}`, bytes: 4, kind: "system" };
  }

  // FPU Format F:I: word1 = reg3(5)|M(5)=00000|op(6)|type(2)|sub(1)
  //   reg3 = (h1>>11)&0x1F, op = (h1>>5)&0x3F, type = (h1>>3)&0x3, sub = (h1>>2)&0x1
  //   Marker: bits 10..6 == 0 (M field)
  if ((h1 & 0x1F) === 0 && ((h1 >> 6) & 0x1F) === 0) {
    const reg3 = (h1 >> 11) & 0x1f;
    const op = (h1 >> 5) & 0x3f;
    const ty = (h1 >> 3) & 0x3;
    const sub = (h1 >> 2) & 0x1;
    const suf = ty === 1 ? "S" : "D";
    let name: string;
    switch (op) {
      case 0: name = `ADDF.${suf}`; break;
      case 4:
        if (sub === 0) name = `SUBF.${suf}`;
        else name = `CVTS.${suf === "S" ? "W" : "W"}`; // CVTS.W
        break;
      case 8: name = `MULF.${suf}`; break;
      case 2: name = `CVTF.W${suf === "S" ? "S" : "D"}`; break;
      case 0x1e: name = `DIVF.${suf}`; break;
      case 16: name = `NEGF.${suf}`; break;
      case 17: name = `ABSF.${suf}`; break;
      case 20: name = "CVTF.SD"; break;
      case 21: name = "CVTF.DS"; break;
      default: name = `FPU.${op.toString(16)}`; break;
    }
    if (name.startsWith("CVTF") || name.startsWith("CVTS")) {
      return { text: `${name} ${r(reg3)}, ${r(reg1)}`, bytes: 4, kind: "fpu" };
    }
    return { text: `${name} ${r(reg3)}, ${r(reg1)}, ${r(reg2)}`, bytes: 4, kind: "fpu" };
  }

  if (lo11 === 0x2b0 && ((h0 >> 11) & 0x1f) === 0b01000) {
    const rh = h0 & 0x1f;
    const rt = w3;
    return { text: `PUSHSP ${r(rh)}, ${r(rt)}`, bytes: 4, kind: "system" };
  }
  if (lo11 === 0x2b0 && ((h0 >> 11) & 0x1f) === 0b01100) {
    const rh = h0 & 0x1f;
    const rt = w3;
    return { text: `POPSP ${r(rh)}, ${r(rt)}`, bytes: 4, kind: "system" };
  }

  if (h0 === 0x7e00 && lo11 === 0x2000) return { text: "SNOOZE", bytes: 4, kind: "system" };

  const EXT: Record<number, string> = {
    0x220: "MUL", 0x222: "MULU", 0x1e0: "MAC", 0x1f0: "MACU",
    0x290: "DIV", 0x280: "DIVH", 0x282: "DIVHU", 0x2a2: "DIVU",
    0x2fc: "DIVQ", 0x2fe: "DIVQU",
    0x062: "SHL", 0x042: "SHR", 0x052: "SAR", 0x066: "ROTL",
    0x150: "JARL",
  };
  if (EXT[lo11]) {
    const name = EXT[lo11];
    switch (name) {
      case "MUL": case "MULU":
        return { text: `${name} ${r(reg2)}, ${r(reg1)} (hi→${r(w3)})`, bytes: 4, kind: "arith" };
      case "MAC": case "MACU": {
        const m = (h1 >> 4) & 0xf;
        return { text: `${name} ${r(reg2)}, ${r(reg1)}, ${r(m)}`, bytes: 4, kind: "arith" };
      }
      case "DIV": case "DIVU": case "DIVQU": case "DIVQ":
        return { text: `${name} ${r(reg2)}, ${r(reg1)} (rem→${r(w3)})`, bytes: 4, kind: "arith" };
      case "DIVH": case "DIVHU":
        return { text: `${name} ${r(reg2)}, ${r(reg1)}${name === "DIVHU" ? ` (rem→${r(w3)})` : ""}`, bytes: 4, kind: "arith" };
      case "SHL": case "SHR": case "SAR": case "ROTL":
        return { text: `${name} ${r(w3)}, ${r(reg1)}, ${r(reg2)}`, bytes: 4, kind: "data" };
      case "JARL":
        return { text: `JARL [${r(reg1)}] → ${r(w3)}`, bytes: 4, kind: "call" };
    }
  }

  if ((lo11 & 0x1f0) === 0x110 || (lo11 & 0x1f0) === 0x118) {
    const cccc = lo11 & 0xf;
    if ((lo11 & 0x1f0) === 0x110) {
      return { text: `CMOV ${COND[cccc] ?? "c" + cccc.toString(16)}, ${r(reg1)} → ${r(w3)}`, bytes: 4, kind: "data" };
    }
    const imm5 = sext(reg1, 5);
    return { text: `CMOV ${COND[cccc] ?? "c" + cccc.toString(16)}, ${imm5} → ${r(w3)}`, bytes: 4, kind: "data" };
  }

  if ((lo11 & 0x1f8) === 0x168) {
    const cccc = lo11 & 0xf;
    return { text: `ADF ${COND[cccc] ?? "c" + cccc.toString(16)}, ${r(reg2)}, ${r(reg1)} → ${r(w3)}`, bytes: 4, kind: "arith" };
  }

  if (reg1 === 0 && ((h0 >> 11) & 0x1f) !== 0) {
    if (lo11 === 0x342) return { text: `BSH ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    if (lo11 === 0x340) return { text: `BSW ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    if (lo11 === 0x346) return { text: `HSH ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    if (lo11 === 0x344) return { text: `HSW ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
  }

  if (reg1 === 0) {
    if (lo11 === 0x354) return { text: `SCH0L ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    if (lo11 === 0x350) return { text: `SCH0R ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    if (lo11 === 0x356) return { text: `SCH1L ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
    if (lo11 === 0x352) return { text: `SCH1R ${r(w3)}, ${r(reg2)}`, bytes: 4, kind: "data" };
  }

  if ((lo11 & 0x3ff) === 0x100 && w3 !== 0) {
    return { text: `LDSR ${r(h0 & 0x1f)} ← SR${w3},${(h0 >> 11) & 0x1f}`, bytes: 4, kind: "system" };
  }
  if ((lo11 & 0x3f) === 0x10 && w3 !== 0) {
    return { text: `STSR SR${w3},${(h0 >> 11) & 0x1f} ← ${r(h0 & 0x1f)}`, bytes: 4, kind: "system" };
  }

  return { text: `?? ${h0.toString(16).padStart(4, "0")} ${h1.toString(16).padStart(4, "0")}`, bytes: 4, kind: "unknown" };
}

export interface DisasmLine {
  addr: number;
  text: string;
  bytes: number;
  kind: DisasmResult["kind"];
}

export function disasmRegion(buf: Uint8Array, off: number, len: number): DisasmLine[] {
  const lines: DisasmLine[] = [];
  let pos = 0;
  while (pos < len - 1) {
    const res = disasm(buf, off + pos);
    lines.push({ addr: off + pos, text: res.text, bytes: res.bytes, kind: res.kind });
    pos += res.bytes;
    if (lines.length >= 1000) break; // safety cap
  }
  return lines;
}
