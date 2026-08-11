# TUI 着色设计：详情面板与树面板

日期：2026-08-11
分支：`sdd/tui-colors`（基于 `main`）
前置切片：TUI（树+详情+过滤，已合入 main）

## 目标与范围

为 TUI 增加语义着色：

1. **详情面板**：label/value 区分、不可用字段红色、能力名绿色等，
   与 CLI `show --color always` 逐字一致
2. **树面板**：桥设备的 `-[sec-sub]` 窗口标记暗淡色（轻量）

## 已锁定的设计决策

- 范围：详情 + 树（不含状态栏/标题着色）
- 详情样式来源：**ANSI 解析转换**——复用 `render_device_detail(..., Always)`
  的 ANSI 输出，TUI 侧转换为 ratatui spans；不重构 output.rs

## 架构与改动点

只动 TUI 侧：

| 改动 | 内容 |
| --- | --- |
| 新增 `src/tui/styled.rs` | `pub fn text_from_ansi(input: &str) -> Text<'static>`：ANSI→ratatui Text 转换器 |
| `src/tui/mod.rs` | `run_tui(color: ColorMode)` 接收全局 `--color`；`App` 持有颜色模式；`load_detail` 按模式调用 `render_device_detail`（Never 走纯文本），`App.detail` 类型由 `String` 改为 `ratatui::text::Text<'static>` |
| `src/tui/ui.rs` | 详情 Paragraph 渲染 `Text`；树标签尾缀 ` -[sec-sub]` 拆为独立 dim span（`--color never` 时不拆、无样式） |
| `src/main.rs` | `Command::Tui` 分支把 `cli.color` 传给 `run_tui` |

滚动逻辑不变（`Paragraph::scroll` 对 `Text` 生效）。

## ANSI 转换器规格（styled.rs）

- 只解析 CSI SGR 序列 `\x1b[…m`，其余字符原样累积；`\n` 切行
- 码字映射（Palette 生成的全集）：
  - `1;36` → 青色加粗（地址）
  - `2` → 暗淡（ID、窗口、dump 偏移）
  - `31` → 红（不可用字段）
  - `32` → 绿（能力名）
  - `0`（空）→ 复位
- **未知码字一律忽略**（保留当前样式继续解析，前向兼容不崩）
- 产出拥有所有权的 `Text<'static>`；转换在 `load_detail` 时一次完成，
  不在每帧绘制时重复
- 无 ANSI 的输入（`--color never` 路径）原样转纯文本 Text

## `--color` 语义

- `never`：详情纯文本、树无样式（窗口标记不拆分）
- `auto` / `always`：着色。TUI 只在 TTY 运行，`auto` 等价 `always`

## 树面板样式

- 标签主体默认色；尾缀 ` -[sec-sub]` 拆为独立 span，暗淡色
  （绘制时按 `" -["` split_once，不改 TreeModel 结构）
- 选中行保持反色；总线节点与展开符号不着色

## 错误处理

- `render_device_detail` 失败路径不变：错误信息转为纯文本 Text
- 转换器对畸形输入（不完整转义序列、非 UTF-8 不会出现——输入是
  String）永不 panic：截断的 `\x1b[...` 尾部按普通文本输出

## 验证方案

无单元测试（项目决策）；构建门禁 + pty 冒烟 + 人工目视：

**构建门禁（myece 容器）**

- `cargo fmt --check`、构建零警告
- CLI 回归：list 9 台、tree/show 输出不变（本切片不动渲染层）

**pty 冒烟**

- 着色模式：typescript 中能抓到 ratatui 发出的颜色序列（青/绿/红之一）
- `--color never`：抓不到颜色序列

**人工目视（myece）**

- 打开 TUI 确认：地址青色加粗、ID 暗淡、`<unavailable>` 红色
  （myece config 仅 0x00–0x3f 可读，天然覆盖）、能力名绿色、
  树窗口标记暗淡

## 非目标

- 状态栏/标题/边框着色
- 256 色/真彩扩展、主题切换
- output.rs 结构化渲染重构（后续切片再议）
