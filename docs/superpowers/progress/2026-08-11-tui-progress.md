# TUI Progress

更新时间：2026-08-11

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/tui`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-11-tui-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-11-tui.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| 前置 | `ae1c74b` | 抽取 `collect_bridge_windows()` 共享拓扑逻辑 |
| Task 1 | `e855a0f` | 引入 ratatui/crossterm；抽取 `render_device_detail()` |
| Task 2 | `228f206` | `tui` 子命令骨架：TTY 检查、TerminalGuard、事件循环 |
| Task 3 | `568820c` | TreeModel（可见行/展开/过滤）+ 分栏 UI + 详情加载 |
| Task 4 | `8f6b2eb` | PgUp/PgDn 详情滚动 + 状态栏设备计数 |
| Task 5 | `d14cae0` | `/` 过滤模式（实时收窄、祖先自动展开） |
| 修复 | `fb3b947` | 过滤状态栏只统计真实匹配设备（祖先桥不计入） |
| 杂项 | `cee0381` `d97ac53` `c0a09c8` | spec、plan、rustfmt import 排序 |

## 验证记录

**myece（构建门禁 + 自动冒烟，pty 经 `script -qec` + `stty rows/cols`）**

- `cargo fmt --check` 通过；构建零警告
- CLI 回归：list 9 台、tree 9 行、show 与重构前逐字一致
- TUI：q 正常退出、导航/展开渲染 `440FX`、详情 `PCI device`、
  PgDn 滚动、`/virtio` 收窄 4/9、Esc 清空恢复 9/9

**dev48（sudo，自动 + 人工待走查）**

- 自动：4 级桥链 00:1f.0→01:1f.0→02:1f.0→03:1f.0 用按键序列逐级展开，
  `03:1f.0` 出现，exit=0
- 人工待查：选中设备详情与 `show` 一致、q 退出后终端恢复

**sg-232e-224（337 台设备，自动 + 人工待走查）**

- 自动：`/x710` 收窄为 `devices: 2/337`，祖先（3c 总线、3c:01.0 桥）
  自动展开，exit=0
- 人工待查：大拓扑滚动流畅、`/3d:00` 地址过滤、Esc 清空

## 校准中发现并修正的问题

- **ratatui API 版本差异**：当前版本 `draw` 闭包与 `render_widget`
  要求 `&mut Frame`（计划写的是 `&Frame`），已适配。
- **pty 冒烟窗口尺寸**：`script` 分配的 pty 默认 0x0，ratatui 不渲染；
  冒烟脚本内先 `stty rows 30 cols 100`。
- **Enter 键字节**：pty 冒烟里 `\n`(LF) 不会被 crossterm 解析为 Enter，
  真实键盘发送 CR；冒烟改用 `\r`，应用代码无需改。
- **Esc 歧义**：`printf` 连发 `\033q` 时 crossterm 等待转义序列后续字节
  而阻塞；真实按键有间隔（crossterm 内部超时判定），冒烟加 sleep 分隔。
- **过滤计数 bug**：状态栏 `devices: n/total` 原按可见行计数，
  祖先桥设备被计入（x710 显示 3/337）；改为只统计标签匹配过滤词的设备。
