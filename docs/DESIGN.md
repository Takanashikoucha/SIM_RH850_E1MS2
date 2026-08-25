# E1MS2 程序执行仿真器 — 设计文档（DESIGN）

> 本文件是**架构与功能设计的单一事实来源**（single source of truth）。
> HANDOFF.md 记录"做了什么/进度"；本文件记录"系统应该是什么样、为什么、怎么演化"。
> **每次做迭代优化前，先读本文件，对照 §5 路线图和 §8 设计原则选择任务；
> 每次完成一个任务，更新 HANDOFF.md 并同步更新本文件（改动的部分）。**

---

## 1. 目标与定位

纯浏览器端的 RH850/E1M-S2（G3MH 主核）程序执行仿真调试器。用户输入三件东西，
不需要任何后端或网络连接：

| 输入 | 说明 | 必需性 |
|---|---|---|
| `.bin` / `.rom` | code flash 用户区镜像（全量 ROM） | **必需**，加载后进入调试界面 |
| `.lst` listing | 交叉汇编 + 源代码混合 listing | 可选，提供源码级视图 |
| `.map` | linker map（符号名 → 地址） | 可选，提供命名符号（监视/栈帧/调用图） |

设计目标（按优先级）：

1. **只用 bin 就能工作**：没有 listing/map 时，反汇编视图 + 内存/寄存器/断点/单步
 全功能可用。这是底线。
2. **像真正的硬件调试器**：功能模型对齐 Renesas E1 Emulator / J-Link /
 IAR J-Link GDB Server 这类工具（见 §6 功能参考矩阵）。
3. **界面直观**：视线动线 = 代码（中）→ 寄存器（右上）→ 数据（中下）。
 任何时刻都能一眼回答三个问题：**PC 在哪？改了哪些状态？下一条指令是什么？**
4. **可观测性**：每一步的可逆（step back/forward）、变化高亮、统计，
 让"黑盒行为"变成可检查的白盒。

## 2. 技术架构

```
┌────────────────────────────────────────────────────────────┐
│ browser (纯前端，无后端) │
│ │
│ Vue 3 + TS + Vite 5 (web/) ← 唯一 UI │
│ └── 10 组件 (App.vue + composables/useSimulator) │
│ │
│ ▼ │
│ web/src/sim/sim.js + sim_bg.wasm ← Rust 引擎 (sim/src/*) │
│ (wasm-bindgen 产物，sim/build-wasm.sh 直接输出到此) │
└────────────────────────────────────────────────────────────┘
```

### 2.1 数据流向

- **控制流**：面板按钮/快捷键 → App.vue 的 action（doStep/onRun/...）→ WASM `stepOnce/runTo`
 → 状态回读（getPc/getReg/...）→ Vue refs → 面板重渲染。
- **历史（可逆执行）**：每次 step 前拍一个 `Snapshot{pc,psw,r[32],fp[16],insCount}`，
 并把该步的 RAM 写日志 `nextLog=[addr,oldByte,...]` 附加在快照上。
 - step back = 恢复快照 + `restoreWriteLog(nextLog)`（回滚 RAM 写）
 - step forward = 恢复快照 + 重新执行该步
 - 限制：FP 寄存器快照当前不恢复（wasm 无 setFpReg，见 §9 已知缺陷）。
- **文件解析**：parsers.ts（宽松正则，格式未定，待真实样例后收紧）。

### 2.2 引擎（Rust `sim/`）

| 模块 | 职责 |
|---|---|
| state.rs | G3 状态：r0–r31、PC、PSW（位定义）、系统寄存器（RBASE/EBASE/INTBP）、异常保存（EIPC/...）、FPU 寄存器、断点集合、条件码表、异常向量偏移 |
| mem.rs | Memory：真实 E1MS2 地址映射。code flash 0x0–0x3FFFFF（rom，只读）+ code 缓冲区（ram，可写，测试用）+ Local RAM 0xFEBF0000 64KB + Global RAM 0xFEEE0000 256KB + I/O 0x10000000–0x1FFFFFFF/0xFF000000–0xFFFDFFFF（稀疏 HashMap）；区域判断方法（in_flash/in_local_ram/in_global_ram/in_io/in_mapped）；写日志 |
| fpu.rs | FPU：f64 后端，单/双精度，RNE（其余舍入模式简化） |
| exec.rs | 解码执行核心（step），按 16/32 位 op6 分发 |
| exception.rs | 异常派发 |
| lib.rs | wasm-bindgen 导出面（Sim struct 的公有方法 = 唯一 JS API） |

**API 最小化原则**：JS 只通过 lib.rs 导出的方法访问引擎。加新功能时优先扩展这个面，
不在 JS 侧复制引擎逻辑（disasm.ts 例外：它是 JS 侧独立解码器，用于 UI 渲染，
编码必须与 exec.rs 保持一致，改动编码要两边同步）。

## 3. 界面设计

> 布局与视觉约定分三部分：§3.1 视觉语言（design tokens）、§3.2 交互约定、
> §3.3 当前布局（★ 现状图）。

**自适应规则**：

- 根布局高度用 `100%` 链（`html/body/#app` 全 `height: 100%`，页面级 `overflow: hidden`）。
- 三列宽度：侧列 `minmax(260px, 280px)` / `minmax(280px, 300px)`；中列 `1fr` 吸收剩余。
（面板拖拽调整大小是路线图项，实现后侧列改为可拖动。）
- 主 grid 行 `auto 1fr auto`（控制条 / 内容 / 状态栏）；
 **新面板加入 grid 时必须用 `minmax(0, …)` 或 `auto` 行，禁止裸 `1fr`/固定大 px**，
 否则小窗口下会被裁掉。
- 内容超出面板高度时在面板内部滚动（`.panel-body { overflow: auto }`），页面级永不滚动。
- **空态收起**：临时性面板（Call Stack）无内容时收成单条细条，hover 显示提示；
 **原则：空内容不应为占位文字预留整块高度**。
- **代码/源码行用 grid 不用 flex**：`.source-line` = `14px 44px 88px
 minmax(0,46ch) minmax(0,1fr)`（bp/ln/addr/asm/注释），注释列紧跟 asm 文本、
 超长 ellipsis，宽屏下不出现横向大空隙。新视图复用行布局时遵守该 grid 规则
 （如 `.disasm-line` 覆盖为 3 列）。

### 3.1 视觉语言（design tokens）

**亮色科技风主题（web/src/style.css :root）**：

- 背景：`--bg #f0f4f8`（浅灰蓝）+ body 双径向渐变光晕（左上靛蓝 6%、右下紫 5%）
- 面板：`--bg-panel rgba(255,255,255,0.82)` + `backdrop-filter: blur(12px)`（玻璃拟态），
 12px 大圆角，三级阴影（sm/shadow/lg）
- 边框两层：`--border #d0d9e4` / `--border-light #b8c4d4`
- 文本：`--text #1e293b`、`--text-dim #64748b`、`--text-bright #0f172a`
- 强调色：`--accent #4f46e5`（靛紫），header/primary 按钮用
 `--gradient-accent: linear-gradient(135deg, #6366f1, #8b5cf6)`，
 header 用 `--gradient-header`（#4f46e5→#7c3aed→#6366f1）
- 语义色：`--green #059669`、`--red #dc2626`、`--yellow #d97706`、
 `--orange #ea580c`、`--purple #7c3aed`、`--cyan #0891b2`、`--pink #db2777`，
 各配 `--*-soft` 浅底
- **反汇编语义着色**（`.inst-*` class）：算术=绿、逻辑=cyan、load=黄、store=橙、
 分支=紫、call=accent 粗体、ret=accent、system=红粗体、FPU=pink、未知=灰斜体
- **状态动效**：
 - 状态药丸：header 上为半透明白底 + 对应色文字 + 发光 pulse 点（box-shadow 光环）
 - 按钮：hover 上浮 1px + accent 光晕 + 浅紫底；active 回位
 - 寄存器 changed：琥珀色描边 + 浅黄底
 - PSW flag 置位：绿色浅底
 - 当前代码行：左侧 2px accent 指示条 + 浅紫底
 - Memory PC 行：浅紫底 + accent 左边框 + 字节级高亮
 - 滚动条：7px 透明轨道

### 3.2 交互约定（全局，新面板必须遵守）

| 交互 | 含义 |
|---|---|
| 单击行 | 切换该行地址的断点 |
| 双击行 | seek：PC 跳到这里（清空历史） |
| 右键行 | run to：运行到该行（临时断点） |
| F10 / F11 | 单步 / 跨函数 |
| F5 | 运行到断点或指令数上限 |
| D | 切换 Disasm/Listing 视图 |
| F | 切换 PC 自动跟随（手动模式可自由滚动） |
| 双击寄存器值 | 原地编辑（hex） |
| 断点行 | 红点 ● + 红色底 |
| PC 行 | 蓝色底 + 边框；自动跟随时视图居中 |

### 3.3 当前布局（web/，Vue 3）★

```
┌────────────────────────────────────────────────────────────────────┐
│ header: ◆ RH850/E1M-S2 | [RBASE input][Set] | 状态药丸 │
├────────────────────────────────────────────────────────────────────┤
│ ControlBar: [↺ Reset][▶ Run][→ Step][⤼ Over][■ Stop][✕ BPs] INS N │
├──────────────┬──────────────────────────────────┬──────────────────┤
│ Memory │ Call Stack (compact) │ Registers │
│ (16×8 hex, │ (heuristic, depth N) │ r0–r31 grid │
│ addr input) ├──────────────────────────────────┤ (changed=yellow)│
│ │ Disassembly (PC follow) │ ─────────────── │
│ (left col) │ click line = toggle breakpoint │ PSW = 0xNNNN │
│ ├──────────────────────────────────┤ [UM][CU2]...[V] │
│ │ Data Watch (add addr, read value) │ (right col) │
├──────────────┴──────────────────────────────────┼──────────────────┤
│ │ Instruction │
│ │ Trace (last 20) │
├──────────────────────────────────────────────────┴──────────────────┤
│ status bar: PC | INS | PSW | RBASE | ROM | BP | WATCH | STOP │
└────────────────────────────────────────────────────────────────────┘
```

**技术栈**：Vue 3 (Composition API, `<script setup>`) + TypeScript + Vite 5
**状态管理**：`useSimulator` composable + `provide/inject` 共享单例
**主题**：亮色科技风（见 §3.1），CSS 变量集中在 `web/src/style.css :root`
**WASM**：`web/src/sim/`（sim/build-wasm.sh 直接输出，无副本同步问题）

**文件加载**：header 📂 Open 按钮（file input）+ 窗口拖放，支持 .bin/.rom/.lst/.map
（parsers.ts 解析 listing 源码注释/符号表 → CodeView 显示，WatchPanel 符号列表一键 watch）。

## 4. 组件清单（web/，当前）

| 文件 | 组件 | 职责 |
|---|---|---|
| App.vue | — | 状态中枢：useSimulator + provide/inject |
| composables/useSimulator.ts | — | WASM 封装、状态 refs、所有 action（step/run/bp/seek/watch） |
| sim/index.ts | — | WASM 接口（SimInstance 类型 + createSim 单例） |
| parsers.ts | — | bin/.rom/.lst/.map 解析（listing 行格式 + map 符号表） |
| disasm.ts | — | JS 侧反汇编（渲染用，编码与 exec.rs 同步） |
| components/ControlBar | 控制条 | 顶部：Reset/Run/Step/StepOver/Stop/ClearBPs/INS |
| components/CodeView | 反汇编 | 中栏：PC 跟随，点击行设断点，源码注释 + 符号名 |
| components/RegisterPanel | 寄存器 | 右栏：r0–r31 网格 + 变化高亮 + PSW 标志位 |
| components/MemoryPanel | 内存 | 左栏：16×8 hex 视图 + 地址输入 + follow PC + PC 指针 |
| components/WatchPanel | 数据监视 | 中栏：手动添加地址 + map 符号列表 + 一键 watch |
| components/TracePanel | 指令追踪 | 右栏：最近 20 条指令简略反汇编 |
| components/CallStackPanel | 调用栈 | 中栏：JARL/JMP 启发式 push/pop |
| components/StatusBar | 状态栏 | 底部：PC/INS/PSW/RBASE/ROM/BP/WATCH/STOP |
| components/FaultBanner | 故障横幅 | PC 掉出 ROM 时显示 |

## 5. 功能路线图（迭代优化按此选择任务）

### Phase 1 — 已交付（基础调试闭环）

- [x] 核心执行：算术/逻辑/移位/乘除/访存/分支/系统/FPU 基本指令（exec.rs）
- [x] 断点、单步、跨函数、运行到游标、可逆执行（prev/next）
- [x] 反汇编视图（无 listing 也能用）、PC 跟随、变化高亮
- [x] ROM hex 视图、寄存器面板（PSW 位展开）、RAM watch、文件管理

### Phase 2 — 进行中（本迭代）

> 设计参考：硬件级 debug 工具（E1 Emulator / J-Link / IAR）的通用功能集（§6）。

- [x] **调用栈 Call Stack**：紧凑条在中列顶部，JARL/JMP 启发 push/pop，符号解析
- [x] **布局**：底部状态栏、header 状态药丸、亮色科技风主题
- [x] **web UI 初版**：Vue 3 三栏布局，拖放加载，快捷键，断点，变化高亮
- [x] **完整反汇编**：disasm.ts 移植到 web，全格式指令着色
- [x] **lst/map 加载**：parsers.ts 解析 listing（源码注释+符号名）+ map 符号表，
 WatchPanel 符号列表一键 watch
- [x] **Memory PC 指针 + follow PC**：PC 行高亮 + ◀ 标记 + 字节级高亮
- [ ] **内存多格式视图**：hex/u8/u16/s16/u32/f32/f64/C[]（web MemoryPanel 扩展）
- [ ] **面板拖拽调整大小**：列宽/行高 resizer
- [ ] **指令补全**：BIT0/1/R/C、SATADD/SATSUB、ADF/SBF、MOVR2R、LD.D/ST.D、FETRAP
- [ ] **暂停机制**：run 可真正中断（现在 runTo 同步大循环，只有 chunk 让出帧）

### Phase 3 — 后续（增强）

- [ ] G3K（PCU）核：第二核视图，FC 地址窗口映射
- [ ] 中断仿真：EIINT/FEINT 触发 + INTBP 向量表交互
- [ ] 断点增强：条件断点、取指断点
- [ ] 执行回放：完整状态录播/快放（依赖 FP 快照补全）
- [ ] 多核时序：双核交替步
- [ ] listing/map 解析器按真实样例收紧（待用户提供样例）
- [ ] 符号化：反汇编行内联符号名（`main` @ 0x00401234）
- [ ] 导出：session 快照（bin+状态）可分享

### 明确不做

- 不追求真实速度（百万指令/秒级）；WASM 性能已够观察
- 不实现完整 MPU / 总线时序 / 外设寄存器行为（I/O 区读 0 写丢弃）
- 不做后端/多用户（隐私优先，数据不出浏览器）

## 6. 功能参考矩阵（硬件 debug 工具对标）

| 功能 | J-Link/GDB | E1 Emulator | 本工具现状 | 计划 |
|---|---|---|---|---|
| 单步/跨函数/运行到 | ✓ | ✓ | ✓ | — |
| 代码断点 | ✓ | ✓ | ✓ | 条件断点 Phase 3 |
| 数据断点(watchpoint) | ✓ | ✓ | ✓ | 已完成（引擎 write_log 扫描 + UI BP 列） |
| 调用栈 | ✓ | ✓ | ✓（启发式） | — |
| 寄存器视图+编辑 | ✓ | ✓ | ✓ | FP 编辑 Phase 3 |
| 内存窗口多格式 | ✓ | ✓ | ✗（仅 16×8 hex） | **Phase 2** |
| 变量监视（符号） | ✓ | ✓ | ✓（map 符号表 + 手动地址） | 多格式 Phase 2 |
| 反汇编窗口 | ✓ | ✓ | ✓ | 符号化 Phase 3 |
| 源码视图 | ✓ | ✓ | ✓（待真实 listing） | — |
| 事件/异常日志 | ✓ | ✓ | ✗（stopReason 有） | 低优先 |
| 执行统计/热函数 | 部分 | ✓ | ✗（insCount 有） | 低优先 |
| 系统寄存器 | ✓ | ✓ | 部分（PSW/FPSR） | RBASE/EBASE 等显示 |
| 断点列表管理 | ✓ | ✓ | 散在各视图（状态栏 BP 圆点） | 断点面板 Phase 3 |
| 布局自定义 | ✓ | ✓ | ✗（固定三栏） | 面板拖拽 Phase 2 |
| **Fault/Run-off 检测** | ✗ | ✗ | **✓（PC 跑出 ROM 时顶部 Fault Banner + Seek back）** | — |
| 运行进度可视化 | 部分 | ✓ | **✓（ControlBar 底部 3px 进度条 + shimmer）** | — |

## 7. 引擎指令覆盖（当前）

已实现（exec.rs）：NOP、RIE、Bcond(Fmt III，8-bit disp，修正)、Format I 16 位
（ADD/SUB/AND/OR/XOR/MOV/CMP/JMP/NOT/MULH/DIVH/ZXB/SXB/ZXH/SXH/TST）、SLD/SST 16 位
（简化）、JR/JARL disp22、ADDI/MOVHI、LD.W/ST.W/LD.H/ST.H、EIRET/FERET、TRAP、
PUSHSP/POPSP、扩展 32 位（MUL/MULU/MAC/MACU/DIVU/SHL/SHR/SAR/JARL[r],r3/SETF/CTRET/SYSCALL）、
**HALT**（0x7E00 0x2000）、FPU Format F:I（ADDF.S/SUBF.S/MULF.S/DIVF.S/ADDF.D 等，RNE 舍入）。
`cargo test` 48 pass, 0 fail, 0 ignore。

编码事实来源：HANDOFF §5（已核实的指令编码，防重复查手册）。
**改动任何编码时：exec.rs 与 disasm.ts 同步 + tests/basic.rs 加向量。**

## 8. 设计原则（新代码/新功能必须遵守）

1. **引擎与 UI 分离**：新引擎能力必须进 Rust（可原生测试）；UI 只做渲染和交互。
 例外是 disasm.ts（渲染解码器），但它的编码改动必须与 exec.rs 同步。
2. **状态单一来源**：所有 CPU 状态读自 WASM 实例（simRef），Vue ref 只是
 渲染镜像。不要在组件里缓存 CPU 值。
3. **可逆性优先**：任何新执行路径（run、run-to、异常进入）都要考虑历史一致性。
4. **渐进增强**：缺 listing/map 时所有核心功能可用；有文件时增强。任何新功能
 不能把"只有 bin"的底线打破。
5. **交互约定全局统一**（§3.2）：新视图（Memory/Stack 等）必须支持
 断点单击/seek 双击/run-to 右键（如适用）+ PC 跟随。
6. **样式走 token**：新样式只用 `:root` 变量 + 语义 class，不写死颜色。
7. **性能底线**：每次 step 只重读变化需要的最小状态；run 用 chunk（10 万指令）
 + requestAnimationFrame 让出帧，禁止阻塞主线程 >50ms。
8. **测试**：Rust 侧新指令必须有 tests/basic.rs 向量；前端改动必须过
 `tsc -b && vite build` + dev server 各模块 200。

## 9. 已知缺陷（与 HANDOFF §4 同步）

- **FPU Format F:I 位域布局需对照手册核实**：marker `(w>>21)&0x7FF==0x230` 与
 op/type/sub 位域重叠，decode 逻辑可能有误
- **CALLT 编码需对照手册核实**：op6=0b01000 与 OR 冲突，guard 条件矛盾
- FPU 舍入 RZ/RP/RM 简化为 RNE；FPSR 只读显示，未实现 CC/XC/XE/RM 位
- SLD/SST 16 位 base 寄存器用 r0（可能是 r30/EP）
- 缺指令：BIT0/1/R/C、SATADD/SATSUB、ADF/SBF、MOVR2R、LD.D/ST.D
- **G3K（PCU 副核）未实现**：用户要求验证副核指令模拟，当前引擎仅覆盖 G3MH
- runTo 同步大循环（chunk 内不可打断）；Run to Cursor 用临时断点
- listing/map 解析器按 mkrom.py 格式 + 宽松启发式，真实 CS+/GCC 输出待验证
- **测试 ROM 数据路径**：test.rom 能跑到 HALT，但部分全局值与预期不符
 （g_total=38≠55 等，遗留，待排查）

## 10. 手册针对性优化清单

> 以下条目是**对照 RH850/E1M-S2 硬件手册（R01UH0603 §33 On-Chip Debugging Unit）
> 和 G3MH 软件手册（R01US0143）** 后，确认值得针对性优化的方向。
> 按"对用户调试体验价值 / 实现复杂度"排序。

### A. 调试器功能增强（对齐 OCD 功能）

| # | 功能 | 手册依据 | 现状 | 实现要点 | 优先级 |
|---|---|---|---|---|---|
| A1 | **数据断点 / Watchpoint** | HW §33.1(10) Event Detection：可检测"访问地址 + 访问数据"组合事件 | ✅ 已完成 | 引擎 `watchpoints: HashMap<u32, u8>` + 每步 write_log 扫描 + UI BP 列 + 状态胶囊 `pill-wp`。手动写内存也触发（OCD 对齐）。 | 完成 |
| A2 | **HALT 指令支持** | SW §7 Special Instruction，opcode 0x7E00 0120 | ✅ 已完成 | exec.rs ADDI arm 内 `if h0==0x7E00 && h1==0x2000` → park core。Step 释放，Reset 清除。状态胶囊 `pill-halt`。 | 完成 |
| A3 | **Single-step 时的数据变化追踪** | HW §33.1(1) 调试功能含 single step | 部分（changedRegs/changedAddrs 已有，但只在寄存器面板 flash） | 扩展：每步后把 write_log 的 (addr, old→new) 推到状态栏 toast，"ST.W 0xFEDF_1000: 0x000F0000 → 0x001F0000"。 | 中 |
| A4 | **执行轨迹 / PC 历史** | HW §33.3 Trace Control（软件 trace 输出） | 部分（history 有快照，但 UI 无"轨迹"视图） | 加一个"Trace"面板：列出最近 N 步的 (insCount, pc, disasm 文本, 寄存器变化数)。可折叠。 | 中 |
| A5 | **外设寄存器窗口** | HW 地址空间：1000_0000H–1FFF_FFFFH on-chip I/O（含所有外设寄存器） | ✗ 只建模 RAM | Memory 模型加 `io` 寄存器表（从手册提取常用寄存器：INTC0/OSTM0/WDT 等），UI 加"IO" tab 显示外设寄存器读写。 | 低（工作量大） |
| A6 | **中断绑定表可视化** | HW INTBP 寄存器：EIINTn → 通道 → 优先级 | ✗ 未做 | UI 加"INTC"面板：读 INTBP，显示 32 个通道的向量地址。需要引擎侧暴露 getIntbp。 | 低 |

### B. 界面 / 动效增强

| # | 功能 | 现状 | 实现要点 | 优先级 |
|---|---|---|---|---|
| B1 | **PC 移动"滑动"动画** | PC 行直接跳到新位置（无过渡） | DisasmView/SourceView/RomView 用 CSS `scroll-behavior: smooth` + 对 PC 行加 `transition: background 0.15s`；或者用 Framer Motion 的 `layout` 动画让 PC 行"滑"过去。 | 中 |
| B2 | **数据流可视化** | 寄存器/RAM 变化只有 flash | 加"数据流"指示：当一步写 RAM 时，Memory 面板对应地址短暂高亮（与 Watch 的 ▲ 联动）。引擎已有 write_log，UI 侧把 changedAddrs 传到 MemoryView 高亮。 | 中 |
| B3 | **断点命中动画** | 断点命中只有状态变化 | 命中时：断点行闪烁红色 2 次 + 状态栏 toast "BP hit @ 0x..." + 自动暂停。 | 低 |
| B4 | **暗色主题** | 只有亮色 | CSS 变量已经齐全，加 `prefers-color-scheme: dark` + 手动切换按钮。 | 低 |
| B5 | **键盘快捷键帮助常驻** | 只在 `?` 弹框 | 状态栏加一个迷你快捷键提示条（可折叠）。 | 低 |

### C. 引擎正确性（影响所有 UI 功能）

| # | 功能 | 现状 | 实现要点 | 优先级 |
|---|---|---|---|---|
| C1 | **JR/JARL disp22 符号扩展验证** | ✅ 已修：Format V 布局确认，test_jr_forward22/test_jarl_and_return 通过 | 拿一个真实 C 程序编译产物，验证 JARL 跳转。 | 完成 |
| C2 | **Bcond 目标地址计算** | ✅ 已确认：disp8 = bits15..8，8-bit 符号扩展，test_bcond 通过 | 与真实编译器输出交叉验证。 | 完成 |
| C3 | **PUSHSP/POPSP 栈帧 ABI** | ✅ 已修：word1 lo7=0x160，rh<=rt 升序 push，test_pushsp_popsp 通过 | 拿真实编译器输出验证 GP 重定位。 | 完成 |
| C4 | **FPU 舍入模式 / 异常** | 简化 RNE | 对照手册 §6 FPU 部分补测试。 | 中 |

### D. 工程化

| # | 功能 | 现状 | 实现要点 | 优先级 |
|---|---|---|---|---|
| D1 | **真实编译器输出测试** | 只有手造 test.rom | 用 Renesas CS+ 或 RH850 GCC 编译一个 hello world，拿 .bin + .lst + .map 做端到端测试。 | 高 |
| D2 | **Vite 构建产物体积优化** | dist 单 bundle，~90KB gzip | 按面板 code-split、按需懒加载。 | 低 |
| D3 | **单元测试补全** | Rust 48 pass，前端 0 测试 | 加 Vitest + Testing Library，覆盖 parsers / disasm / App 核心交互。 | 中 |

## 11. 构建与验证命令（每次迭代必须跑）

```bash
# 快速启动（无需 Node.js / Rust，仅需 Python 3）
./start.sh              # Linux/macOS
# .\start.ps1          # Windows PowerShell

# 1. 引擎测试（改了 Rust 才需要）
cargo test --manifest-path sim/Cargo.toml

# 2. 重编 wasm（改了 Rust 才需要；产物 → web/src/sim/）
# ⚠️ build-wasm.sh 内含 cargo clean --target wasm32-unknown-unknown
# （防缓存静默命中，浏览器继续跑旧代码）
bash sim/build-wasm.sh

# 3. 前端类型检查 + 构建
cd web && npm run build

# 4. 开发服务器（热重载，需 Node.js）
cd web && npm run dev          # http://127.0.0.1:5174

# 5. 测试 ROM 重新生成（改了 mkrom.py 才需要）
python3 testdata/mkrom.py      # → test.rom + test.lst + test.map

# 6. Node smoke test（验证 WASM 引擎）
node testdata/smoke.mjs        # 预期输出 SMOKE OK

# 7. 浏览器 E2E（chrome-headless-shell + CDP，见 testdata/cdp_test.js）
```

## 12. 文档地图

| 文档 | 作用 |
|---|---|
| HANDOFF.md | 交接记录：进度、决策、环境坑、更新日志（"做了什么"） |
| docs/DESIGN.md（本文件） | 架构 + 功能设计 + 路线图（"应该是什么样"） |
| docs/manual/r01uh0603ej0110.txt | 硬件手册（地址空间/核配置/异常） |
| docs/manual/r01us0143ej0130.txt | G3MH 软件手册（指令编码，查指令用） |
| docs/manual/r01us0165ej0120.txt | G3K 软件手册 |
