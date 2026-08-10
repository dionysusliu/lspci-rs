# PCIe Capability Full Decoding Design

日期：2026-08-10
状态：已与用户确认（brainstorming 定稿）

## 目标

将 PCIe capability（标准 cap ID 0x10）解码扩展到与 `lspci -vv` Express
块完全对齐（约 20 行输出）：DevCap/DevCtl/DevSta 全位展开、
LnkCap/LnkCtl/LnkSta 全位展开、SlotCap/SlotCtl/SlotSta（slot 设备）、
RootCtl/RootSta（root port）、以及 v2 的 DevCap2/DevCtl2/LnkCtl2/LnkSta2
寄存器组。

当前实现只有：version / device_type / slot_implemented /
interrupt_message_number / dev_ctl / dev_sta 原始 word / link
speed-width / link_training。本切片替换 `decoders/pcie.rs` 为全量版本。

## 明确不做

- 其他 capability 的补齐（AER root 组、SR-IOV VF BAR 等，下一切片）
- 配置空间写入
- 单元测试（沿用用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 验证环境

- **sg-232e-224**：X710 网卡 endpoint（PCIe cap v2，含 *2 寄存器组）
  + root port（Slot/Root 寄存器组），对照 `sudo lspci -vv`
- **dev48**：QEMU NVMe endpoint / PCI-PCI 桥，辅助对照
- 位表以 PCI Express 规范为准，真机校准兜底（既有流程）

## 架构

- 替换 `crates/pci/src/decoders/pcie.rs`：`PcieCapability` 重构为
  结构化寄存器组，按设备类型条件存在（`Option` = 该类型设备无此寄存器）
- 预取无需调整：结构止于 cap base +0x3C，现有 extended 预取 0x60 足够
- text 渲染：pcie 条目特殊化为多行（类似 AER 的处理）
- JSON：嵌套结构化对象，条件字段 `skip_serializing_if`

## 寄存器映射（相对 cap base）

| 偏移 | 寄存器 | 宽度 | 存在条件 |
| ---: | --- | --- | --- |
| +0x02 | Capabilities | word | 总是 |
| +0x04 | Device Capabilities | dword | 总是 |
| +0x08 | Device Control | word | 总是 |
| +0x0A | Device Status | word | 总是 |
| +0x0C | Link Capabilities | dword | 总是 |
| +0x10 | Link Control | word | 总是 |
| +0x12 | Link Status | word | 总是 |
| +0x14 | Slot Capabilities | dword | slot_implemented |
| +0x18 | Slot Control | word | slot_implemented |
| +0x1A | Slot Status | word | slot_implemented |
| +0x1C | Root Control | word | root port |
| +0x20 | Root Status | dword | root port |
| +0x24 | Device Capabilities 2 | dword | version >= 2 |
| +0x28 | Device Control 2 | word | version >= 2 |
| +0x30 | Link Control 2 | word | version >= 2 |
| +0x32 | Link Status 2 | word | version >= 2 |

### Capabilities (+0x02 word)

version bits0–3；device/port type bits4–7（0 endpoint、1 legacy endpoint、
4 root port、5 upstream switch、6 downstream switch、7 PCIe-PCI 桥、
8 PCI-PCIe 桥、9 RCiEP、0xa RC Event Collector）；slot_implemented bit8；
interrupt message number bits9–13。

### Device Capabilities (+0x04 dword) → lspci `DevCap:`

MaxPayload 支持 bits0–2（128/256/512/1024/2048/4096）；Phantom Functions
bits3–4；Extended Tag bit5；L0s Acceptable Latency bits6–8；
L1 Acceptable Latency bits9–11；RBE（Role-Based Error Reporting）bit15；
Attention Button bit16 / Attention Indicator bit17 / Power Indicator bit18；
FLReset bit28；Slot Power Limit value bits25–18 与 scale bits27–26
（渲染为 W）。

### Device Control (+0x08 word) → lspci `DevCtl:`

错误报告使能：CorrErr bit0、NonFatalErr bit1、FatalErr bit2、UnsupReq
bit3；Relaxed Ordering bit4；MaxPayload bits5–7；Extended Tag bit8；
Phantom Functions bit9；Aux Power bit10；No Snoop bit11；MaxReadReq
bits12–14（128–4096）；Bridge Config Retry bit15。

### Device Status (+0x0A word) → lspci `DevSta:`

CorrErr bit0、NonFatalErr bit1、FatalErr bit2、UnsupReq bit3、
AuxPwr bit4、TransPend bit5。

### Link Capabilities (+0x0C dword) → lspci `LnkCap:`

Max Link Speed bits0–3（gen 编码）；Max Width bits4–9；ASPM Support
bits10–11；L0s Exit Latency bits12–14；L1 Exit Latency bits15–17；
ClockPM bit18；Surprise Down bit19；DLL Active bit20；Link BW Notif
bit21；ASPM Opt Compliance bit22；Port Number bits24–31。

### Link Control (+0x10 word) → lspci `LnkCtl:`

ASPM Control bits0–1（Disabled/L0s/L1/L0sL1）；RCB bit3；Link Disable
bit4；Retrain bit5；Common Clock bit6；Extended Synch bit7；ClockPM
bit8；Autonomous Width Disable bit9；BW Interrupt bit10；
Autonomous BW Interrupt bit11。

### Link Status (+0x12 word) → lspci `LnkSta:`

Current Speed bits0–3；Negotiated Width bits4–9；Link Training bit11；
Slot Clock bit12；DLL Active bit13；BW Management bit14；
Autonomous BW bit15。`(downgraded)` 由 Current Speed < LnkCap Max Speed
或 Width < Max Width 推导（lspci 形态：`Speed 8GT/s (downgraded)`）。

### Slot Capabilities/Control/Status（slot_implemented）

SlotCap (+0x14 dword)：Attention Button bit0、Power Controller bit1、
MRL bit2、Attention Indicator bit3、Power Indicator bit4、
Hotplug Surprise bit5、Hotplug Capable bit6、Slot Power Limit
bits7–14/15–16、Electromechanical bit17、No Command Completed bit18、
Physical Slot Number bits19–31。
SlotCtl (+0x18 word) 与 SlotSta (+0x1A word) 对应使能/状态位，
渲染为 lspci `SlotCap:/SlotCtl:/SlotSta:` 形态。

### Root Control/Status（root port）

RootCtl (+0x1C word)：SERR 使能 bits0–2、PME Interrupt bit3、
CRS Visible bit4。
RootSta (+0x20 dword)：PME Requester ID bits0–15、PME Status bit16、
PME Pending bit17 等，渲染为 lspci `RootCtl:/RootSta:` 形态。

### v2 寄存器组（version >= 2）

渲染目标为 lspci 的对应行（位常量以实现时真机校准为准）：

- `DevCap2:`：Completion Timeout 范围、TimeoutDis、NROPrPrP、LTR、
  10BitTagComp/10BitTagReq、OBFF、ExtFmt、EETLPPrefix、
  EmergencyPowerReduction(Init)、FRS、TPHComp/ExtTPHComp、
  AtomicOpsCap（32bit/64bit/128bitCAS）
- `DevCtl2:`：Completion Timeout 选择、TimeoutDis、LTR、10BitTagReq、
  OBFF 控制、AtomicOpsCtl ReqEn
- `LnkCtl2:`：Target Speed、Compliance De-emphasis、Transmit Margin、
  Enter Modified Compliance、Compliance SOS、De-emphasis Level
- `LnkSta2:`：Current De-emphasis、EqualizationComplete/Phase1/2/3、
  LinkEqualizationRequest、Retimer、CrosslinkRes

## 渲染

### Text（多行，挂在 pcie 节点 content 下）

```text
      pcie id=0x0010 offset=0x0a0 next=none state=Valid
        content: version=2 type=endpoint slot=false
          DevCap: MaxPayload 256 bytes, PhantFunc 0, Latency L0s <64ns, L1 <1us
                  ExtTag+ AttnBtn- AttnInd- PwrInd- RBE+ FLReset+ SlotPowerLimit 0W
          DevCtl: CorrErr- NonFatalErr- FatalErr- UnsupReq-
                  RlxdOrd- ExtTag+ PhantFunc- AuxPwr- NoSnoop-
                  MaxPayload 256 bytes, MaxReadReq 256 bytes
          DevSta: CorrErr- NonFatalErr- FatalErr- UnsupReq- AuxPwr- TransPend-
          LnkCap: Port #0, Speed 8GT/s, Width x8, ASPM not supported, Exit Latency L0s <64ns, L1 <1us
                  ClockPM- Surprise- LLActRep- BwNot- ASPMOptComp+
          LnkCtl: ASPM Disabled; RCB 64 bytes, Disabled- CommClk+
                  ExtSynch- ClockPM- AutWidDis- BWInt- AutBWInt-
          LnkSta: Speed 8GT/s (ok), Width x8, TrErr- Train- SlotClk+ DLActive- BWMgmt- ABWMgmt-
```

v2 设备追加 DevCap2/DevCtl2/LnkCtl2/LnkSta2 行；slot/root 设备追加
SlotCap/SlotCtl/SlotSta、RootCtl/RootSta 行。

### JSON

`JsonPcie` 重构：基础字段 + 嵌套对象 `dev_cap` / `dev_ctl` / `dev_sta` /
`lnk_cap` / `lnk_ctl` / `lnk_sta`（均为 bool/数值字段对象）+
`slot_*` / `root_*` / `*_2` 条件对象（不存在时省略）。

## 验证

1. **sg-232e-224**：X710 endpoint（v2，含 *2 组）与 root port（Slot/Root
   组），与 `sudo lspci -vv` Express 块逐行对照
2. **dev48**：NVMe endpoint / 桥，辅助对照
3. **myece**：不回归（扩展链不可读，输出不变）
4. 位表差异按既有校准流程修正并记录

## 全局约束

- decoder 模块零 FFI；解码失败不得使 `inspect()` 失败。
- 不改变 `list` 行为；不引入新依赖。
- 分支策略：从 main 切 `sdd/pcie-full-decoding`，完成后走
  finishing-a-development-branch。
