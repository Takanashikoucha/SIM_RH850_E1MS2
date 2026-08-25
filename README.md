# RH850/E1M-S2 程序执行仿真器

基于 Renesas RH850/E1M-S2 硬件架构的浏览器端 CPU 指令级仿真器，支持单步调试、断点、
寄存器/内存观察、反汇编视图和调用栈跟踪。

## 快速开始

> **依赖**：仅 Python 3（标准库）。预编译 WASM 引擎已包含在仓库中，
> 无需 Node.js、无需 Rust 工具链。
> ⚠️ 仓库约 14MB，其中 ~14MB 是 `docs/manual/` 下的 Renesas 手册文本
> （查指令编码用的参考，不用可忽略；PDF 原件不入库，见 `docs/manual/README.md`）。

```bash
# Linux / macOS — 无需 Node.js，Python 3 即可
./start.sh              # 打开 http://127.0.0.1:8080

# Windows (PowerShell)
.\start.ps1             # 打开 http://127.0.0.1:8080

# 自定义端口
./start.sh 9000

# 开发模式（热重载，需 Node.js + npm）
./start.sh --dev        # 打开 http://127.0.0.1:5174
```

打开浏览器后，点击 **📂 Open** 加载测试 ROM：
`testdata/test.rom`（由 `testdata/mkrom.py` 生成的单文件汇编器输出，
覆盖 C 全语法的测试程序，从 `main()` 运行到 `HALT`）。

## 项目结构

```
├── web/               前端（Vue 3 + TypeScript + Vite 5）
│   ├── src/
│   │   ├── sim/       WASM 引擎产物（预编译，无需 Rust 即可运行）
│   │   ├── components/  UI 面板（控制条/代码/寄存器/内存/监视/追踪/调用栈）
│   │   ├── composables/  useSimulator 状态管理
│   │   └── style.css  亮色科技风主题（design tokens）
│   └── dist/          构建产物（已提交，Python 静态服务直接加载）
├── sim/               Rust 引擎源码（wasm-bindgen 输出 cdylib）
│   └── src/           state/mem/fpu/exec/exception
├── testdata/          测试 ROM + 单文件汇编器（mkrom.py）
├── docs/
│   ├── DESIGN.md      设计文档（架构/组件/路线图）
│   └── manual/        RH850/E1M-S2 手册文本提取（指令编码参考）
├── HANDOFF.md         工作交接记录 + 路线图（下一步行动）
├── serve.py           Python 3 零依赖静态服务器
├── start.sh           Linux/macOS 一键启动
└── start.ps1          Windows PowerShell 一键启动
```

## 架构

```
浏览器 (纯前端，无后端)
│
├── Vue 3 + TS 前端 (web/)
│   ├── 10 个 UI 组件（控制条/反汇编/寄存器/内存/监视/追踪/调用栈/状态栏）
│   └── useSimulator composable（状态管理 + WASM 封装）
│
└── WASM 引擎 (web/src/sim/sim_bg.wasm)
    ├── state.rs   G3 寄存器文件 + PCW + 条件码 + 异常向量
    ├── mem.rs     E1MS2 地址映射（flash/RAM/I/O）
    ├── fpu.rs     浮点单元（f64 后端，RNE 舍入）
    ├── exec.rs    指令解码 + 执行（~200 条 G3 指令）
    └── exception.rs 异常/中断处理
```

## 功能

- **单步调试**：Step / Step Over / Reset / Run
- **断点**：点击代码行设置，ClearBPs 清除
- **寄存器观察**：r0–r31 + PC + PSW（含标志位展开），变化高亮，可编辑
- **内存观察**：16×8 hex 视图 + PC 跟随 + 地址跳转
- **数据监视**：手动添加地址 + map 符号表一键 watch
- **反汇编**：JS 侧解码（与 exec.rs 同步），指令类型着色
- **调用栈**：JARL/JMP 启发式 push/pop
- **指令追踪**：最近 20 条指令简略反汇编
- **文件加载**：.rom/.bin（原始 ROM）、.lst（编译器 listing）、.map（符号表）

## 开发

```bash
# 修改 Rust 引擎后重新编译 WASM
bash sim/build-wasm.sh    # 自动 cargo clean + build + wasm-bindgen

# 修改前端后（开发模式自动热重载）
cd web && npm run dev

# 前端类型检查 + 构建
cd web && npm run build

# 引擎单元测试
cd sim && cargo test --test basic   # 48 tests

# 重新生成测试 ROM（修改 mkrom.py 后）
python3 testdata/mkrom.py
```

### 构建 WASM 的环境要求

Rust 工具链 + `wasm32-unknown-unknown` target + `wasm-bindgen` CLI。
`sim/build-wasm.sh` 会自动处理（含 `cargo clean --target wasm32-unknown-unknown` 防缓存）。

## 测试

```bash
# Node smoke test（验证 WASM 引擎 + 测试 ROM）
node testdata/smoke.mjs    # 预期输出 SMOKE OK（HALT after 312 instructions）

# 浏览器 E2E（chrome-headless-shell + CDP，需本地有 .playwright-browsers/）
# 见 testdata/cdp_test.js 的用法
```

## 文档

- **HANDOFF.md** — 工作交接记录 + 路线图（下一步行动 / 已知缺陷 / 指令编码参考）
- **docs/DESIGN.md** — 设计文档（架构 / 组件清单 / 功能矩阵 / 构建命令）
- **docs/manual/** — RH850/E1M-S2 手册文本（硬件手册 + G3MH/G3K 软件手册，~14MB）

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 UI | Vue 3 + TypeScript + Vite 5 |
| 引擎 | Rust (cdylib) → wasm-bindgen → WebAssembly |
| 静态服务 | Python 3 标准库（零依赖） |
| 开发服务器 | Vite 5（热重载） |
| 测试 | Rust cargo test + Node smoke + 浏览器 CDP E2E |
