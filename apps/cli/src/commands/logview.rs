//! Git 日志 TUI 浏览器（ratatui）。
//!
//! 对应 git-mate `internal/logview`：提交列表 + diff 详情 + 关键词搜索。
//! 按键：↑/↓ 或 j/k 移动 | Enter/d 切换 diff | / 搜索 | Esc 退出搜索/diff | q 退出

use byte_git_core::logview as core_lv;
use clap::Args;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use std::io::stdout;
use std::path::Path;

#[derive(Args, Debug)]
pub struct LogviewCmd {
    /// 初始拉取条数（默认 200，g 加载更多）
    #[arg(long, default_value = "200")]
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    List,
    Diff,
    Search,
}

struct App {
    repo: std::path::PathBuf,
    all: Vec<core_lv::CommitEntry>,
    filtered: Vec<core_lv::CommitEntry>,
    cursor: usize,
    offset: usize,
    mode: Mode,
    search: String,
    diff: String,
    diff_scroll: u16,
    diff_hash: String,
    limit: usize,
}

impl App {
    fn apply_filter(&mut self) {
        if self.search.is_empty() {
            self.filtered = self.all.clone();
        } else {
            let kw = self.search.to_lowercase();
            self.filtered = self
                .all
                .iter()
                .filter(|e| {
                    e.hash.contains(&kw)
                        || e.author.to_lowercase().contains(&kw)
                        || e.subject.to_lowercase().contains(&kw)
                })
                .cloned()
                .collect();
        }
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    fn load_more(&mut self) -> anyhow::Result<()> {
        self.limit += 200;
        self.all = core_lv::fetch_commits(&self.repo, self.limit, "")?;
        self.apply_filter();
        Ok(())
    }
}

pub fn run(repo: &Path, c: LogviewCmd) -> anyhow::Result<()> {
    let all = core_lv::fetch_commits(repo, c.limit, "")?;
    if all.is_empty() {
        anyhow::bail!("没有提交记录");
    }
    let mut app = App {
        repo: repo.to_path_buf(),
        filtered: all.clone(),
        all,
        cursor: 0,
        offset: 0,
        mode: Mode::List,
        search: String::new(),
        diff: String::new(),
        diff_scroll: 0,
        diff_hash: String::new(),
        limit: c.limit,
    };

    enable_raw_mode()?;
    let _ = stdout().execute(EnterAlternateScreen);
    let backend = ratatui::init();
    let result = event_loop(&mut app, backend);
    disable_raw_mode()?;
    let _ = stdout().execute(LeaveAlternateScreen);
    result
}

fn event_loop<B: ratatui::backend::Backend>(app: &mut App, mut terminal: ratatui::Terminal<B>) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;
        // 列表视口跟随光标（预留标题/状态栏各 1 行）
        if app.mode == Mode::List {
            if let Ok(size) = terminal.size() {
                let visible = size.height.saturating_sub(2) as usize;
                if app.cursor < app.offset {
                    app.offset = app.cursor;
                } else if visible > 0 && app.cursor >= app.offset + visible {
                    app.offset = app.cursor + 1 - visible;
                }
            }
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match (&app.mode, key.code) {
                // 搜索输入
                (Mode::Search, KeyCode::Char(ch)) => {
                    app.search.push(ch);
                    app.apply_filter();
                }
                (Mode::Search, KeyCode::Backspace) => {
                    app.search.pop();
                    app.apply_filter();
                }
                (Mode::Search, KeyCode::Esc) => {
                    app.search.clear();
                    app.apply_filter();
                    app.mode = Mode::List;
                }
                (Mode::Search, KeyCode::Enter) => app.mode = Mode::List,

                (_, KeyCode::Char('q')) | (_, KeyCode::Esc) => return Ok(()),
                (Mode::List, KeyCode::Char('/')) => app.mode = Mode::Search,
                (Mode::List, KeyCode::Up) | (Mode::List, KeyCode::Char('k')) => {
                    app.cursor = app.cursor.saturating_sub(1);
                }
                (Mode::List, KeyCode::Down) | (Mode::List, KeyCode::Char('j')) => {
                    if app.cursor + 1 < app.filtered.len() {
                        app.cursor += 1;
                    }
                }
                (Mode::List, KeyCode::Char('g')) => app.load_more()?,
                (Mode::List, KeyCode::Enter) | (Mode::List, KeyCode::Char('d')) => {
                    if let Some(e) = app.filtered.get(app.cursor) {
                        app.diff = core_lv::fetch_diff(&app.repo, &e.hash)?;
                        app.diff_hash = e.hash.clone();
                        app.diff_scroll = 0;
                        app.mode = Mode::Diff;
                    }
                }
                (Mode::Diff, KeyCode::Up) | (Mode::Diff, KeyCode::Char('k')) => {
                    app.diff_scroll = app.diff_scroll.saturating_sub(1);
                }
                (Mode::Diff, KeyCode::Down) | (Mode::Diff, KeyCode::Char('j')) => {
                    app.diff_scroll += 1;
                }
                (Mode::Diff, KeyCode::Enter) | (Mode::Diff, KeyCode::Char('d')) | (Mode::Diff, KeyCode::Char(' ')) => {
                    app.mode = Mode::List;
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    use ratatui::layout::Constraint::{Length, Min};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{List, ListItem, Paragraph};

    let mode_title = match app.mode {
        Mode::Search => "byte-git log（输入关键词过滤，Esc 返回）",
        Mode::Diff => &format!("diff {}（j/k 滚动，q 退出）", &app.diff_hash[..7.min(app.diff_hash.len())]),
        Mode::List => "byte-git log（↑↓/jk 选择 · Enter 看diff · / 搜索 · g 更多 · q 退出）",
    };

    let chunks = ratatui::layout::Layout::vertical([Length(1), Min(0)]).split(f.area());
    f.render_widget(Paragraph::new(mode_title).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)), chunks[0]);

    if app.mode == Mode::Diff {
        let lines: Vec<Line> = app.diff.lines().map(|l| {
            let style = if l.starts_with('+') {
                Style::default().fg(Color::Green)
            } else if l.starts_with('-') {
                Style::default().fg(Color::Red)
            } else if l.starts_with("@@") {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            Line::styled(l.to_string(), style)
        }).collect();
        let visible_height = chunks[1].height as usize;
        let start = app.diff_scroll as usize;
        let end = (start + visible_height).min(lines.len());
        let window: Vec<Line> = lines[start.min(lines.len())..end].to_vec();
        let para = Paragraph::new(window);
        f.render_widget(para, chunks[1]);
        return;
    }

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let selected = i == app.cursor;
            let ty_span = if e.ty.is_empty() {
                Span::raw("      ")
            } else {
                let color = match e.ty.as_str() {
                    "feat" => Color::Green,
                    "fix" => Color::Yellow,
                    "refactor" => Color::Magenta,
                    "perf" => Color::LightRed,
                    "docs" => Color::LightBlue,
                    _ => Color::DarkGray,
                };
                let label = if e.scope.is_empty() {
                    format!("{:<6}", e.ty)
                } else {
                    format!("{:<6}", format!("{}({})", e.ty, e.scope))
                };
                Span::styled(label, Style::default().fg(color))
            };
            let line = Line::from(vec![
                Span::styled(format!("{} ", &e.hash[..7.min(e.hash.len())]), Style::default().fg(Color::DarkGray)),
                ty_span,
                Span::raw(e.subject.clone()),
            ]);
            let style = if selected {
                Style::default().bg(Color::Rgb(54, 74, 130)).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items.into_iter().skip(app.offset).collect::<Vec<_>>());
    f.render_widget(list, chunks[1]);

    // 状态栏（覆盖列表底部一行）
    if let Some(e) = app.filtered.get(app.cursor) {
        let status = format!(" {} {} <{}> {}", &e.hash[..7.min(e.hash.len())], e.author, e.date, e.subject);
        let area = ratatui::layout::Rect {
            x: chunks[1].x,
            y: chunks[1].bottom().saturating_sub(1),
            width: chunks[1].width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(status).style(Style::default().fg(Color::Black).bg(Color::Cyan)),
            area,
        );
    }
}
