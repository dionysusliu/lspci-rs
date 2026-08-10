# More Standard Capability Decoders Design

日期：2026-08-10
状态：已与用户确认（brainstorming 定稿）

## 目标

在已合并的五个标准 capability decoder（PM/MSI/MSI-X/PCIe/Vendor Specific）
基础上，补齐其余常见标准 capability 的解码：

| Cap ID | 名称 | dev48 可对照 |
| ---: | --- | --- |
| 0x03 | VPD | 无设备（结构解码，记录不可验证） |
| 0x04 | Slot Identification | ✅（QEMU PCI-PCI bridge 00:1f.0） |
| 0x07 | PCI-X | 无设备（结构解码，记录不可验证） |
| 0x0c | Hot-Plug | ✅（QEMU PCI-PCI bridge 00:1f.0） |

## 明确不做

- 扩展 capability decoder（AER/SR-IOV 等，用户决定先 A 后 B；
  B 的真机验证环境已确认为 sg-232e-224 物理机：扩展空间 4096 字节
  完整可读，网卡 3d:00.0 具备 AER/DSN/ARI/SR-IOV/TPH/ACS）
- VPD 数据内容读取（`pci_read_vpd`）：本切片只解码 VPD capability
  的地址/数据寄存器结构
- 单元测试（沿用用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 架构

完全复用既有 decoder 框架，无新机制：

- decoder 为纯函数 `fn decode_x(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<XCapability>`
- `PciCapabilityContent` 枚举增加四个变体
- `decode_content` 分发增加 0x03/0x04/0x07/0x0c 四个分支
- session 预取逻辑不变（现有 64 字节预取已覆盖全部四个协议）
- text/JSON 渲染沿用既有模式（content 行 / `content.type` 鉴别字段）

## 各 decoder 字段集

### Slot Identification (0x04)

```rust
pub struct SlotIdCapability {
    pub slot: u8,      // cap+2 字节
    pub chassis: u8,   // cap+3 字节
}
```

Text 渲染对齐 lspci 形态：`slot_id=<n> first=<bool> chassis=0x<nn>`
（lspci 显示 `Slot ID: 0 slots, First+, chassis 01`；"First" 语义按
slot 字节编码在实现时对照 lspci ls-caps.c 确定，dev48 真机校验）。

### Hot-Plug (0x0c)

```rust
pub struct HotPlugCapability {
    pub hot_plug_capable: bool,  // cap+2 字节 bit 0
}
```

Text：`hot_plug_capable=true`（对照 lspci `Hot-plug capable`）。

### VPD (0x03)

```rust
pub struct VpdCapability {
    pub address_flag: bool,  // 地址寄存器 bit 15 (F)
    pub address: u16,        // 地址寄存器 bits 0-14
    pub data: u16,           // cap+4 word
}
```

地址寄存器 word 位于 cap+2，数据寄存器 word 位于 cap+4。
不实现 VPD 内容事务读取。

### PCI-X (0x07)

```rust
pub struct PciXCapability {
    // 命令寄存器（cap+2 word）
    pub parity_error_recovery: bool, // bit 0
    pub relaxed_ordering: bool,   // bit 1
    pub max_memory_block: u8,     // bits 2-3
    pub max_split: u8,            // bits 4-6
    // 状态寄存器（cap+4 dword）
    pub bus: u8,                  // bits 8-15
    pub device: u8,               // bits 3-7
    pub function: u8,             // bits 0-2
    pub status_raw: u32,          // 完整状态原始值
}
```

位布局按 PCI-X 规范；dev48 无 PCI-X 设备，记录为"本环境不可验证"。

## JSON 输出

`content.type` 取值：`slot_id`、`hot_plug`、`vpd`、`pci_x`。
字段序列化约定沿用既有：bool 用 bool，计数/编号用 number，
原始寄存器值用 `0x...` hex 字符串。

## 验证

1. **dev48 真机对照**（0000:00:1f.0）：Slot Identification 与 Hot-Plug
   的解码结果与 `sudo lspci -s 00:1f.0 -vv` 对照一致。
2. **VPD / PCI-X**：dev48 无对应设备，仅保证编译与分发正确，
   在进度文档中记录"本环境不可验证"。
3. **myece 不回归**：fmt/check/list/show 输出与合并前一致。

## 全局约束

- decoder 模块零 FFI；decoder 失败不得使 `inspect()` 失败。
- 不改变 `list` 行为。
- 分支策略：从 main 切 `sdd/more-standard-decoders`，完成后走
  finishing-a-development-branch。
