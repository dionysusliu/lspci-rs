# TUI 着色 Progress

更新时间：2026-08-11

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/tui-colors`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-11-tui-colors-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-11-tui-colors.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `7a51f39` | `tui/styled.rs` ANSI→ratatui Text 转换器；详情按 `--color` 着色；`App.detail` 改为 `Text<'static>` |
| Task 2 | `128888e` | 树窗口标记 dim span；`--color never` 全退化为无样式 |
| 辅助 | — | `tools/count-colors.sh`：统计 pty typescript 中的颜色 SGR 序列 |

## 设计要点

- 详情样式来源：`render_device_detail(session, addr, None, self.color)` 产出 ANSI
  文本，经 `styled::text_from_ansi` 转为 ratatui `Text<'static>`。
- 转换器只解析 CSI SGR `\x1b[…m`，码字映射：
  `1;36`→青加粗、`2`→暗淡、`31`→红、`32`→绿、`0`→复位；未知码字忽略。
- 树面板绘制时按 `" -["` 拆分标签，窗口尾缀做成 dim span（`--color never` 时不拆）。
- `--color` 语义：`never`→纯文本无样式；`auto`/`always`→着色（TUI 只在 TTY 运行）。

## 验证记录

**构建门禁（myece 容器）**

- `cargo fmt --check` 通过；构建零警告
- CLI 回归：list 9 台；show `--color always` 输出含 ANSI 转义（≥1）

**pty 冒烟**

- 着色模式：typescript 含颜色 SGR 序列（≥1），exit=0
- `--color never`：颜色序列计数为 0，树渲染完整（`QEMU PCI-PCI` 出现），exit=0

## 发现并修正的问题

- **ratatui 用 indexed color**：`Color::Cyan` 被 crossterm 后端渲染为
  `\x1b[38;5;6;49m`（fg+bg 合并），不是基本色 `\x1b[36m`；颜色计数脚本
  相应适配。
- **awk 正则**：`\[` 在 gawk 动态正则中被当字面量，导致初始版本计数为 0；
  改用 `[[]` 匹配方括号。

## 人工目视走查（myece，待用户执行）

```bash
ssh myece && podman exec -it 95c90e05ab1a bash
cd /workspace && cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- tui
```

清单：地址青色加粗；vendor/device ID 暗淡；`<unavailable>` 红；
能力名绿色；桥 `-[01-01]` 暗淡；`--color never` 全无色。
