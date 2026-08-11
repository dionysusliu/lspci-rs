# TUI 设计：lspci-rs 交互式设备浏览器

日期：2026-08-11
分支：`sdd/tui`（基于 `main`）
前置切片：CLI enhancement（color + tree，已合入 main）

## 目标与范围

为 lspci-rs 增加交互式 TUI：左侧可折叠拓扑树 + 右侧完整解码详情，
支持 fzf 式过滤。只读浏览，无写操作。

**已锁定的设计决策**（brainstorming 结论）：

- 主交互范式：**树导航 + 详情面板**（方案 C，拒绝 Miller 列与纯主从列表）
- 详情呈现：**复用 `show` 的完整解码文本**，可滚动纯文本面板（方案 A）
- MVP 范围：**极简导航 + `/` 过滤**（方案 B，不含手动刷新、无模态命令面板）
- 代码组织：**单二进制 + `tui` 子命令 + 模块目录**（方案 1），不做渲染库抽取

## 依赖

`lspci-rs` crate 新增：

- `ratatui`（immediate-mode widget 库，Rust TUI 事实标准）
- `crossterm`（终端后端，ratatui 的默认 backend）

`pci` crate 不加依赖。具体版本在实现计划阶段于容器内 `cargo add` 锁定。
要求 Rust 1.97 / edition 2024 兼容（当前环境已满足）。

## 架构与组件

| 文件 | 职责 |
| --- | --- |
| `src/tui/mod.rs` | `run_tui()`：TTY 检查、PciSession/snapshot、建树、进入终端、App 状态机与事件循环 |
| `src/tui/tree.rs` | 树模型：节点、可见行扁平列表、展开状态集合、过滤 |
| `src/tui/ui.rs` | ratatui 绘制：分栏、树列表、详情 Paragraph、状态栏/输入框 |

共享重构（已随本分支先行落地，commit `ae1c74b`）：

- `src/tree.rs` 抽出 `pub struct BridgeWindow { pub secondary: u8, pub subordinate: u8 }`
  与 `pub fn collect_bridge_windows(session, snapshot) -> Vec<(PciAddress, BridgeWindow)>`，
  CLI tree 渲染与 TUI 树模型共用；两侧仅保留各自呈现逻辑。
- `run_show` 中"按地址生成详情文本"抽取为
  `render_device_detail(session, snapshot, address, color: ColorMode) -> String`，
  CLI show 与 TUI 共用（CLI 传用户 `--color`，TUI 传 `ColorMode::Never`）。

树节点结构：

- 顶层节点 = 未被任何桥窗口覆盖的总线，标签 `dddd:bb`
- 设备节点标签 `bb:ss.f 厂商 设备名`（复用 list/tree 的命名逻辑，
  含 `Device <id>` ids 回退），桥设备附 `-[sec-sub]` 窗口标记
- 桥设备的子树 = 其 secondary 总线上的设备（递归）

## 按键与过滤

Normal 模式：

| 键 | 动作 |
| --- | --- |
| `j`/`k`、`↓`/`↑` | 移动光标，详情随之更新 |
| `l`/`→`/`Enter`/空格 | 展开节点 |
| `h`/`←` | 折叠当前节点；已折叠则跳到父节点 |
| `PgUp`/`PgDn` | 滚动详情 |
| `/` | 进入过滤模式 |
| `q`/`Esc` | 退出 |

Filter 模式（底部输入框）：

- 逐字符输入，实时收窄：地址文本（`0000:3d:00.0`）或厂商/设备名子串匹配
- 匹配节点保留，祖先链保留并自动展开，其余隐藏
- `Enter` 确认回 Normal；`Esc` 清空过滤回 Normal
- 状态栏显示 `filter: <text> (n/total)`

初始状态：顶层总线展开一级（显示其下设备），桥设备默认折叠，
光标在第一个设备。详情按需同步生成（光标移动即读 config），不预加载。

## 界面布局

- 水平分栏：左 40% 树、右 60% 详情，各带标题块
  （左：当前顶层总线；右：选中设备地址）
- 树：总线节点 `▸/▾ dddd:bb`；设备节点按层级 2 空格缩进；选中行高亮反色
- 详情：`render_device_detail(..., ColorMode::Never)` 纯文本 Paragraph，
  设备切换时滚动归零
- 底部状态栏：按键帮助 + 过滤指示器；过滤模式替换为输入框
- 终端过窄（<60 列）不做降级，ratatui 自动截断

## 启动流程与错误处理

1. stdout 非 TTY 时报错退出：`tui requires an interactive terminal`
2. 打开 PciSession、扫描 snapshot（与 list/show 相同路径）
3. `collect_bridge_windows()` 建树，初始展开顶层总线
4. `enable_raw_mode` + `EnterAlternateScreen`，构造 ratatui Terminal
5. 事件循环：crossterm `Event::read` 阻塞等待按键，无定时刷新（静态快照）

错误处理：

- 终端恢复是硬要求：Drop guard 保证任意路径（含 panic）执行
  `disable_raw_mode` + `LeaveAlternateScreen`
- 单设备 config 读失败不崩：详情中不可读字段渲染为 `<unavailable: …>`
  （复用现有机制）
- 权限不足与 CLI 行为一致：字段显示 unavailable，不额外处理

## 验证方案

无单元测试（项目决策）；构建门禁 + 真机走查：

**构建门禁（myece 容器）**

- `cargo fmt --check`、`cargo build` 零警告
- CLI 回归：`list` 9 台、`tree`/`show` 输出与重构前逐字一致

**TUI 走查**

- myece（9 台、config 仅 0x00–0x3f 可读）：导航、展开/折叠、
  `<unavailable>` 降级渲染、退出后终端恢复
- dev48（sudo，4 级桥链）：深层嵌套展开/折叠、`h` 跳父节点
- sg-232e-224（337 台）：大拓扑滚动；`/` 过滤 `x710` 收窄到 2 台且
  祖先自动展开；`3d:00` 按地址过滤生效

交互手感与渲染观感由用户在真机人工确认。

## 非目标

- 手动刷新/重扫（配置空间视为静态）
- 详情面板着色（后续切片再议，届时考虑渲染库抽取）
- 模态命令面板、多 tab、鼠标操作
- 写操作（enable/disable 设备等）
