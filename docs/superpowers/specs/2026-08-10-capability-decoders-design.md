# Capability Protocol Decoders Design

日期：2026-08-10
状态：已与用户确认（brainstorming 定稿）

## 目标

在现有通用 capability 链发现（walker）之上，为标准 PCI capability 增加
**协议级解码**：把 capability 的配置空间字节解读为类型化字段，
输出对齐 `lspci -vv` 的可见字段集。

本切片范围（5 个标准 capability）：

| Cap ID | 名称 |
| ---: | --- |
| 0x01 | Power Management (PM) |
| 0x05 | MSI |
| 0x09 | Vendor Specific |
| 0x10 | PCI Express |
| 0x11 | MSI-X |

## 明确不做

- 扩展 capability decoder（AER、SR-IOV 等）
- header 字段语义解读（Command/Status、BAR 类型等）
- VPD 读取
- 配置空间写入
- 单元测试与 fixture 文件（用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 环境事实

- myece ECS（容器 `95c90e05ab1a`）：配置空间仅 `0x000..0x040` 可读，
  capability 区域不可读，**无法**在本环境验证解码结果。
- dev48（11.161.48.161，Alibaba Cloud Linux 3，x86_64）：root 下标准
  256 字节配置空间可读，存在真实 capability 设备（virtio MSI-X、
  QEMU NVMe 控制器等）。作为本切片的真机验证环境。

## 领域模型

### 新增内容枚举

```rust
pub enum PciCapabilityContent {
    Pm(PmCapability),
    Msi(MsiCapability),
    MsiX(MsiXCapability),
    Pcie(PcieCapability),
    VendorSpecific(VendorSpecificCapability),
}
```

### PciCapability 增加字段

```rust
pub struct PciCapability {
    pub id: u16,
    pub kind: PciCapabilityKind,
    pub offset: u16,
    pub next: Option<u16>,
    pub state: PciCapabilityState,
    pub content: Option<PciCapabilityContent>,   // 新增
}
```

语义：

- `Some(...)`：解码成功
- `None`：没有对应 decoder，或 `state != Valid` 无法解码。
  原因由既有 `state` 字段解释，**不重复建模**可用性。
- payload 部分不可读时 decoder 尽力解码；关键字节缺失则返回 `None`，
  不引入部分解码类型。

设计取舍（已与用户确认）：不用 `PciField<PciCapabilityContent>`，
避免与 `state` 表达同一"不可读"事实造成冗余。

## Decoder 架构

### 纯函数 + ConfigSpaceSnapshot 输入

decoder 不接触 FFI。给 `ConfigSpaceSnapshot` 增加纯函数读取方法：

```rust
impl ConfigSpaceSnapshot {
    pub fn read(&self, offset: u32, length: u32) -> Result<Vec<u8>, ConfigReadError>;
}
```

从现有 `ConfigSpaceReader::read` 下沉 segment 查找/拼接逻辑，
reader 的 `read` 复用该方法（fetch 后调用）。

### 统一签名与分发

```rust
pub(crate) fn decode_content(
    snapshot: &ConfigSpaceSnapshot,
    capability: &mut PciCapability,
)
```

按 `capability.id` 分发到各 decoder：

```rust
fn decode_pm(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PmCapability>;
fn decode_msi(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<MsiCapability>;
fn decode_msix(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<MsiXCapability>;
fn decode_pcie(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PcieCapability>;
fn decode_vendor_specific(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<VendorSpecificCapability>;
```

新增模块 `crates/pci/src/decoders/`（每个协议一个文件 + `mod.rs` 分发）。
仅解码 standard 链；extended 链节点的 `content` 恒为 `None`。

### Session 集成

`inspect()` 中链发现之后，用同一个已缓存 reader 的 snapshot 对 standard
链节点逐个调用 `decode_content`，然后 snapshot 照旧丢弃——
保持"普通 inspection 不保留 raw config"的既有约束。

## 各 decoder 字段集（对齐 lspci -vv）

### PM (0x01)

寄存器：PMC (cap+2, word)、PMCSR (cap+4, word)。

字段：version、PME_Support、PMEClk、DSI、AuxCurrent、D1_Support、
D2_Support、各 D 状态 PME 支持、当前 PowerState (D0–D3)、PME_Enable、
PME_Status、No_Soft_Reset、Data_Select、Data_Scale。

### MSI (0x05)

寄存器：Message Control (cap+2, word)、Address (cap+4, dword)、
Upper Address（64-bit 时 cap+8）、Data。

字段：enable、multiple_message_capable / multiple_message_enable
（lspci 的 Count=1/8 形式）、64-bit、maskable、address、data。

### MSI-X (0x11)

寄存器：Message Control (cap+2, word)、Table Offset/BIR (cap+4, dword)、
PBA Offset/BIR (cap+8, dword)。

字段：enable、count（table size，bits 10:0 + 1）、masked（function mask）、
vector table（BIR + offset）、PBA（BIR + offset）。

### PCIe (0x10)

寄存器：PCI Express Capabilities (cap+2, word)、DevCap/DevCtl/DevSta、
LnkCap/LnkCtl/LnkSta、SlotCap/SlotCtl/SlotSta、RootCtl/RootSta。

字段：capability version、device/port type、slot implemented、
interrupt message number；DevCtl 关键控制位与 DevSta 状态位；
LnkCap/LnkSta 的 speed/width、链路训练与活动状态；Slot/Root 寄存器
按 device/port type 决定是否存在（type 无关的寄存器跳过，不显示）。
以 `lspci -vv` 实际输出为字段对齐基准。

### Vendor Specific (0x09)

字段：length（cap+2 字节）+ 后续原始字节的 hex 串。不做协议解释。

## CLI 输出

### Text

capability 条目下增加 content 行（key=value，逗号分隔，单行）：

```text
    standard: chain=complete
      msix id=0x0011 offset=0x040 next=none state=Valid
        content: enable=true count=2 masked=false table=BAR1+0x0 pba=BAR1+0x800
```

### JSON

capability 对象增加 `content` 字段，带类型鉴别字段；无内容时省略：

```json
{
  "id": "0x0011",
  "offset": "0x040",
  "state": "Valid",
  "content": {
    "type": "msix",
    "enable": true,
    "count": 2,
    "masked": false,
    "table": { "bar": 1, "offset": "0x0" },
    "pba": { "bar": 1, "offset": "0x800" }
  }
}
```

约定：数值型寄存器用 JSON number，地址/位域类用 `0x...` hex 字符串，
布尔用 bool，BAR 引用用 `{bar, offset}` 对象。

## 验证（只做真机对照，无单元测试）

1. **dev48 真机对照**：容器编译的二进制（同为 Alibaba Cloud Linux 3，
   glibc 兼容）scp 到 dev48，sudo 运行：

   ```bash
   sudo /tmp/lspci-rs show 0000:00:03.0 --format text   # virtio MSI-X
   sudo lspci -s 00:03.0 -vv                            # 对照基准
   ```

   逐字段对照。覆盖设备：virtio（MSI-X）、QEMU NVMe 控制器
   （MSI-X，可能含 PCIe cap）、存在 PM 的桥设备。
   二进制不兼容时的退路：在 dev48 上装 rustup 直接编译。

2. **myece 不回归检查**：容器内 `show` 仍正确显示
   `chain=unavailable: ReadError`，解码路径在字节不可读时不 panic、
   不改变现有输出。

## 全局约束

- 所有 unsafe/FFI 保持在 pci crate 既有边界内；decoder 模块零 FFI。
- decoder 失败（字节不可读）不得使 `inspect()` 整体失败。
- 不改变 `list` 行为；`list` 仍不读配置空间。
