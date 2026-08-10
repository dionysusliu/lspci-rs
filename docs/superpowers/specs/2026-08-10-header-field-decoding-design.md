# Header Field Semantic Decoding Design

日期：2026-08-10
状态：已与用户确认（brainstorming 定稿）

## 目标

对 PCI 配置空间标准 Header（0x00–0x3F）中的 Command 寄存器、Status
寄存器做位级语义解读，并为 BAR（Base Address Register）补充类型信息
（IO/MEM、32/64 位、prefetchable），输出对齐 `lspci -v` 的
`Control:` / `Status:` / `Region N:` 形态。

## 明确不做

- Header Type（设备/桥）、BIST、Expansion ROM、Interrupt Pin 等其余
  header 字段的解读（用户选择范围 A，这些留下一切片）
- capability 相关改动（22 个 decoder 已完成）
- 配置空间写入
- 单元测试（沿用用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 验证环境

- **myece**：配置空间 0x00–0x3F 可读——恰好覆盖 header 区，可验证
- **dev48**：标准 256 字节可读（sudo），对照 `lspci -v`
- **sg-232e-224**：全量可读，辅助对照
- Command/Status 位表为 PCI 规范稳定定义，预期无需真机校准，
  但仍逐项对照 lspci 输出

## 架构（完全复用既有框架）

- 新增纯解码模块 `crates/pci/src/header.rs`：
  `decode_command(word: u16) -> CommandRegister`、
  `decode_status(word: u16) -> StatusRegister`、
  `decode_bar_type(dword: u32) -> Option<PciBarType>`（纯函数，无 FFI）
- `PciDeviceDetails` 增加 `command: PciField<CommandRegister>`、
  `status: PciField<StatusRegister>`；header 不可读时为
  `PciField::Unavailable`
- `PciResource` 增加 `bar_type: Option<PciBarType>`；config 不可读时
  为 `None`
- **数据来源零新增读取**：`inspect()` 的链发现已预取 0x000..0x040，
  Command(0x04)/Status(0x06)/BAR(0x10+4i) 字节都在 snapshot 缓存里，
  直接从 snapshot 解码；header 不可读（`header_readable == false`）时
  三个字段均置 Unavailable/None

## 领域模型

### CommandRegister（0x04 word，PCI 规范位定义）

| 位 | 字段 | lspci 名 |
| ---: | --- | --- |
| 0 | io_space | I/O |
| 1 | memory_space | Mem |
| 2 | bus_master | BusMaster |
| 3 | special_cycle | SpecCycle |
| 4 | mem_write_invalidate | MemWINV |
| 5 | vga_palette_snoop | VGASnoop |
| 6 | parity_error_response | ParErr |
| 7 | stepping | Stepping |
| 8 | serr_enable | SERR |
| 9 | fast_back_to_back | FastB2B |
| 10 | interrupt_disable | DisINTx |

### StatusRegister（0x06 word）

| 位 | 字段 | lspci 名 |
| ---: | --- | --- |
| 3 | interrupt_status | INTx |
| 4 | capabilities_list | Cap |
| 5 | capable_66mhz | 66MHz |
| 6 | udf | UDF |
| 7 | capable_fast_back_to_back | FastB2B |
| 8 | master_parity_error | ParErr |
| 9–10 | devsel_timing（0=fast 1=medium 2=slow） | DEVSEL |
| 11 | signaled_target_abort | >TAbort |
| 12 | received_target_abort | <TAbort |
| 13 | received_master_abort | <MAbort |
| 14 | signaled_system_error | >SERR |
| 15 | detected_parity_error | <PERR |

### PciBarType（从 0x10+4*index 的原始 BAR dword 解码）

- bit0：1 = IO，0 = Memory
- Memory 时：bits1–2 = 00 32 位 / 01 1M（legacy）/ 10 64 位；
  bit3 = prefetchable
- IO 时：is_64_bit/prefetchable 均为 false
- libpci 的 resources 已按 lspci 口径跳过 64 位 BAR 占用的上槽，
  BAR 寄存器偏移与 resource index 一一对应（0x10 + 4*index）

## 渲染

### Text

```text
  control: I/O+ Mem+ BusMaster+ SpecCycle- MemWINV- VGASnoop- ParErr- Stepping- SERR- FastB2B- DisINTx+
  status: Cap+ 66MHz- UDF- FastB2B- ParErr- DEVSEL=fast >TAbort- <TAbort- <MAbort- >SERR- <PERR- INTx-
  resources:
    BAR0 start=0xc4004000 size=0x1000 type=memory-64-prefetch flags=0x14220c
```

- control/status 放在 capabilities 段之前、resources 之前
- BAR type 形态：`io` / `memory-32` / `memory-64` / `memory-32-prefetch` /
  `memory-64-prefetch`（1M legacy 类型按 memory-32 处理）

### JSON

- `control` / `status`：bool 字段对象，`devsel` 为字符串
  （`"fast"` / `"medium"` / `"slow"`）；Unavailable 时沿用
  `PciField` 的 state/reason 形态
- resources 条目增加 `bar_type`：字符串（同上形态）或 null

## 验证

1. **myece**：header 可读，直接验证 control/status/BAR type 输出
2. **dev48**：与 `sudo lspci -v` 逐字段对照（选 virtio 网卡与 PCI 桥
   各一台，覆盖 endpoint 与桥）
3. **sg-232e-224**：辅助对照
4. handoff 进度文档记录对照结果

## 全局约束

- 解码模块零 FFI；解码失败不得使 `inspect()` 失败。
- 不改变 `list` 行为；不引入新依赖。
- 分支策略：从 main 切 `sdd/header-field-decoding`，完成后走
  finishing-a-development-branch。
