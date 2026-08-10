# PCI Config-Space / Capability Work Progress

更新时间：2026-08-10（Task 5/6 完成后）

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/config-space-capability`
- 本阶段仍未合并回 `main`

## 已提交实现

| 阶段 | ECS commit | 状态 |
| --- | --- | --- |
| 配置空间和 capability 领域类型 | `e9444e7` | 已完成并 review 通过 |
| segment reader / FFI 边界 | `4dc4c8e` | 已完成 |
| 标准/扩展 capability walker | `3a41d26` | 已完成并 review 通过 |
| config reader 接入编译修复 | `e3598d0` | 已完成并 review 通过 |
| pci-sys 暴露 config-read FFI 符号 | `bb98fec` | 已完成 |
| PciSession 集成（find_raw_device / read_config / inspect） | `1b218fd` | 已完成 |
| reader 修复：成功判定 + 分块回退 | `4314275` | 已完成 |
| header 可读时保留 capability report | `8ca9919` | 已完成 |
| CLI `show --config` 与 renderer | `eebf6a8` | 已完成 |

## 真机验证中发现并修复的缺陷

1. **libpci 返回值语义**：`pci_read_block` 成功返回 `1`、失败返回 `0`/`-1`，
   原实现按"返回读取字节数"判定，导致所有配置读取被误判为失败。
2. **大块读被后端拒绝**：该环境中 256 字节整块读失败但 64/16 字节读成功，
   reader 增加失败后二分回退，最小粒度 16 字节，保留部分成功 segments。
3. **capability 语义**：header 可读但链不可读时，保留 `Available(report)`
   并携带链状态，而不是整体 `Unavailable`；仅 header 不可读才返回 `Unavailable`。

## 环境事实（ECS 容器 95c90e05ab1a）

- 配置空间仅 `0x000..0x040` 可读（/proc/bus/pci 与 sysfs 均在 0x40 处截断，
  独立 C 程序验证一致）；0x40 之后及扩展配置空间不可读。
- `lspci -vv` 同样报告 `Capabilities: <access denied>`，与本工具输出一致。
- `0000:00:05.0` 的 capability 链指针为 `0x40`，位于不可读区域。

## 真机验证结果（2026-08-10）

- `cargo fmt --all -- --check`：通过。
- `cargo check --workspace --target x86_64-unknown-linux-gnu`：通过。
- `git diff --check`：通过。
- `list --format text` 与系统 `lspci`：9/9 设备，地址、vendor/device/class ID、名称全部对应。
- `show 0000:00:05.0 --config standard`：0x00–0x3f 与 `lspci -xx` 逐字节一致；
  0x40–0xff 报告为 `unavailable <ReadError>`，无零填充。
- `show --config extended --format json`：segments/failures 结构正确。

## 设计文档

- 设计规范：`docs/superpowers/specs/2026-08-09-config-space-and-capability-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-09-config-space-and-capability.md`
- 前置调研：`docs/research/lspci-config-space.md`

## 下一步

1. 执行 whole-branch review，决定是否合并回 `main`。
2. 后续阶段可选：配置空间写入、协议级 capability decoder（MSI/PCIe/AER 等）。
