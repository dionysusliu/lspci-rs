# Remaining Extended Capability Decoders Design

日期：2026-08-10
状态：已与用户确认（brainstorming 定稿）

## 目标

补齐 sg-232e-224 上存在的其余扩展 capability 解码（B 切片第二轮）：
9 个协议型 + 2 个 dump 型，共 11 个 decoder。

| 扩展 Cap ID | 名称 | 形态 |
| ---: | --- | --- |
| 0x02 | Virtual Channel (VC) | 协议型（多寄存器组，真机校准） |
| 0x0b | Vendor Specific Extended | dump 型（vendor/rev + hex） |
| 0x0f | Address Translation Services (ATS) | 协议型 |
| 0x13 | Page Request Interface (PRI) | 协议型 |
| 0x17 | Transaction Processing Hints (TPH) | 协议型 |
| 0x18 | Latency Tolerance Reporting (LTR) | 协议型 |
| 0x19 | Secondary PCI Express | 协议型（均衡参数，真机校准） |
| 0x1b | Process Address Space ID (PASID) | 协议型 |
| 0x1d | Downstream Port Containment (DPC) | 协议型 |
| 0x1f | Precision Time Measurement (PTM) | 协议型 |
| 0x23 | Designated Vendor-Specific (DVSEC) | dump 型（vendor/rev/DVSEC ID + hex） |

注：用户确认的范围选项 A 为 DPC/PTM/TPH/LTR/VC/Secondary PCIe/PASID/ATS
+ Vendor-Ext/DVSEC；PRI 因机器存在样本（×2）且 decoder 极小，一并纳入，
避免下切片再补。

## 明确不做

- Lane Margining (0x27)、Phys Layer 16GT (0x26)、RCEC (0x07)、
  MR-IOV、Multicast、ReBAR、DPA、PM-Mux 等（机器上无样本或价值低）
- header 字段语义解读、配置空间写入
- 单元测试（沿用用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 验证环境

sg-232e-224 物理机（既有验证环境）：全部目标类型在机器上有样本
（DPC×29、PTM×29、Vendor-Ext×21、Secondary PCIe×13、DVSEC×12、
TPH×6、VC×4、LTR×4、PASID×4、PRI×2、ATS×2）。

## 架构（完全复用既有框架）

- decoder 纯函数 `fn decode_x(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<XCapability>`，
  新文件放 `crates/pci/src/decoders/`（vc.rs / vendor_ext.rs / ats.rs /
  pri.rs / tph.rs / ltr.rs / secondary_pcie.rs / pasid.rs / dpc.rs /
  ptm.rs / dvsec.rs）
- `PciCapabilityContent` 新增对应变体；`decode_content` 增加
  `(PciCapabilityKind::Extended, id)` 分发分支
- **预取调整**：extended 节点预取从 0x40 提高到 0x60（VC/DPC 等结构
  更大；0x1000 空间内无越界风险）
- dump 型 decoder 约定：结构总长度字段已知（vendor-ext 无长度则读取到
  下一个 cap 偏移；DVSEC 有 DVSEC Length 字段），只 dump 到结构边界，
  不越界到相邻 cap

## 各 decoder 字段集（lspci -vvv 基准，位布局实现时真机校准）

### DPC (0x1d)

cap/control/status 寄存器：触发原因位、中断使能位、DPC Status
（触发状态/原因/中断状态）、Error Source ID；RP PIO 相关寄存器以 hex 输出。

### PTM (0x1f)

ptm_cap（PTM Root Capable / Clock Capable 等能力位）、ptm_ctrl
（PTM Enable / Root Select）、PTM Granularity。

### TPH (0x17)

cap（ST Table 位置模式、No ST/Device Specific 模式支持、MSI-X 向量数）、
ctrl（ST Mode Select 等）；ST table 存于设备内时 dump 原始条目。

### LTR (0x18)

Max Snoop Latency、Max No-Snoop Latency：各含 value 与 scale 字段，
渲染为可对照 lspci 的形态。

### VC (0x02)

Extended VC Count、Port VC Capability/Control/Status、各 VC 的
Resource Capability/Control/Status。寄存器组较大，位布局以真机校准为准。

### Secondary PCIe (0x19)

Link Control 3 / Lane Equalization Control 等均衡相关寄存器；
位布局以真机校准为准。

### PASID (0x1b)

PASID Capability（Execute/Privileged Mode Supported、PASID 宽度）、
PASID Control。

### ATS (0x0f)

ATS Capability（Invalidate Queue Depth 等）、ATS Control。

### PRI (0x13)

PRI Control/Status、Outstanding Page Request Capacity/Allocation 等。

### Vendor Specific Extended (0x0b)

无标准结构：dump 至下一个 capability 偏移为止的原始字节（hex），
不解读内容。

### DVSEC (0x23)

DVSEC Vendor ID、Revision、DVSEC ID、DVSEC Length；Length 范围内
原始字节 hex dump。

## 降级条款（已与用户确认）

VC 与 Secondary PCIe 若真机校准成本过高，允许降级为
"原始寄存器 hex + 少量关键字段"，并在进度文档中明确记录降级范围。

## 渲染

- text：content 行（dump 型为单行 hex；VC/DPC 可多行）
- JSON：`content.type` 取值 `vc` / `vendor_ext` / `ats` / `pri` / `tph` /
  `ltr` / `secondary_pcie` / `pasid` / `dpc` / `ptm` / `dvsec`；
  寄存器 hex 字符串，位标志用 bool 或名称数组

## 验证

1. **sg-232e-224 真机对照**：每类选一个样本设备，与 `sudo lspci -vvv`
   逐项对照；VC/Secondary PCIe 的位表必须真机校准后才算完成
   （或按降级条款记录）。
2. **dev48/myece 不回归**：扩展链输出保持原状（不可读环境不变；
   dev48 上已有的标准 cap 解码不变）。
3. handoff 进度文档记录每类的样本设备、对照结果、校准/降级说明。

## 全局约束

- decoder 模块零 FFI；解码失败不得使 `inspect()` 失败。
- 不改变 `list` 行为；不引入新依赖。
- 分支策略：从 main 切 `sdd/remaining-extended-decoders`，完成后走
  finishing-a-development-branch。
