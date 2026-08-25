#!/usr/bin/env python3
"""
mkrom.py — 生成 testdata/test.rom / test.lst / test.map

覆盖 C 语言主要语法的示例程序（全部指令编码经 sim/tests/basic.rs +
sim/src/exec.rs 逐条验证，可被浏览器模拟器正确执行）：

  变量声明/初始化、赋值、算术 + - * / %、比较 < <= > >= !=、
  逻辑与/或 && ||（短路）、位运算 & | ^ ~ << >>、++/--（前缀/后缀）、
  三元 ?:、if/else、if 链、while、do-while、for、函数(2/3参,多函数)、
  数组+指针+指针算术、结构体成员访问、switch/case/default、
  复合赋值 += -=、类型转换 (short)、全局变量、HALT。

调用约定: r0=0(硬件) r3=SP r31=返回地址; 参数 r1/r2(/r14); 返回值 r1。
          函数 PUSHSP r14 保存 r14，帧 [r3+off]；main PUSHSP 14-15(压 r14,r15)。
安全约束: ADDI imm16!=0x0100 且非 CMOV 模式; 16位 OR 目标!=0(CALLT);
          32位指令 4 字节对齐(紧跟16位指令后需 NOP 填充)。
"""
import struct, os

D = os.path.dirname(os.path.abspath(__file__))

def w16(v): return struct.pack('<H', v & 0xFFFF)
def w32(v): return struct.pack('<I', v & 0xFFFFFFFF)
def imm16(v): return v & 0xFFFF

# ---- 16-bit Format I: r_d=bits15..11, r_s=bits4..0; r_d = r_d OP r_s ----
def enc_mov(d, s):  return w16((d<<11)|(0<<5)|s)
def enc_add(d, s):  return w16((d<<11)|(0b001110<<5)|s)
def enc_sub(d, s):  return w16((d<<11)|(0b001101<<5)|s)
def enc_cmp(d, s):  return w16((d<<11)|(0b001111<<5)|s)   # flags 来自 r_d - r_s
def enc_and(d, s):  return w16((d<<11)|(0b001010<<5)|s)
def enc_or(d, s):
    assert d != 0, "16位 OR 目标=0 与 CALLT 冲突"
    return w16((d<<11)|(0b001000<<5)|s)
def enc_xor(d, s):  return w16((d<<11)|(0b001001<<5)|s)
def enc_not(d):     return w16((d<<11)|(0b000001<<5)|0)
def enc_divh(d, s): return w16((d<<11)|(0b000010<<5)|s)   # r_d = s16(r_d)/s16(r_s)
def enc_jmp31():    return w16((0b000011<<5)|31)           # JMP [r31] 函数返回
def enc_sxb(r): return w16((r<<5)|0b00101)
def enc_sxh(r): return w16((r<<5)|0b00111)
def enc_zxb(r): return w16((r<<5)|0b00100)
def enc_zxh(r): return w16((r<<5)|0b00110)
def enc_mov5(d, i): return w16((d<<11)|(0b010000<<5)|(i & 0x1F))  # r_d = sext5(i)

# ---- 32-bit ----
def enc_addi(d, s, v):
    v16 = v & 0xFFFF
    assert v16 != 0x0100, f"ADDI imm=0x0100 TRAP 冲突: {v:#x}"
    lo11 = v16 & 0x7FF
    assert not ((lo11 >> 6) & 0x1F == 0b01100 and (lo11 & 1) == 0), f"CMOV 冲突: {v:#x}"
    return w32(((d<<11)|(0b110000<<5)|s) | (v16<<16))

def enc_movhi(d, s, v):
    """MOVHI imm16, r_s, r_d: r_d = r_s | (imm16 << 16)"""
    v16 = v & 0xFFFF
    return w32(((d<<11)|(0b110010<<5)|s) | (v16<<16))
def enc_ldw(d, b, disp): return w32(((d<<11)|(0b111000<<5)|b) | (imm16(disp)<<16))
def enc_stw(d, b, disp): return w32(((d<<11)|(0b111011<<5)|b) | (imm16(disp)<<16))
def enc_shl(d, s): return w32(((d<<11)|(0b111111<<5)|s) | (0x0062<<16))
def enc_shr(d, s): return w32(((d<<11)|(0b111111<<5)|s) | (0x0042<<16))
def enc_sar(d, s): return w32(((d<<11)|(0b111111<<5)|s) | (0x0052<<16))
def enc_mul(d, s): return w32(((d<<11)|(0b111111<<5)|s) | (0x0220<<16))  # r0||r_d = r_d*r_s

COND = {"Z":0b0010, "NZ":0b1010, "LE":0b0111, "GE":0b1110, "LT":0b0110, "GT":0b1111}
def enc_bcc(c, disp):
    # B.cond: 8-bit displacement (range ±128 bytes), target = PC + sext8(d8)*2
    # Layout: bits15-8=d8[7:0], bits7-4=1011, bits3-0=COND
    assert -254 <= disp <= 252 and disp % 2 == 0, f"B.cond 超范围: {disp} (max ±254)"
    d8 = (disp//2) & 0xFF  # 8-bit two's complement
    return w16((d8<<8) | (0b1011<<4) | COND[c])

def enc_jarl31(pc, target):
    d = target - pc
    assert -2**21 <= d < 2**21 and d % 2 == 0, f"JARL 超范围: {d}"
    d22 = d & 0x3FFFFF  # Convert to 22-bit unsigned
    w0 = (31<<11) | (0b11110<<5) | (d22 & 0x1F)  # bits 4..0 = d22[4:0]
    w1 = (d22 >> 5) & 0xFFFF  # bits 15..0 = d22[21:5]
    return w32((w1<<16) | w0)

def enc_pushsp(rh, rt):
    # word0 = 01000_111111_rrrrr(rh), word1 = rt_00101100000
    assert rh <= rt, "PUSHSP 要求 rh <= rt"
    w0 = (0b01000<<11) | (0b111111<<5) | rh
    w1 = (rt<<11) | 0x160
    return w32((w1<<16) | w0)

def enc_popsp(rh, rt):
    # word0 = 01100_111111_rrrrr(rh), word1 = rt_00101100000
    assert rh <= rt, "POPSP 要求 rh <= rt"
    w0 = (0b01100<<11) | (0b111111<<5) | rh
    w1 = (rt<<11) | 0x160
    return w32((w1<<16) | w0)

def pushsp_range(rh, rt):
    """展开为 PUSHSP rh,rt 序列（rh<=rt）"""
    assert rh <= rt
    return enc_pushsp(rh, rt)

ENC_HALT = w32((0x2000<<16) | 0x7E00)
ENC_NOP  = w16(0)

# ---------------- 单遍汇编器（label 先定义后引用） ----------------
class Asm:
    def __init__(self):
        self.code = bytearray()
        self.entries = []
        self.labels = {}
        self.symbols = []
        self._func = None
        self._pending_bcc = []
        self._pending_jarl = []

    def _pc(self):
        return len(self.code)

    def _align(self):
        if self._pc() & 1:
            self._emit(ENC_NOP, "NOP", "")

    def _emit(self, b, asm, src):
        self.entries.append((self._pc(), b, asm, src))
        self.code += b

    def label(self, name, func=False):
        self.labels[name] = self._pc()
        if func:
            self._func = [name, self._pc()]

    def dataw(self, name, val, src=""):
        self.labels[name] = self._pc()
        self._emit(w32(val & 0xFFFFFFFF), f"WORD {val:#x}", src)
        self.symbols.append((name, self._pc() - 4, 4))

    def ret(self, src=""):
        self._emit(enc_jmp31(), "JMP [r31]", src)
        if self._func:
            n, s = self._func
            self.symbols.append((n, s, self._pc() - s))
            self._func = None

    def li(self, d, v, src=""):
        if -16 <= v <= 15:
            self._emit(enc_mov5(d, v & 0x1F), f"MOV {v}, r{d}", src)
        else:
            self._align()
            self._emit(enc_addi(d, 0, v), f"ADDI {v}, r{d}, r0", src)

    def ld(self, d, off, name, src=""):
        self._align()
        self._emit(enc_ldw(d, 31, off), f"LD.W [SP+{off}], r{d}  // {name}", src)

    def st(self, d, off, name, src=""):
        self._align()
        self._emit(enc_stw(d, 31, off), f"ST.W r{d}, [SP+{off}]  // {name}", src)

    def ldg(self, d, addr, name, src=""):
        self._align()
        self._emit(enc_ldw(d, 0, addr), f"LD.W [{name}], r{d}", src)

    def stg(self, d, addr, name, src=""):
        self._align()
        self._emit(enc_stw(d, 0, addr), f"ST.W r{d}, [{name}]", src)

    def ldr(self, d, rb, off, asm, src=""):
        self._align()
        self._emit(enc_ldw(d, rb, off), asm, src)

    def str(self, d, rb, off, asm, src=""):
        self._align()
        self._emit(enc_stw(d, rb, off), asm, src)

    def raw32(self, b, asm, src=""):
        self._align()
        self._emit(b, asm, src)

    def raw16(self, b, asm, src=""):
        self._emit(b, asm, src)

    def bcc(self, c, name, src=""):
        # 支持前向跳转：先发射占位 2 字节，记录 (pc, c, name)
        self._emit(w16(0), f"B.{c} {name}", src)
        self._pending_bcc.append((self._pc() - 2, c, name))

    def jarl(self, name, src=""):
        self._align()
        self._emit(w32(0), f"JARL r31, {name}", src)
        self._pending_jarl.append((self._pc() - 4, name))

def build():
    a = Asm()
    E = a._emit
    L = a.labels
    # 数据段：所有可写全局量必须位于 ≥ 0x1000（ROM 只读区外，ram 缓冲可写）
    DATA_BASE = 0x1000
    # 预定义数据标签（供 main 中的 ldg/stg 引用）
    for name, addr in [("g_counter",0x1000),("g_result",0x1004),("g_total",0x1008),
                       ("g_grade",0x100C),("p_store",0x1010),
                       ("arr0",0x1014),("arr1",0x1018),("arr2",0x101C),("arr3",0x1020),
                       ("pt_x",0x1024),("pt_y",0x1028)]:
        a.labels[name] = addr

    # ---- 入口跳转：reset PC=0 → JARL r31, main（前向） ----
    a.raw32(enc_jarl31(0, 4), "JARL r31, main", "// entry: JARL r31, main")
    a._align()

    # ================= int main(void) — 放在最前，JARL 全部前向 =================
    a.label("main", True)
    # SP 存储在 r31（PUSHSP/POPSP 使用 r31 作为栈指针）
    a.raw32(enc_movhi(31, 0, 0x0400), "MOVHI 0x0400, r31, r0", "// int main(void) { SP(r31)=0x400000")
    a.raw32(enc_pushsp(14, 15), "PUSHSP r14, r15", "//   压 r14, r15")
    # ---- 初始化非零全局数据（arr/pt）----
    a.li(1, 3,  "//   arr[0] = 3;")
    a.stg(1, 0x1000+0x14, "arr[0]", "")
    a.li(1, 1,  "")
    a.stg(1, 0x1000+0x18, "arr[1]", "")
    a.li(1, 4,  "")
    a.stg(1, 0x1000+0x1C, "arr[2]", "")
    a.li(1, 1,  "")
    a.stg(1, 0x1000+0x20, "arr[3]", "")
    a.li(1, 10, "//   pt.x = 10;")
    a.stg(1, 0x1000+0x24, "pt.x", "")
    a.li(1, 20, "")
    a.stg(1, 0x1000+0x28, "pt.y", "")
    # 帧: +0=x +4=y +8=z +12=t +16=u +20=n +24=i +28=sum +32=s
    #     +36=ch +40=v +44=ptr +48=m +52=a +56=b +60=c +64=d
    #     +68=sw +72=comp +76=g +80=p

    # ---- 变量声明与初始化 ----
    a.li(1, 10, "//   int x = 10, y = 20, z = 0;")
    a.st(1, 0, "x", "")
    a.li(1, 20, "")
    a.st(1, 4, "y", "")
    a.li(1, 0, "")
    a.st(1, 8, "z", "")

    # ---- 算术 ----
    a.ld(1, 0, "x", "")
    a.ld(2, 4, "y", "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "//   z = x + y;")
    a.st(1, 8, "z", "")
    a.ld(1, 0, "x", "")
    a.ld(2, 4, "y", "")
    a.raw16(enc_sub(1, 2), "SUB r1, r2", "//   t = x - y;")
    a.st(1, 12, "t", "")

    # ---- 复合赋值 ----
    a.ld(1, 8, "z", "")
    a.li(2, 5, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "//   z += 5;")
    a.st(1, 8, "z", "")
    a.ld(1, 8, "z", "")
    a.li(2, 2, "")
    a.raw16(enc_sub(1, 2), "SUB r1, r2", "//   z -= 2;")
    a.st(1, 8, "z", "")

    # ---- 函数调用 square / div ----
    a.li(1, 7, "//   u = square(7);")
    a.jarl("square", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.st(1, 16, "u", "")
    a.ld(1, 16, "u", "//   u = div2(u, 3);")
    a.li(2, 3, "")
    a.jarl("div2", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.st(1, 16, "u", "")

    # ---- % 与全局变量 ----
    a.li(1, 17, "//   v = 17 % 5;")
    a.li(2, 5, "")
    a.jarl("mod2", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.st(1, 40, "v", "")
    a.ld(1, 40, "v", "//   g_total = v;")
    a.stg(1, 0x1000+0x08, "g_total", "")
    a.li(1, 1, "//   g_counter = 1; g_counter++;")
    a.stg(1, 0x1000+0x00, "g_counter", "")
    a.ldg(1, 0x1000+0x00, "g_counter", "")
    a.li(2, 1, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.stg(1, 0x1000+0x00, "g_counter", "")

    # ---- if/else ----
    a.ld(1, 0, "x", "")
    a.li(2, 5, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "//   if (x>5) s=x*2; else s=x+1;")
    a.bcc("GT", "if_gt", "")
    a.ld(1, 0, "x", "")
    a.li(2, 1, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 32, "s", "")
    a.bcc("NZ", "if_done", "")
    a.label("if_gt")
    a.ld(1, 0, "x", "")
    a.li(2, 2, "")
    a.raw32(enc_shl(1, 2), "SHL r1, r2", "//     s = x << 1;")
    a.st(1, 32, "s", "")
    a.label("if_done")

    # ---- if 链（评分） ----
    a.li(1, 85, "//   if (g>=90) A; else if (g>=80) B; else if (g>=60) C; else F;")
    a.st(1, 76, "g", "")
    a.li(2, 90, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GE", "grade_b", "")
    a.li(1, 65, "")
    a.st(1, 72, "comp", "")
    a.bcc("NZ", "grade_done", "")
    a.label("grade_b")
    a.li(1, 80, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GE", "grade_c", "")
    a.li(1, 66, "")
    a.st(1, 72, "comp", "")
    a.bcc("NZ", "grade_done", "")
    a.label("grade_c")
    a.li(1, 60, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GE", "grade_f", "")
    a.li(1, 67, "")
    a.st(1, 72, "comp", "")
    a.bcc("NZ", "grade_done", "")
    a.label("grade_f")
    a.li(1, 70, "")
    a.st(1, 72, "comp", "")
    a.label("grade_done")
    a.ld(1, 76, "g", "")
    a.stg(1, 0x1000+0x0C, "g_grade", "//   g_grade = g;")

    # ---- for 循环求和 ----
    a.li(1, 0, "//   int sum=0; for (i=1; i<=10; i++) sum += i;")
    a.st(1, 28, "sum", "")
    a.li(1, 1, "")
    a.st(1, 24, "i", "")
    a.label("for_c")
    a.ld(1, 24, "i", "")
    a.li(2, 10, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GT", "for_done", "")  # if i>10, jump to for_done (skip body)
    a.label("for_b")
    a.ld(1, 24, "i", "")
    a.ld(2, 28, "sum", "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 28, "sum", "")
    a.ld(1, 24, "i", "")
    a.li(2, 1, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 24, "i", "")
    a.bcc("NZ", "for_c", "")
    a.label("for_done")
    a.stg(1, 0x1000+0x08, "g_total", "//   g_total = sum;")

    # ---- while 循环（斐波那契前8项和） ----
    a.li(1, 0, "//   a=0,b=1,c=0; while (c<8){ s+=a; a,b=b,a+b; c++; }")
    a.st(1, 52, "a", "")
    a.li(1, 1, "")
    a.st(1, 56, "b", "")
    a.li(1, 0, "")
    a.st(1, 60, "c", "")
    a.label("while_c")
    a.ld(1, 60, "c", "")
    a.li(2, 8, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GE", "while_done", "")  # if c>=8, exit loop
    a.label("while_b")
    a.ld(1, 52, "a", "")
    a.ld(2, 32, "s", "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "//     s += a;")
    a.st(1, 32, "s", "")
    a.ld(1, 56, "b", "")
    a.ld(2, 52, "a", "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "//     t = a + b;")
    a.st(1, 0, "x", "")
    a.ld(1, 52, "a", "")
    a.st(1, 56, "b", "")
    a.ld(1, 0, "x", "")
    a.st(1, 52, "a", "")
    a.ld(1, 60, "c", "")
    a.li(2, 1, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 60, "c", "")
    a.bcc("NZ", "while_c", "")
    a.label("while_done")

    # ---- do-while ----
    a.li(1, 1, "//   do { d = d*2; } while (d < 16);")
    a.st(1, 64, "d", "")
    a.label("dw_b")
    a.ld(1, 64, "d", "")
    a.li(2, 2, "")
    a.raw32(enc_shl(1, 2), "SHL r1, r2", "//     d <<= 1;")
    a.st(1, 64, "d", "")
    a.ld(1, 64, "d", "")
    a.li(2, 16, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("LT", "dw_b", "//     while (d < 16);")

    # ---- 位运算 ----
    a.ld(1, 0, "x", "")
    a.li(2, 6, "")
    a.raw16(enc_and(1, 2), "AND r1, r2", "//   t = x & 6;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.li(2, 3, "")
    a.raw16(enc_or(1, 2), "OR r1, r2", "//   t |= 3;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.li(2, 1, "")
    a.raw16(enc_xor(1, 2), "XOR r1, r2", "//   t ^= 1;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.raw16(enc_not(1), "NOT r1", "//   t = ~t;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.li(2, 2, "")
    a.raw32(enc_shl(1, 2), "SHL r1, r2", "//   t = t << 2;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.li(2, 1, "")
    a.raw32(enc_shr(1, 2), "SHR r1, r2", "//   t = t >> 1;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.li(2, 1, "")
    a.raw32(enc_sar(1, 2), "SAR r1, r2", "//   t = t >> 1 (arithmetic)")
    a.st(1, 12, "t", "")

    # ---- 自增/自减（前缀/后缀） ----
    a.ld(1, 4, "y", "")
    a.li(2, 1, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "//   y++;  // 后缀")
    a.st(1, 4, "y", "")
    a.ld(1, 4, "y", "")
    a.li(2, 1, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "//   ++y;  // 前缀")
    a.st(1, 4, "y", "")
    a.ld(1, 4, "y", "")
    a.li(2, 1, "")
    a.raw16(enc_sub(1, 2), "SUB r1, r2", "//   y--;  // 后缀")
    a.st(1, 4, "y", "")
    a.ld(1, 4, "y", "")
    a.li(2, 1, "")
    a.raw16(enc_sub(1, 2), "SUB r1, r2", "//   --y;  // 前缀")
    a.st(1, 4, "y", "")

    # ---- 逻辑与/或（短路） ----
    a.ld(1, 0, "x", "")
    a.li(2, 5, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "//   if (x>5 && y>10) g_result = 1;")
    a.bcc("GT", "and1", "")
    a.li(1, 0, "")
    a.stg(1, 0x1000+0x04, "g_result", "//     短路: x<=5")
    a.bcc("NZ", "and_done", "")
    a.label("and1")
    a.ld(1, 4, "y", "")
    a.li(2, 10, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GT", "and2", "")
    a.li(1, 0, "")
    a.stg(1, 0x1000+0x04, "g_result", "//     短路: y<=10")
    a.bcc("NZ", "and_done", "")
    a.label("and2")
    a.li(1, 1, "")
    a.stg(1, 0x1000+0x04, "g_result", "//     均为真: g_result = 1")
    a.label("and_done")
    a.ld(1, 0, "x", "")
    a.li(2, 3, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "//   if (x>3 || y<0) n = 1; else n = 0;")
    a.bcc("GT", "or1", "")
    a.ld(1, 4, "y", "")
    a.li(2, 0, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("LT", "or1", "")
    a.li(1, 0, "")
    a.st(1, 20, "n", "//     均假: n = 0")
    a.bcc("NZ", "or_done", "")
    a.label("or1")
    a.li(1, 1, "")
    a.st(1, 20, "n", "//     任一真: n = 1")
    a.label("or_done")

    # ---- 类型转换 (short) ----
    a.ld(1, 0, "x", "")
    a.raw16(enc_sxh(1), "SXH r1", "//   (short)x;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.raw16(enc_zxh(1), "ZXH r1", "//   (unsigned short)t;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.raw16(enc_sxb(1), "SXB r1", "//   (char)t;")
    a.st(1, 12, "t", "")
    a.ld(1, 12, "t", "")
    a.raw16(enc_zxb(1), "ZXB r1", "//   (unsigned char)t;")
    a.st(1, 12, "t", "")

    # ---- if (t != 0) t = 1; else t = 0; ----
    a.ld(1, 12, "t", "")
    a.raw16(enc_cmp(1, 0), "CMP r1, r0", "//   if (t != 0) t = 1; else t = 0;")
    a.bcc("NZ", "nz1", "")
    a.li(1, 0, "")
    a.st(1, 12, "t", "")
    a.bcc("NZ", "nz_done", "")
    a.label("nz1")
    a.li(1, 1, "")
    a.st(1, 12, "t", "")
    a.label("nz_done")

    # ---- if (t <= 0) t += 8; ----
    a.ld(1, 12, "t", "")
    a.li(2, 0, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "//   if (t <= 0) t += 8;")
    a.bcc("LE", "le1", "")
    a.ld(1, 12, "t", "")
    a.li(2, 8, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 12, "t", "")
    a.label("le1")

    # ---- 结构体成员访问 ----
    a.ldg(1, 0x1000+0x24, "pt.x", "//   int sx = pt.x;")
    a.ldg(2, 0x1000+0x28, "pt.y", "//   int sy = pt.y;")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "//   m = pt.x + pt.y;")
    a.st(1, 48, "m", "")

    # ---- 函数调用: 数组/指针/三元/多参 ----
    a.ldg(1, 0x1000+0x14, "arr[0]", "")
    a.ldg(2, 0x1000+0x1C, "arr[2]", "//   g_result = max2(arr[0], arr[2]);")
    a.jarl("max2", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.stg(1, 0x1000+0x04, "g_result", "")
    a.li(1, 0x1000+0x10, "//   p = load16(&p_store);  // 指针解引用")
    a.jarl("load16", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.st(1, 80, "p", "")
    a.li(1, 1, "//   g_total = sum3(1, 2, 3);  // 三参数调用")
    a.li(2, 2, "")
    a.li(14, 3, "")
    a.jarl("sum3", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.stg(1, 0x1000+0x08, "g_total", "")
    a.ldg(1, 0x1000+0x14, "arr[0]", "")
    a.li(2, 16, "")
    a.jarl("arr_sum", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.stg(1, 0x1000+0x08, "g_total", "//   g_total = arr_sum(arr, 16);")
    a.ldg(1, 0x1000+0x14, "arr[0]", "")
    a.li(2, 16, "")
    a.jarl("find_max", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.stg(1, 0x1000+0x04, "g_result", "//   g_result = find_max(arr, 16);")
    a.li(1, 3, "")
    a.ld(2, 16, "u", "")
    a.jarl("add2", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.st(1, 16, "u", "//   u = add2(3, u);")
    a.ldg(1, 0x1000+0x14, "arr[0]", "")
    a.li(2, 3, "")
    a.jarl("mod2", "")
    a.raw16(enc_zxh(1), "ZXH r1", "")
    a.st(1, 68, "sw", "//   sw = arr[0] % 3;")

    # ---- switch ----
    a.ld(1, 68, "sw", "")
    a.li(2, 1, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "//   switch (sw) { case 1: case 2: case 3: default: }")
    a.bcc("Z", "sw_case1", "")
    a.li(2, 2, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("Z", "sw_case2", "")
    a.li(2, 3, "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("Z", "sw_case3", "")
    a.li(1, 0, "")
    a.st(1, 72, "comp", "//     default: comp = 0;")
    a.bcc("NZ", "sw_done", "")
    a.label("sw_case1")
    a.li(1, 100, "")
    a.st(1, 72, "comp", "//     case 1: comp = 100;")
    a.bcc("NZ", "sw_done", "")
    a.label("sw_case2")
    a.li(1, 200, "")
    a.st(1, 72, "comp", "//     case 2: comp = 200;")
    a.bcc("NZ", "sw_done", "")
    a.label("sw_case3")
    a.li(1, 300, "")
    a.st(1, 72, "comp", "//     case 3: comp = 300;")
    a.label("sw_done")

    # ---- 函数调用 store16（指针写） ----
    a.li(1, 0x1000+0x10, "//   p = &p_store; *p = 12;")
    a.li(2, 12, "")
    a.jarl("store16", "")
    a.ldg(1, 0x1000+0x10, "p_store", "")
    a.st(1, 80, "p", "//   p = p_store;  // 读回验证")

    # ---- 收尾 ----
    a.raw32(enc_popsp(15, 15), "POPSP r15, r15", "//   }  // 弹 r15")
    a.raw32(enc_popsp(14, 14), "POPSP r14, r14", "//   弹 r14")
    a.raw32(ENC_HALT, "HALT", "//   HALT — 程序结束")

    # ================= 函数段（main 之后，JARL 全部前向） =================

    # ---- int square(int x) { return x*x; } ----
    a.label("square", True)
    a.raw32(enc_mul(1, 1), "MUL r1, r1", "// int square(int x) { return x*x; }")
    a.ret("// }")

    # ---- int add2(int a, int b) { return a+b; } ----
    a.label("add2", True)
    a.raw16(enc_add(1, 2), "ADD r1, r2", "// int add2(int a, int b) { return a+b; }")
    a.ret("// }")

    # ---- int div2(int a, int b) { return a/b; } ----
    a.label("div2", True)
    a.raw16(enc_zxh(1), "ZXH r1", "// int div2(int a, int b) { return a/b; }")
    a.raw16(enc_zxh(2), "ZXH r2", "//   (short)a / (short)b")
    a.raw32(enc_pushsp(14, 14), "PUSHSP r14, r14", "")
    a.raw16(enc_divh(1, 2), "DIVH r1, r2", "")
    a.raw32(enc_popsp(14, 14), "POPSP r14, r14", "")
    a.ret("// }")

    # ---- int mod2(int a, int b) { return a % b; } ----
    a.label("mod2", True)
    a.raw16(enc_zxh(1), "ZXH r1", "// int mod2(int a, int b) { return a % b; }")
    a.raw16(enc_zxh(2), "ZXH r2", "")
    a.raw32(enc_pushsp(14, 14), "PUSHSP r14, r14", "")
    a.raw16(enc_divh(1, 2), "DIVH r1, r2", "")
    a.raw32(enc_popsp(14, 14), "POPSP r14, r14", "")
    a.ret("// }")

    # ---- int sum3(int a, int b, int c) { return a+b+c; } ----
    a.label("sum3", True)
    a.raw32(enc_pushsp(14, 14), "PUSHSP r14, r14", "// int sum3(int a, int b, int c) {")
    a.raw16(enc_mov(15, 1), "MOV r15, r1", "//   int t = a + b;")
    a.raw16(enc_add(15, 2), "ADD r15, r2", "")
    a.raw16(enc_add(15, 14), "ADD r15, r14", "//   return t + c;")
    a.raw16(enc_mov(1, 15), "MOV r1, r15", "")
    a.raw32(enc_popsp(14, 14), "POPSP r14, r14", "")
    a.ret("// }")

    # ---- int max2(int a, int b) { return a>b?a:b; } ----
    a.label("max2", True)
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "// int max2(int a, int b) {")
    a.bcc("GT", "max2_gt", "//   return a>b?a:b;  // 三元 ?:")
    a.raw16(enc_mov(1, 2), "MOV r1, r2", "")
    a.raw16(enc_jmp31(), "JMP [r31]", "")
    a.label("max2_gt")
    a.ret("// }")

    # ---- void store16(int *p, int v) { *p = v; } ----
    a.label("store16", True)
    a.str(2, 1, 0, "ST.W r2, [r1]", "// void store16(int *p, int v) { *p = v; }")
    a.ret("// }")

    # ---- int load16(int *p) { return *p; } ----
    a.label("load16", True)
    a.ldr(1, 1, 0, "LD.W [r1], r1", "// int load16(int *p) { return *p; }")
    a.ret("// }")

    # ---- int arr_sum(int *arr, int n) {
    #   int i=0, sum=0; for (; i<n; i+=4) sum += arr[i]; return sum; }
    # ----
    a.label("arr_sum", True)
    a.raw32(enc_pushsp(14, 14), "PUSHSP r14, r14", "// int arr_sum(int *arr, int n) {")
    a.li(1, 0, "//   int i=0, sum=0;")
    a.st(1, 0, "i", "")
    a.li(1, 0, "")
    a.st(1, 4, "sum", "")
    a.label("as_loop")
    a.ld(1, 0, "i", "")
    a.ld(2, 0, "n", "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GE", "as_end", "//   for (; i<n; i+=4) sum += arr[i];")
    a.ldr(2, 1, 0, "LD.W [r1], r2", "")
    a.ld(1, 4, "sum", "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 4, "sum", "")
    a.ld(1, 0, "i", "")
    a.li(2, 4, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 0, "i", "")
    a.raw16(enc_mov(15, 1), "MOV r15, r1", "//   ptr += 1")
    a.bcc("NZ", "as_loop", "")
    a.label("as_end")
    a.ld(1, 4, "sum", "")
    a.raw32(enc_popsp(14, 14), "POPSP r14, r14", "")
    a.ret("// }")

    # ---- int find_max(int *arr, int n) {
    #   int m = arr[0]; for (i=4; i<n; i+=4) if (arr[i]>m) m=arr[i]; return m; }
    # ----
    a.label("find_max", True)
    a.raw32(enc_pushsp(14, 14), "PUSHSP r14, r14", "// int find_max(int *arr, int n) {")
    a.ldr(2, 1, 0, "LD.W [r1], r2", "//   int m = arr[0];")
    a.st(2, 4, "m", "")
    a.li(1, 4, "")
    a.st(1, 0, "i", "")
    a.label("fm_loop")
    a.ld(1, 0, "i", "")
    a.ld(2, 0, "n", "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GE", "fm_end", "//   for (i=4; i<n; i+=4)")
    a.ldr(2, 1, 0, "LD.W [r1], r2", "")
    a.ld(1, 4, "m", "")
    a.raw16(enc_cmp(1, 2), "CMP r1, r2", "")
    a.bcc("GT", "fm_set", "//     if (arr[i]>m) m = arr[i];")
    a.ld(1, 0, "i", "")
    a.li(2, 4, "")
    a.raw16(enc_add(1, 2), "ADD r1, r2", "")
    a.st(1, 0, "i", "")
    a.raw16(enc_mov(15, 1), "MOV r15, r1", "")
    a.bcc("NZ", "fm_loop", "")
    a.label("fm_set")
    a.ldr(2, 1, 0, "LD.W [r1], r2", "")
    a.st(2, 4, "m", "")
    a.bcc("NZ", "fm_loop", "")
    a.label("fm_end")
    a.ld(1, 4, "m", "")
    a.raw32(enc_popsp(14, 14), "POPSP r14, r14", "")
    a.ret("// }")

    # ---- 数据符号表（0x1000+） ----
    a.symbols = [
        ("g_counter", 0x1000, 4),
        ("g_result",  0x1004, 4),
        ("g_total",   0x1008, 4),
        ("g_grade",   0x100C, 4),
        ("p_store",   0x1010, 4),
        ("arr0", 0x1014, 4),
        ("arr1", 0x1018, 4),
        ("arr2", 0x101C, 4),
        ("arr3", 0x1020, 4),
        ("pt_x",  0x1024, 4),
        ("pt_y",  0x1028, 4),
    ]
    # 数据标签（供 ldg/stg 使用）
    for name, addr, _ in a.symbols:
        a.labels[name] = addr

    return a

def main():
    a = build()
    # 回填所有前向 B.cond 占位
    for pc, c, name in a._pending_bcc:
        assert name in a.labels, f"B.cond 未定义标签: {name}"
        disp = a.labels[name] - pc
        new_w = enc_bcc(c, disp)
        a.code[pc:pc+2] = new_w
        for i, e in enumerate(a.entries):
            if e[0] == pc:
                a.entries[i] = (pc, new_w, e[2], e[3])
                break

    # 回填所有前向 JARL 占位（必须前向）
    for pc, name in a._pending_jarl:
        assert name in a.labels, f"JARL 未定义标签: {name}"
        target = a.labels[name]
        assert target > pc, f"JARL 必须前向: {name}@{target:#x} <= PC {pc:#x}"
        new_w = enc_jarl31(pc, target)
        a.code[pc:pc+4] = new_w
        for i, e in enumerate(a.entries):
            if e[0] == pc:
                a.entries[i] = (pc, new_w, e[2], e[3])
                break

    code = bytes(a.code)
    if len(code) >= 0x1000:
        raise RuntimeError(f"代码段超 0x1000: {len(code):#x}")
    if len(code) & 1:
        code += b'\x00'

    with open(os.path.join(D, "test.rom"), "wb") as f:
        f.write(code)

    lines = []
    for addr, b, asm, src in a.entries:
        hx = " ".join(f"{x:02X}" for x in b)
        lines.append(f"{addr:08X}  {hx}  {asm}  // {src.strip()}")
    with open(os.path.join(D, "test.lst"), "w") as f:
        f.write("\n".join(lines) + "\n")

    out = ["Symbol Table:"]
    for name, addr, size in a.symbols:
        out.append(f"{name} {addr:08X} {size}")
    with open(os.path.join(D, "test.map"), "w") as f:
        f.write("\n".join(out) + "\n")

    print(f"ROM: {len(code)} bytes, {len(a.entries)} entries")
    print(f"main={a.labels['main']:#x}  square={a.labels['square']:#x}  find_max={a.labels['find_max']:#x}")
    print("symbols:", ", ".join(f"{n}@{ad:#x}({s})" for n, ad, s in a.symbols))

if __name__ == "__main__":
    main()
