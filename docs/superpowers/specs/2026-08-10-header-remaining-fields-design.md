# Remaining Header Fields Decoding Design

日期：2026-08-10
状态：已与用户确认（brainstorming 定稿）

## 目标

补齐 PCI 标准 Header（0x00–0x3F）剩余字段的语义解码，覆盖 Type 0
通用字段与 Type 1 桥专属字段，渲染对齐 `lspci -v` 形态。

已解码（不重复）：vendor/device/class/revision/subsystem/IRQ number、
Command/Status 寄存器、BAR 类型、capability 链与 23 个 decoder。

## 明确不做

- Type 2（CardBus）桥的完整窗口解码（CIS 指针仅输出原始值）
- 配置空间写入
- 单元测试（沿用用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 验证环境

- **dev48**：标准 256 字节可读（sudo），有 PCI-PCI 桥（如 00:1f.0）
- **sg-232e-224**：物理机，桥设备齐全，对照 `lspci -v`
- **myece**：header 区（0x00–0x3F）可读，endpoint 回归
- 字段为 PCI 规范稳定定义，真机校准兜底（既有流程）

## 架构（完全复用既有框架）

- 扩展 `crates/pci/src/header.rs`：新增解码函数（纯函数，输入 snapshot）
- `PciDeviceDetails` 增加新字段；`inspect()` 从已预取的 0x00–0x40
  snapshot 解码（零新增读取）；header 不可读时置 `PciField::Unavailable`
- text/JSON 渲染扩展（沿用现有模式）

## 字段映射

### Type 0/1 通用字段

| 偏移 | 字段 | 解码 |
| ---: | --- | --- |
| 0x0c | Cache Line Size | 原始字节；渲染为 `值×4 bytes`（对齐 lspci `Cache Line Size: 64 bytes`） |
| 0x0d | Latency Timer | 原始字节；渲染 `latency=N` |
| 0x0e | Header Type | bits6–0：0=device、1=bridge、2=cardbus；bit7=multifunction |
| 0x0f | BIST | bit7=capable、bit6=start、bits5–0=completion code |
| 0x30 / 0x38 | Expansion ROM BAR | Type 0 在 0x30、Type 1 在 0x38；bit0=enable、bits31–11=地址 |
| 0x3c | Interrupt Line | 原始字节 |
| 0x3d | Interrupt Pin | 1=INTA、2=INTB、3=INTC、4=INTD、0=无 |
| 0x28 | CardBus CIS Pointer | 仅 Type 2 显示原始 dword |

### Type 1 桥专属（`bridge` 字段，非桥设备为 NotApplicable）

| 偏移 | 字段 | 解码 |
| ---: | --- | --- |
| 0x18 / 0x19 / 0x1a | primary / secondary / subordinate bus | 原始字节 |
| 0x1b | Secondary Latency Timer | 原始字节 |
| 0x1c / 0x1d + 0x30 / 0x32 | IO 窗口 | base=(base&0xf0)<<8 \| upper<<16；limit=(limit&0xf0)<<8 \| upper<<16 \| 0xfff |
| 0x1e | Secondary Status | 逐位（与 Status 位语义相同子集） |
| 0x20 / 0x22 | Memory 窗口 | base<<16；limit<<16 \| 0xfffff |
| 0x24 / 0x26 + 0x28–0x2f | Prefetchable Memory 窗口 | base/limit 低 16 + upper32；bit0 表示 64 位 |
| 0x3e | Bridge Control | 逐位：ParErr(0)、SERR(1)、ISA(2)、VGA(3)、VGA16(4)、MasterAbort(5)、SecBusReset(6)、FastB2B(7)、PrimDiscard(8)、SecDiscard(9)、DiscardTimeout(10)、DiscardSERR(11)、SplitResponse(12) |

渲染对齐 lspci 桥输出形态：

```text
  bus: primary=00 secondary=01 subordinate=02 sec_latency=0
  io behind bridge: 0xc000-0xdfff [size=8K]
  memory behind bridge: 0xc2000000-0xc24fffff [size=5M]
  prefetchable memory behind bridge: ...
  bridge control: ParErr- SERR+ ISA- VGA+ VGA16- MasterAbort- SecBusReset- FastB2B- ...
```

窗口 size 由 limit−base+1 计算并以 K/M 缩写渲染（对齐 lspci）。
窗口未配置（base>limit 或全 0）时渲染 `disabled`。

## 渲染位置

text 输出顺序：status → cache_line_size → latency → header_type →
bist → expansion rom → interrupt → 桥字段块（若桥）→ resources →
capabilities（现状不变）。JSON 对应增加字段对象，桥块为嵌套对象
（非桥时省略）。

## 验证

1. **dev48**：PCI-PCI 桥（00:1f.0）与 `sudo lspci -v` 对照总线号、
   IO/MEM/Prefetch 窗口、Bridge Control
2. **sg-232e-224**：桥设备辅助对照
3. **myece**：endpoint 通用字段回归（header 可读）
4. 字段差异按既有校准流程修正并记录

## 全局约束

- 解码模块零 FFI；解码失败不得使 `inspect()` 失败。
- 不改变 `list` 行为；不引入新依赖。
- 分支策略：从 main 切 `sdd/header-remaining-fields`，完成后走
  finishing-a-development-branch。
