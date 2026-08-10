# Read-Only Decoder Enhancements Design

日期：2026-08-11
状态：已与用户确认（brainstorming 定稿）

## 目标

四项只读解码增强（配置空间写入明确推迟）：

1. **LnkCap2** 寄存器解码（PCIe cap +0x2c，v2 设备）
2. **Latency/Timeout 编码文本化**（DevCap L0s/L1、LnkCap Exit Latency
   的 PCIe 编码 → ns/µs 文本）
3. **AER Root 寄存器组**（root port：Root Error Command/Status、
   Error Source ID）
4. **SR-IOV VF BAR** 解码（VF BAR0–5 类型与地址、VF Migration 寄存器）

## 明确不做

- 配置空间写入（用户明确推迟）
- 新增 capability 类型（DP C、ACS 之外的新 cap）
- 单元测试（沿用用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 验证环境

- **sg-232e-224**：X710 v2 endpoint（LnkCap2、SR-IOV Region 0/3，
  SR-IOV 未使能但 VF BAR 寄存器有值）；root port（AER root 组）；
  latency 文本对照 lspci
- **dev48**：辅助对照
- **myece**：回归（扩展链不可读，输出不变）
- 位定义以 PCI Express 规范为准，真机校准兜底（既有流程）

## 架构（完全复用既有框架）

- 扩展现有 decoder 文件（`decoders/pcie.rs`、`decoders/aer.rs`、
  `decoders/sriov.rs`），不新增 capability 类型
- 数据来自已预取的 snapshot（PCIe cap 预取 0x60 覆盖 +0x2c；
  SR-IOV/AER 扩展 cap 预取 0x60 覆盖 VF BAR 与 root 组）
- latency 编码换算为渲染层纯函数（PCIe 编码表）
- text/JSON 渲染扩展（沿用现有模式）

## 各项设计

### 1. LnkCap2（PcieCapability 增加 `lnk_cap2` 条件组，v2 时读取 +0x2c）

dword +0x2c：
- bits7–1：Supported Link Speeds Vector（bit1=2.5、bit2=5、bit3=8、
  bit4=16、bit5=32、bit6=64、bit7=…按规范）
- bit0：Crosslink Supported
- bit8：Retimer Presence Detected Supported、bit9：2Retimers
- bit10：DRS Supported（按规范 DRS 位；以真机校准）

渲染对齐 lspci：
`LnkCap2: Supported Link Speeds: 2.5-8GT/s, Crosslink- Retimer- 2Retimers- DRS-`

### 2. Latency 编码文本化

PCIe 编码表（3-bit）：
- L0s Exit/Acceptable：0=<64ns、1=<128ns、2=<256ns、3=<512ns、
  4=<1us、5=<2us、6=<4us、7=>4us
- L1 Exit/Acceptable：0=<1us、1=<2us、2=<4us、3=<8us、4=<16us、
  5=<32us、6=<64us、7=>64us

应用点：
- DevCap：`Latency L0s <512ns, L1 <64us`（现输出原始编码值，改文本）
- LnkCap：`Exit Latency L0s <64ns, L1 <1us`（同上）
- JSON：保留原始编码值字段，新增文本字段或替换为文本（以 spec 实现时
  与既有 JSON 字段兼容为准：保留原值，text 渲染用换算）

### 3. AER Root 寄存器组（AerCapability 增加 root 组字段）

仅 root port（device_type==4 的 PCIe cap 所挂设备的 AER cap；
判定以 PCI header type==bridge 为准，从 snapshot 0x0e 读取）：

| 偏移（AER cap 内） | 寄存器 | 解码 |
| ---: | --- | --- |
| +0x2c | Root Error Command | CE 报告使能 bit0、NF bit1、Fatal bit2 |
| +0x30 | Root Error Status | CE Received bit0、Multiple CE bit1、NF bit2、Multiple NF bit3、Fatal bit5、Multiple Fatal bit6、First UE Fatal bit4、First UE Non-Fatal bit7？bits 按规范：bits27-16 Advanced Error Interrupt Message Number 等；以真机校准 |
| +0x34 | Error Source Identification | bits15–0 CE Source ID、bits31–16 NF/Fatal Source ID |

渲染对齐 lspci AER root 输出（`RootErrCmd:`、`RootErrSta:`、`ErrSrc:`）。

### 4. SR-IOV VF BAR

SriovCapability 增加：

- `vf_bars: [Option<SriovVfBar>; 6]`，VfBar { kind: Io/Memory,
  is_64_bit, prefetchable, address: u64 }（解码口径与 header BAR 一致：
  bit0=IO、bits2–1 类型、bit3 prefetch；64 位 BAR 占两个槽位，
  第二个槽位为 None）
- VF Migration State Array Offset（+0x38 dword）与
  VF Migration State Array Size（+0x3c dword）
  （现 PciSriovCapability 已有 migration_state_array_offset；补齐 size）

渲染对齐 lspci `Region N: Memory at ... (64-bit, prefetchable) [size=...]`
形态（IO 类型显示 `I/O ports at ...`）。

## 验证

1. **sg-232e-224**：X710 LnkCap2 对照 lspci；SR-IOV Region 0/3 对照；
   root port AER root 组对照；latency 文本对照
2. **dev48**：辅助对照
3. **myece**：回归（扩展链不可读，输出不变）
4. 差异按既有校准流程修正并记录

## 全局约束

- decoder 模块零 FFI；解码失败不得使 `inspect()` 失败。
- 不改变 `list` 行为；不引入新依赖；不做配置空间写入。
- 分支策略：从 main 切 `sdd/readonly-enhancements`，完成后走
  finishing-a-development-branch。
