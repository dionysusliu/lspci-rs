# TUI 着色实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TUI 详情面板按 CLI 语义着色（ANSI→ratatui 转换），树面板窗口标记暗淡色，支持 `--color never` 关闭。

**Architecture:** 详情改用 `render_device_detail(..., <color>)` 产出 ANSI 文本，经新模块 `tui/styled.rs` 转为 ratatui `Text<'static>`；树面板绘制时把标签尾缀 ` -[sec-sub]` 拆成 dim span。不改 output.rs。

**Tech Stack:** ratatui（已在依赖中），无新增依赖

## Global Constraints

- 远程开发：`ssh myece` → `podman exec 95c90e05ab1a bash -lc 'cd /workspace && ...'`；文件经本地 sftp 推到 myece:/tmp 再 `podman cp` 进容器（podman cp 后记得 `chmod +x` 脚本）
- 构建命令：`cargo build -p lspci-rs --target x86_64-unknown-linux-gnu`；门禁含 `cargo fmt --all --check`、零警告
- 无单元测试（项目决策）：pty 冒烟用 `tools/tui-smoke.sh`（注意：按键需 `sleep 1` 延迟送入，Enter 用 `\r`）
- 分支：`sdd/tui-colors`（已建，spec `61267cb`）
- ANSI 码字全集（Palette 生成）：`1;36` 青加粗、`2` 暗淡、`31` 红、`32` 绿、`0` 复位；未知码字忽略
- ratatui 当前版本要求 `&mut Frame`（本分支代码已适配）

---

### Task 1: styled.rs 转换器与详情着色接线

**Files:**
- Create: `crates/lspci-rs/src/tui/styled.rs`
- Modify: `crates/lspci-rs/src/tui/mod.rs`
- Modify: `crates/lspci-rs/src/main.rs`

**Interfaces:**
- Produces: `pub fn text_from_ansi(input: &str) -> ratatui::text::Text<'static>`（Task 2 的树 dim span 不依赖它，但详情渲染依赖）
- Consumes: `crate::render_device_detail(session, address, None, color)`、`crate::color::ColorMode`

- [ ] **Step 1: tui/styled.rs（全文）**

```rust
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Convert ANSI-colored text into an owned ratatui Text.
/// Only SGR sequences are interpreted; unknown codes are ignored.
pub fn text_from_ansi(input: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buffer = String::new();
    let mut style = Style::default();

    let mut rest = input;
    loop {
        let next = rest.find(|c| c == '\x1b' || c == '\n');
        let Some(pos) = next else {
            buffer.push_str(rest);
            break;
        };
        buffer.push_str(&rest[..pos]);
        if rest.as_bytes()[pos] == b'\n' {
            push_span(&mut spans, &mut buffer, style);
            lines.push(Line::from(std::mem::take(&mut spans)));
            rest = &rest[pos + 1..];
            continue;
        }
        // ESC: try to parse CSI SGR
        let after = &rest[pos + 1..];
        if let Some(stripped) = after.strip_prefix('[') {
            if let Some(end) = stripped.find(|c: char| ('@'..='~').contains(&c)) {
                if stripped.as_bytes()[end] == b'm' {
                    push_span(&mut spans, &mut buffer, style);
                    style = apply_sgr(style, &stripped[..end]);
                }
                rest = &stripped[end + 1..];
                continue;
            }
        }
        // malformed sequence: emit the ESC as plain text
        buffer.push('\x1b');
        rest = after;
    }

    push_span(&mut spans, &mut buffer, style);
    lines.push(Line::from(spans));
    Text::from(lines)
}

fn push_span(spans: &mut Vec<Span<'static>>, buffer: &mut String, style: Style) {
    if !buffer.is_empty() {
        spans.push(Span::styled(std::mem::take(buffer), style));
    }
}

fn apply_sgr(style: Style, params: &str) -> Style {
    match params {
        "0" | "" => Style::default(),
        "1;36" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "2" => style.add_modifier(Modifier::DIM),
        "31" => Style::default().fg(Color::Red),
        "32" => Style::default().fg(Color::Green),
        _ => style,
    }
}
```

- [ ] **Step 2: mod.rs 接线**

在 `mod tree;` 旁加 `mod styled;`。`App` 结构体加字段 `pub color: ColorMode,`（放在 `filter_input` 后）。`App::new` 签名与初始化改为：

```rust
    fn new(session: PciSession, model: tree::TreeModel, color: ColorMode) -> App {
```

构造字段处加 `color,`（其余字段不变）。

`load_detail` 中生成详情的部分改为：

```rust
        let text =
            crate::render_device_detail(&mut self.session, address, None, self.color)
                .unwrap_or_else(|error| format!("failed to load details: {error}"));
        self.detail = styled::text_from_ansi(&text);
```

`App.detail` 类型改为 `ratatui::text::Text<'static>`（初始值 `Text::default()`，`use ratatui::text::Text;`）。

PageDown 的行数上限由 `self.detail.lines().count()` 改为 `self.detail.lines.len()`：

```rust
            KeyCode::PageDown => {
                let lines = self.detail.lines.len() as u16;
                self.detail_scroll = (self.detail_scroll + 10).min(lines.saturating_sub(1));
            }
```

`run_tui` 签名改为 `pub fn run_tui(color: ColorMode) -> Result<(), Box<dyn std::error::Error>>`，
其中 `let mut app = App::new(session, model, color);`。

- [ ] **Step 3: main.rs 传参**

```rust
        Command::Tui => {
            if let Err(error) = tui::run_tui(color) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
```

- [ ] **Step 4: 构建门禁 + 颜色冒烟**

```bash
cargo fmt --all && cargo fmt --all --check && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu 2>&1 | grep -cE 'warning|error'
# 期望 0
( sleep 1; printf q ) | timeout 20 script -qec "stty rows 30 cols 100; exec target/x86_64-unknown-linux-gnu/debug/lspci-rs tui" /tmp/tui.ts >/dev/null; echo exit=$?
# 统计青/红/绿 SGR 序列数（选中行的反色 7m 不计入）
awk 'BEGIN{esc=sprintf("%c",27)} { n += gsub(esc "\\[[0-9;]*(31|32|36)[0-9;]*m", "&") } END { print n+0 }' /tmp/tui.ts
# 期望 >= 1
./tools/strip-ansi.sh /tmp/tui.ts | grep -c 'PCI device'
# 期望 >= 1（详情仍完整）
```

- [ ] **Step 5: CLI 回归**

```bash
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --color never | wc -l   # 9
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- tree --color never | wc -l   # 9
```

- [ ] **Step 6: Commit**

```bash
git add crates/lspci-rs/src/tui/styled.rs crates/lspci-rs/src/tui/mod.rs crates/lspci-rs/src/main.rs
git commit -m "tui: styled detail pane via ANSI-to-ratatui conversion"
```

---

### Task 2: 树窗口标记暗淡色与 --color never

**Files:**
- Modify: `crates/lspci-rs/src/tui/ui.rs`

**Interfaces:**
- Consumes: `App.color: ColorMode`、`App.detail: Text<'static>`（Task 1）

- [ ] **Step 1: ui.rs 详情面板适配 Text**

`draw_detail` 中：

```rust
    let paragraph = Paragraph::new(app.detail.clone())
        .scroll((app.detail_scroll, 0))
        .block(Block::bordered().title(title));
```

`Text` 的 clone 是廉价的（Cow 共享），无需其他改动。

- [ ] **Step 2: ui.rs 树窗口标记 dim span**

`draw_tree` 里构造行内容处替换（原 `let text = format!(...); items.push(ListItem::new(Line::from(text))...)`）：

```rust
        let prefix = format!("{}{}", "  ".repeat(row.depth), marker);
        let colored = !matches!(app.color, ColorMode::Never);
        let line = match (colored, row.label.split_once(" -[")) {
            (true, Some((head, tail))) => Line::from(vec![
                Span::raw(format!("{prefix}{head}")),
                Span::styled(
                    format!(" -[{tail}"),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]),
            _ => Line::from(format!("{prefix}{}", row.label)),
        };
        items.push(ListItem::new(line).style(style));
```

imports 增加 `use crate::color::ColorMode;`（`Style`/`Modifier` 已在）。

- [ ] **Step 3: 构建门禁 + 双模式冒烟**

```bash
cargo fmt --all && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu 2>&1 | grep -cE 'warning|error'   # 0
# 着色模式：有颜色序列
( sleep 1; printf q ) | timeout 20 script -qec "stty rows 30 cols 100; exec target/x86_64-unknown-linux-gnu/debug/lspci-rs tui" /tmp/tui.ts >/dev/null
awk 'BEGIN{esc=sprintf("%c",27)} { n += gsub(esc "\\[[0-9;]*(31|32|36)[0-9;]*m", "&") } END { print n+0 }' /tmp/tui.ts   # >= 1
# never 模式：无颜色序列，功能不回归
( sleep 1; printf q ) | timeout 20 script -qec "stty rows 30 cols 100; exec target/x86_64-unknown-linux-gnu/debug/lspci-rs tui --color never" /tmp/tui.ts >/dev/null
awk 'BEGIN{esc=sprintf("%c",27)} { n += gsub(esc "\\[[0-9;]*(31|32|36)[0-9;]*m", "&") } END { print n+0 }' /tmp/tui.ts   # 0
./tools/strip-ansi.sh /tmp/tui.ts | grep -c 'QEMU PCI-PCI'   # >= 1
```

- [ ] **Step 4: Commit**

```bash
git add crates/lspci-rs/src/tui/ui.rs
git commit -m "tui: dim bridge window markers and honor --color never"
```

---

### Task 3: 全量验证、进度文档与收尾

**Files:**
- Create: `docs/superpowers/progress/2026-08-11-tui-colors-progress.md`

**Interfaces:**
- Consumes: Task 1–2 全部产物

- [ ] **Step 1: myece 全量门禁**

```bash
cargo fmt --all --check
cargo build -p lspci-rs --target x86_64-unknown-linux-gnu 2>&1 | grep -cE 'warning|error'   # 0
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --color never | wc -l    # 9
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:02.0 --color always | cat -v | grep -c '\^\[\['   # >= 1（CLI 着色仍正常）
```

- [ ] **Step 2: 人工目视走查（myece，用户执行）**

```bash
ssh myece && podman exec -it 95c90e05ab1a bash
cd /workspace && cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- tui
```

清单：地址青色加粗；vendor/device ID 暗淡；`<unavailable>` 红（myece config 仅 0x00–0x3f 可读，字段大量不可用，正好覆盖）；能力名绿色；桥 `-[01-01]` 暗淡；`--color never` 全无色。

- [ ] **Step 3: 写进度文档并提交**

按 `docs/superpowers/progress/` 既有格式：工作区、commit 表、验证记录、发现并修正的问题。

```bash
git add docs/superpowers/progress/2026-08-11-tui-colors-progress.md
git commit -m "docs: record TUI colors progress"
```

- [ ] **Step 4: 收尾**

REQUIRED SUB-SKILL: superpowers:finishing-a-development-branch。
测试套件 = Step 1 门禁；base branch = main。
