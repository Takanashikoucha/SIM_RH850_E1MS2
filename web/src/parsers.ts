// Listing (.lst) and map file parsers

export interface LstLine {
  addr: number;
  hex: string;      // raw hex bytes (may be empty)
  asm: string;      // assembly text
  src: string;      // source comment (after //)
}

export interface MapSymbol {
  name: string;
  addr: number;
  size: number;
}

/**
 * Parse a listing file.
 * Expected line format: `ADDR  [HEX ]ASM [//SRC]`
 * - ADDR: 8 hex chars
 * - HEX: optional hex bytes (may be empty)
 * - ASM: assembly text
 * - SRC: source comment after //
 */
export function parseListing(text: string): LstLine[] {
  const lines: LstLine[] = [];
  const re = /^([0-9a-fA-F]{8})\s+(?:(?:[0-9a-fA-F]{2,}\s+)?)(.*)$/;
  for (const raw of text.split("\n")) {
    const line = raw.trimEnd();
    if (!line) continue;
    const m = line.match(re);
    if (!m) continue;
    const addr = parseInt(m[1], 16) >>> 0;
    const rest = m[2].trim();

    // Try to separate hex bytes from asm
    // Hex bytes look like "00 08 01 0E" — space-separated pairs of hex digits
    // The asm text always starts with a letter (uppercase mnemonic)
    // Find the position where the hex bytes end and asm begins
    let hex = "";
    let asmText = rest;
    // Scan through rest, collecting hex byte pairs until we hit a non-hex token
    const tokens = rest.split(/\s+/);
    const hexTokens: string[] = [];
    let asmStart = 0;
    for (let i = 0; i < tokens.length; i++) {
      // A hex byte token is exactly 2 hex digits
      if (/^[0-9a-fA-F]{2}$/.test(tokens[i])) {
        hexTokens.push(tokens[i]);
      } else {
        asmStart = i;
        break;
      }
    }
    if (asmStart > 0 && asmStart < tokens.length) {
      hex = hexTokens.join(" ");
      asmText = tokens.slice(asmStart).join(" ");
    } else if (asmStart === tokens.length) {
      // All tokens are hex bytes (no asm text)
      hex = hexTokens.join(" ");
      asmText = "";
    }

    // Extract source comment
    let src = "";
    const srcIdx = asmText.indexOf("//");
    if (srcIdx !== -1) {
      src = asmText.slice(srcIdx + 2).trim();
      asmText = asmText.slice(0, srcIdx).trim();
    }

    lines.push({ addr, hex, asm: asmText, src });
  }
  return lines;
}

/**
 * Parse a linker map file.
 * Expected format:
 *   Symbol Table:
 *     name  ADDRESS  SIZE
 *     name  ADDRESS  SIZE
 * ...
 */
export function parseMap(text: string): MapSymbol[] {
  const symbols: MapSymbol[] = [];
  const lines = text.split("\n");
  let inSymbolTable = false;
  for (const raw of lines) {
    const line = raw.trim();
    if (/^Symbol\s*Table:/i.test(line)) {
      inSymbolTable = true;
      continue;
    }
    if (!inSymbolTable) continue;
    if (!line) {
      // Blank line may end the symbol table
      if (symbols.length > 0) break;
      continue;
    }
    // Skip headers like "Symbol  Address  Size"
    if (/^Symbol\s+Address/i.test(line)) continue;

    // Try: name  hex_addr  decimal_size
    const m = line.match(/^(\S+)\s+([0-9a-fA-F]{4,8})\s+(\d+)\s*$/);
    if (m) {
      symbols.push({
        name: m[1],
        addr: parseInt(m[2], 16) >>> 0,
        size: parseInt(m[3], 10),
      });
      continue;
    }
    // Try: name  hex_addr (no size)
    const m2 = line.match(/^(\S+)\s+([0-9a-fA-F]{4,8})\s*$/);
    if (m2) {
      symbols.push({
        name: m2[1],
        addr: parseInt(m2[2], 16) >>> 0,
        size: 0,
      });
    }
  }
  return symbols;
}
