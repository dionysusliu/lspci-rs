# CLI Enhancement Progress

更新时间：2026-08-11

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/cli-enhancement`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-11-cli-enhancement-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-11-cli-enhancement.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `4d0d39b` | color 模块（ColorMode auto/always/never + Palette）与全局 `--color` |
| Task 2 | `53add0b` | list/show 文本渲染集中着色（地址青色、ID 暗淡、不可用红色、能力名绿色） |
| Task 3 | `05b4413` | tree 子命令：按桥窗口渲染设备拓扑 |
| 校准 | `aac4f48` | tree 改为逐桥层级渲染，消除桥重复出现 |
| 修复 | `bbd9abb` | 恢复不可用字段标签（perl 替换残留 `$1`）+ 简化 tree 递归 |
| 校准 | `786f187` | 顶层桥子设备加续行符缩进，消除与顶层平级的视觉歧义 |
| 打磨 | `0f896a7` | ids 库无条目时回退 lspci 风格 `Device 1bb8` / `Vendor 8086` |
| 打磨 | `ed296df` | tree 条目显示厂商名（`Intel(R) Device 1bb8`） |

## 设计要点

- `ColorMode` 为 clap ValueEnum，`--color` 全局参数，默认 `auto`（stdout 为
  TTY 时着色）。JSON 输出永不着色。
- 着色集中在 `output.rs` 渲染层，解码器保持纯函数、无颜色概念。
- tree 通过读取 class=0x0604 桥的 secondary/subordinate 窗口（config 0x19）
  建立拓扑：未被任何窗口覆盖的总线为顶层，桥设备显示 `-[sec-sub]` 标签并
  递归进入 secondary 总线。

## dev48 验证记录

- `tree --color never` 与 `sudo lspci -t` 拓扑完全一致：
  bus 00 下 11 个设备 + 桥链 `1f.0-[01-04] → 01:1f.0-[02-04] →
  02:1f.0-[03-04] → 03:1f.0-[04]`，嵌套关系与标签逐一对应。
- `--color always` 输出含 ANSI 转义；`--color never` 无转义；
  管道（auto 非 TTY）无转义；`--format json --color always` 无转义。

## sg-232e-224 真机验证（Ice Lake，337 台设备）

- tree 设备集合与 `list` 完全一致（337 条，diff 为空），且与
  `lspci -D` 设备计数一致——无遗漏、无重复。
- 拓扑与 `lspci -t` 对应：多个顶层总线（00/16/29/3c/4f/62/…），
  X710 双口卡正确嵌套于 `3c:01.0 -[3d-3e]` 之下；
  4 块 NVMe 分别挂于 `4f:01.0/03.0/05.0/07.0` 单设备桥下。
- 发现并修复：顶层桥的子设备原与顶层同级缩进（视觉歧义），
  现加 `|` 续行符嵌套（commit `786f187`）。
- 行数差异（rs 337 行 vs lspci -t 322 行）来自渲染风格：
  lspci 将桥链横向合并为一行，本工具逐设备一行，属预期差异。
- 大量 `<unknown>` 设备名源于 Ice Lake-SP uncore 设备不在 pci.ids
  （机器自带 lspci 3.8.0 同样显示 `Device 1bb8`）；已回退为
  lspci 风格 `Device <id>`（commit `0f896a7`），`<unknown>` 计数归零。

## 校准中发现并修正的问题

- 桥重复出现：初版全窗口遍历导致桥既作为子节点又作为顶层出现；改为逐桥
  渲染 secondary 总线 + 顶层仅渲染未覆盖总线。
- host bridge（class 0x0600）误当作 PCI 桥读出垃圾窗口：窗口收集限定
  class=0x0604。
- 着色重构的 perl 替换把 11 处不可用字段标签替换成了字面量 `$1`
  （clippy `empty format string` 暴露），已全部恢复为正确标签。

## myece 回归

- `cargo fmt --check` 通过；`cargo build` 零警告。
- `list` 设备数 9 不变；`show 0000:00:02.0` 字段完整、标签正确；
  `tree` 渲染 8 个 bus 00 设备 + 空桥 `00:1f.0 -[01-01]`。
- 备注：容器未预装 clippy，本次临时安装；剩余 clippy 提示均为先前切片
  遗留（pci crate + render_window_size），不在本次范围。
