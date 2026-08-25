# E1MS2 程序执行仿真器 — 工作交接记录（HANDOFF）

> 本文件是跨会话的工作记录：目标、进度、关键结论、决策、**路线图**都在里面。
> 每次会话开始时先读本文件，结束时更新它。
>
> ⚠️ **迭代优化前必须先读 `docs/DESIGN.md`（设计文档）**：架构、界面设计、
> 功能路线图（§5）、硬件 debug 工具功能对标（§6）、设计原则（§8）、
> 构建验证命令（§11）都在那里。本文件只记录"做了什么/进度"，
> "应该是什么样"以设计文档为准。每完成一个任务：更新本文件 + 同步设计文档。
>
> 📌 **当前状态**：
> - 前端：`web/`（Vue 3 + TS + Vite 5）为唯一 UI，亮色科技风主题
> - 引擎：G3MH 主核 + FPU，48 原生测试通过；HALT/Watchpoint/JR/JARL/PUSHSP 全修
> - 测试 ROM：`testdata/test.rom` 覆盖全部 C 语法，浏览器 E2E 跑通到 HALT（312 条指令）
> - 可移植：Python 3 静态服务 + 预编译 WASM，无需 Node.js 即可启动
> - **下一步**：G3K 副核 → 内存多格式视图 → 面板拖拽调整大小
> - ⚠️ 遗留：测试 ROM 部分 RAM 值与预期不符（g_total=38 ≠ 55），数据路径待排查

## 1. 项目目标

做一个**纯前端网页工具**，用于仿真调试 RH850/E1M-S2 上的程序执行。
用户输入：
- 全量 ROM（bin 格式，即 code flash 用户区镜像）
- 编译产物：带汇编+源代码的文件（交叉汇编 listing）+ map 文件

期望功能：
1. 加载后进入仿真调试界面：
   - 左：ROM 数据查看（部分数据，跟随 PC 跳转显示，显示地址）
   - 中上：控制区（重置、单步、跨函数执行、上一步/下一步）
   - 中中：带汇编和源代码的文件内容（只读，可打断点）
   - 中下：RAM 监视区（从 map 选取变量或手动添加监视）
   - 右上：寄存器显示区
   - 右下：文件浏览器（切换显示哪个文件）
2. 良好布局和动效

## 2. 关键硬件/架构事实（已核实，来自手册文本）

### 核心配置（硬件手册 R01UH0603 Rev1.10，§3.1/§4）
- CPU1 = **RH850G3MH**（主核，32 位，带 FPU 单/双精度，PUSHSP/POPSP，MPU 16 区）
- PCU (PE3) = **RH850G3K**（外设控制核）
- 地址空间（Table 4.1）：
  - `0000 0000H–003F FFFFH` code flash 用户区（4MB，复位向量地址）
  - `0100 0000H–0100 7FFFH` code flash 用户 boot 区（32KB）
  - `1000 0000H–1FFF FFFFH` on-chip I/O（CPU1 可访问；PCU 不能，PCU 用 FC00_0000H–FE5F_FFFFH 窗口）
  - `FEBF 0000H–FEBF FFFFH` Local RAM (CPU1) 64KB
  - `FEDF 0000H–FEDF FFFFH` Local RAM (self) 64KB（CPU1 视角=自己）
  - `FEEE 0000H–FEF1 FFFFH` Global RAM 256KB
  - `FF00 0000H–FFFD FFFFH` on-chip I/O（含 Data flash FF20_0000H 64KB）
- 复位向量：user 区头部 `0000 0000H` 或 user boot 区 `0100 0000H`

### G3 指令集（软件手册 G3MH R01US0143 Rev1.30，G3K R01US0165 Rev1.20）
- 变长指令：16/32/64 位；PC0=0（半字对齐）
- 32 个 32 位通用寄存器 r0–r31；r0 恒 0；r3=SP、r4=GP、r5=TP、r30=EP、r31=LP（编译器约定）
- 条件码表 Table 7.4（V/NV、C/L、Z/NZ、NH/H、S/N、T、SA、LT/GE、LE/GT）
- PSW：UM、CU2-0、EBV、debug 位、SAT、CY(O)、N(Z)、S、V；复位值 0020H
- 系统寄存器经 LDSR/STSR，(regID, selID) 寻址：
  - SR5,0=PSW；SR2,1=RBASE；SR3,1=EBASE；SR4,1=INTBP；SR0,2=HTCFG0；SR6,1=PID
  - SR13,0/EIIC、SR14,0/FEIC、SR28,0/EIWR、SR29,0/FEWR、SR5,0=PSW
- FPU 协处理器：FPSR/FPEPC/FPST/FPCC/FPCFG（SR6–10,0）
- 异常向量（Table 4.3，直接向量法）：base = (PSW.EBV==1)? EBASE : RBASE
  - RESET=+000H, SYSERR=+010H, FETRAP=+030H, TRAP0=+040H, TRAP1=+050H,
    RIE=+060H, FPINT=+070H, UCPOP=+080H, MIP/MDP=+090H, PIE=+0A0H,
    Debug=+0B0H, MAE=+0C0H, FENMI=+0E0H, FEINT=+0F0H,
    EIINTn(pri 0–15)=+100H..+1F0H（按优先级；INTBP 表查法：INTBP + 通道×4 读 32 位地址）
- 异常优先级（Table 4.1）：RESET=1, FENMI=3, SYSERR=2, FEINT=3, EIINT=4,
  MIP=10, SYSERR(FE)=10, RIE=10, UCPOP=10, PIE=10, MAE=11, FPINT=11, SYSCALL=12, FETRAP=12, TRAP=12
- 特权：UM=1 用户态，0 特权态；PIE（特权指令异常）
- 指令分四类：basic（§7.2，~151 页起）、cache（§7.3）、FPU（§7.4）、coprocessor（应用定义）
- 基本指令分组（§7.2.1）：Load(LD/SLD)、Store(ST/SST)、MUL/MAC、ADD/SUB/CMP/MOV/ADDI/MOVHI/SUBR/SUBB、
  条件算术 ADF/SBF、饱和 SATADD/SATSUB/SATSUBI/SATSUBR、逻辑 AND/OR/XOR/NOT/ANDI/ORI/XORI/TST、
  数据操作 BINS/BSH/BSW/CMOV/HSH/HSW/ROTL/SAR/SASF/SETF/SHL/SHR/SXB/SXH/ZXB/ZXH、
  位搜索 SCH0L/0R/1L/1R、DIV/DIVH/DIVHU/DIVU/DIVQ/DIVQU、分支 JARL/JMP/JR/Bcond、
  系统 PUSHSP/POPSP/EIRET/FERET/SYSCALL/INTSYSCALL/TRAP/INVD、
  位操作 BIT0/BIT1/BITR/BITC、PUSH/POP、MOVR2R/MOVR2M 等

## 3. 决策记录（Decision Log）

| # | 日期 | 决策 | 理由 |
|---|------|------|------|
| 1 | 2026-07-24 | 工具为**纯浏览器前端**，无后端 | 用户上传本地数据，隐私+简单 |
| 2 | 2026-07-24 | 用 **Rust→WASM**（wasm-bindgen）写指令解码/执行引擎 | 性能（大程序循环执行快）、类型安全、可移植 |
| 3 | 2026-07-24 | 用 **Vue 3 + Vite + TypeScript** 做 UI | 布局组件化、组合式 API 简洁、Vite 构建快 |
| 4 | 2026-07-24 | 单核仿真先做 **G3MH（主核 CPU1）**，G3K 留扩展 | E1MS2 主核是 G3MH，先覆盖主程序 |
| 5 | 2026-07-24 | 指令集**分阶段实现**：Phase 1 覆盖常见指令 + 基础异常（RIE 兜底）；Phase 2 加 FPU + PUSHSP/POPSP + 中断 | 完整 G3 集 200+ 条，全做耗时，先能跑 90% 代码 |
| 6 | 2026-07-24 | map 文件解析 + listing 解析，提取变量地址/名称用于 RAM 监视 | 用户要从 map 选变量 |
| 7 | 2026-08-25 | 预编译 WASM 提交进仓库 + Python 3 静态服务 | 用户无需 Rust/Node.js 即可启动，一条命令跑起来 |
| 8 | 2026-08-25 | 手册 PDF 不入库，只提交 `pdftotext -layout` 文本提取 | PDF 共 37MB 太大；文本 ~14MB 可查指令编码，PDF 可从 Renesas 官网免费下载 |

## 4. 进度（当前状态）

- [x] 引擎 `sim/`（Rust，cdylib）：state.rs（G3 状态+PSW+条件码+向量）、mem.rs
  （真实 E1MS2 地址映射：flash 只读 + Local RAM + Global RAM + I/O 稀疏）、
  fpu.rs（f64 后端，RNE）、exec.rs（16/32 位全部分发）、exception.rs、
  lib.rs（wasm-bindgen 导出面）。
  `cargo test` **48 pass, 0 fail, 0 ignore**。
- [x] 前端 `web/`（Vue 3 + TS + Vite 5）：10 组件（App.vue + ControlBar/CodeView/
  RegisterPanel/MemoryPanel/WatchPanel/TracePanel/CallStackPanel/StatusBar/FaultBanner
  + composable useSimulator）。WASM 产物在 `web/src/sim/`（build-wasm.sh 直接输出）。
- [x] 可移植启动：`start.sh`（Linux/macOS）+ `start.ps1`（Windows）+ `serve.py`
  （Python 3 零依赖静态服务器）。预编译 WASM 已提交，无需 Rust 即可运行。
- [x] 浏览器 E2E：chrome-headless-shell + CDP 模式（`testdata/cdp_test.js`），
  加载 test.rom → step 到 HALT 验证。
- [x] 测试 ROM：`testdata/mkrom.py` 单遍汇编器 → test.rom(1222B) + test.lst + test.map，
  覆盖 C 全语法（見下方 C 语法清单）。

### 已知缺陷（遗留）

- FPU 舍入 RZ/RP/RM 简化为 RNE
- FPU Format F:I 位域布局需对照手册核实（marker 与 op/type/sub 重叠）
- CALLT 编码需对照手册核实（op6 与 OR 冲突，guard 矛盾）
- SLD/SST 16 位 base 寄存器用 r0（可能是 r30/EP）
- 缺指令：BIT0/1/R/C、SATADD/SATSUB、ADF/SBF、MOVR2R、LD.D/ST.D
- runTo 同步大循环（chunk 内不可打断）；Run to Cursor 用临时断点
- **G3K（PCU 副核）未实现**
- listing/map 解析器是宽松启发式（mkrom.py 格式已验证，真实 CS+ 样例待验证）
- **测试 ROM 部分 RAM 值与预期不符**（g_total=38≠55 等，数据路径待排查）

### 测试 ROM 覆盖的 C 语法

变量声明/初始化、赋值、算术 + - * / %、比较 < <= > >= !=、逻辑与/或 && ||（短路）、
位运算 & | ^ ~ << >>、++/--（前缀/后缀）、三元 ?:、if/else、if 链、while、
do-while、for、函数调用（2/3 参数、多函数、指针传参）、数组+指针+指针算术、
结构体成员访问、switch/case/default、复合赋值 += -=、类型转换 (short)、
全局变量、SP 帧（PUSHSP/POPSP）、HALT。

寄存器约定：r0=0（硬件）、r31=SP、参数 r1/r2(/r14)、返回 r1（DIVH/MULH 后 ZXH）、
r15/r16 临时、局部变量 [r31+0/4/8/12+]、全局变量 0x1000+（ST.W/LD.W 绝对地址）。

## 5. 已核实的指令编码（防重复查手册，位域从 MSB 到 LSB）

- MOV reg1,reg2 = `rrrrr000000RRRRR`（op6=0）
- ADD = `rrrrr001110RRRRR`；SUB = `rrrrr001101RRRRR`；CMP = `rrrrr001111RRRRR`
- AND = `rrrrr001010RRRRR`；OR = `rrrrr001000RRRRR`；XOR = `rrrrr001001RRRRR`；NOT = `rrrrr000001RRRRR`
- JMP [reg1] = `00000000011RRRRR`（op6=0b000110）
- MULH = `rrrrr000111RRRRR`；DIVH = `rrrrr000010RRRRR`
- ADDI imm16,reg1,reg2 = `rrrrr110000RRRRR` + imm16（Format VI，op6=0b110000，16/32 位共用 op6 靠长度区分）
- MOVHI = `rrrrr110010RRRRR` + imm16（op6=0b110010）
- Bcond 16 位：Format III 布局 `disp8(15..8) | opcode4(7..4) | cccc(3..0)`，opcode4=0b1011，**目标=PC+sign_extend(disp8)*2**（不是 disp9！手册 Table 7.1 确认 4-bit opcode 字段）。测试向量 0x04B2(BZ,disp8=4)、0xFCB2(BZ,disp8=-4) 已验证
- JR/JARL disp22：Format V = `reg2(5,15..11) | opcode5(5,10..6) | disp22(22, bits5..0 + word1)`，JR opcode5=0b011110(reg2=0)，JARL opcode5=0b111100(reg2≠0)，目标=PC+sign_extend(disp22)*2
- **扩展指令 word1 低位 11 位模式（用 hex 匹配 h1&0x7FF）**：
  - MUL = 0x220（`wwwww0100010 0000`）；MULU = 0x222；MAC = 0x1E0（reg3 在 word1 bits7..4！）；MACU = 0x1F0
  - DIVU = 0x2A2（reg2/=reg1，商→reg2 余→wwwww）；SHL = 0x062；SHR = 0x042；SAR = 0x052
  - JARL [reg1],reg3 = 0x150（reg3=word1 wwwww，PC=[reg1]，reg3←PC+4）
  - SETF cccc = word1 全 0 + word0 `rrrrrr111110cccc`（op6=0b111110=62）
  - **坑：Rust 二进制字面量下划线会误位（0b0001100_000010 实际=0x060≠0x062）→ 全部改 hex**
- ZXB=`00000000100RRRRR`(op6=4) SXB=`...101RRRRR`(op6=5) ZXH=`...110RRRRR`(op6=6) SXH=`...111RRRRR`(op6=7) TST=`rrrrr001011RRRRR`(op6=11)——**寄存器在低 5 位（手册排版误导，debug 实测确认）**
- **MULH 16 位 op6 = `0b011111`（31）**，不是 `0b000111`（=7！）。之前 MULH arm 写成 `0b000111` 实际值=7，遮蔽了 SXH 的 `7u32` arm → SXH 永远不执行。**坑：Rust `0b000111` 是 7 不是 39，写 op6 时用 `0b011111` 或 hex `0x1F` 防混淆**
- SLD/SST 16 位 Format IV = `rrrrr0110ddddddd` 等，op6 落在 20..27（当前用此范围检测）
- LD.W = `rrrrr111000RRRRR`+disp16（op6=0b111000）；ST.W = op6=0b111011；LD.H = 0b111001；ST.H = 0b111010
- EIRET = word0 `0000011111100000` + word1 `0000000101001000`(0x0148)
- TRAP = word0 `00000111111vvvvv` + word1 `0000000100000000`(0x0100)，vector5
- SYSCALL = word0 `11010111111vvvvv` + word1 `00VVV00101100000`
- PUSHSP = `01000111111RRRRR` + `wwwww00101100000`（rh,rt 寄存器对列表）
- FPU Format F:I：word0 `rrrrr111111RRRRR`，word1 `wwwww100011000000` + `op(6)type(2)sub(1)`（bit15..11=reg3）。已核实：
  - ADDF.S op=000000 type=01；SUBF.S op=000100；MULF.S op=001000；DIVF.S op=011110
  - ADDF.D op=000000 type=00；CVTF.WS op=000010 type=01（reg1=0）
  - 操作：reg3 ← reg1 OP reg2（dest 是 word1 的 wwwww！）
- **PSW NZ 位语义**：bit9=1 表示"非零"（NZ），Z=1 当且仅当 bit9=0。所有 `set_psw(PSW_NZ, ...)` 必须用 `result != 0`；`cond()` 里 `z = (psw & PSW_NZ) == 0`。之前全反了，BE/BZ/BT/TST 结果判断全错

## 6. 下一步行动（Next Steps / 路线图）

1. **G3K（PCU）副核**：第二核指令集 + FC 地址窗口映射（用户已要求验证副核）
2. **内存多格式视图**：web MemoryPanel 扩展 hex/u8/u16/s16/u32/f32/f64/C[] 格式
3. **面板拖拽调整大小**：列宽/行高 resizer
4. **测试 ROM 数据路径排查**：g_total/g_grade/p_store 值与预期不符，数据路径待查
5. 指令补全：BIT0/1/R/C、SATADD/SATSUB、ADF/SBF、MOVR2R、LD.D/ST.D
6. 真实 CS+/GCC 编译产物端到端测试（待用户提供样例）

> 更完整的分阶段路线图（Phase 1 已交付 / Phase 2 进行中 / Phase 3 后续）
> 以及手册对标优化清单见 `docs/DESIGN.md` §5 / §10。

## 7. 风险与开放问题

- G3 基本指令集 ~200 条，目前覆盖常用子集；用户程序用到未实现指令会 RIE 兜底
- FPU Format F:I 位域、CALLT 编码待手册核实（见 §4 已知缺陷）
- 真实编译器 listing/map 格式未验证（当前解析器按 mkrom.py 输出格式 + 宽松启发式）

## 8. 环境备注

- 可移植：`./start.sh`（Linux/macOS）或 `.\start.ps1`（Windows），Python 3 即可
- 开发模式：`./start.sh --dev`（需 Node.js + npm，Vite 热重载）
- 3 份手册 PDF 原件不入库（.gitignore），文本提取在 `docs/manual/`
- chrome-headless-shell 在 `.playwright-browsers/`（不入库，仅开发调试用）
- **改了 exec.rs 后必须**：`bash sim/build-wasm.sh`（内含 cargo clean，防缓存命中）
- **改了 mkrom.py 后必须**：`python3 testdata/mkrom.py` 重新生成 rom/lst/map

## 更新日志

- 2026-08-25：**GitHub 发布准备 + 可移植化 + 仓库清理**：
  - 重构目录：前端统一到 `web/`，手册文本移至 `docs/manual/`（PDF 不入库）
  - 消除硬编码路径（smoke.mjs、cdp_test.js、build-wasm.sh 改用相对路径）
  - 新增 `start.sh` / `start.ps1` / `serve.py`（Python 3 零依赖静态服务 + Vite 开发模式双轨）
  - 预编译 WASM 产物提交进仓库（无需 Rust 即可启动）
  - 新增 `README.md`（项目入口）+ `docs/manual/README.md`（手册说明）
  - 清理迭代痕迹：移除旧 UI 布局存档/历史注释、修正文档与代码不符的描述、
    章节编号重排、更新决策记录、同步 DESIGN.md
  - 本地 git 仓库建立，待远程地址
