pub mod animation;
pub mod config;

use pomotui_protocol::{SessionKind, Snapshot};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    VermilionPaperLight,
    VermilionPaperDark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    English,
    SimplifiedChinese,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Dashboard,
    Today,
    History,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Overlay {
    None,
    Palette,
    Settings,
    Help,
    CreateTask,
    RenameTask,
    ConfirmDelete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputKey {
    Char(char),
    Escape,
    Enter,
    Backspace,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Command(pomotui_protocol::Command),
    SetLanguage(Language),
    Quit,
}

pub struct App {
    pub snapshot: Option<Snapshot>,
    pub theme: Theme,
    pub language: Language,
    pub view: View,
    pub overlay: Overlay,
    pub selected_task: usize,
    pub palette_index: usize,
    pub input: String,
    pub narrow: bool,
    pub warning: Option<String>,
    pub message: Option<String>,
    completion: Option<CompletionPlayback>,
}

struct CompletionPlayback {
    animation: animation::Animation,
    elapsed_ms: u64,
}

impl App {
    #[must_use]
    pub const fn new(snapshot: Option<Snapshot>, theme: Theme) -> Self {
        Self {
            snapshot,
            theme,
            language: Language::English,
            view: View::Dashboard,
            overlay: Overlay::None,
            selected_task: 0,
            palette_index: 0,
            input: String::new(),
            narrow: false,
            warning: None,
            message: None,
            completion: None,
        }
    }

    pub fn key(&mut self, key: char) -> Option<Action> {
        let key = match key {
            '\u{1b}' => InputKey::Escape,
            '\n' | '\r' => InputKey::Enter,
            '\u{8}' | '\u{7f}' => InputKey::Backspace,
            '↑' => InputKey::Up,
            '↓' => InputKey::Down,
            value => InputKey::Char(value),
        };
        self.handle_key(key)
    }

    pub fn handle_key(&mut self, key: InputKey) -> Option<Action> {
        if self.overlay != Overlay::None {
            return self.handle_overlay_key(key);
        }
        match key {
            InputKey::Char('q') => return Some(Action::Quit),
            InputKey::Char('j') | InputKey::Down => self.move_task(true),
            InputKey::Char('k') | InputKey::Up => self.move_task(false),
            InputKey::Char('h') | InputKey::Left => self.view = previous_view(self.view),
            InputKey::Char('l') | InputKey::Right => self.view = next_view(self.view),
            InputKey::Char(':') => self.overlay = Overlay::Palette,
            InputKey::Char('?') => self.overlay = Overlay::Help,
            InputKey::Char('s') => self.overlay = Overlay::Settings,
            InputKey::Char('n') => self.begin_text_entry(Overlay::CreateTask),
            InputKey::Char('r') if self.selected_task().is_some() => {
                self.begin_text_entry(Overlay::RenameTask);
            }
            InputKey::Char('c') => return self.complete_or_reopen_selected(),
            InputKey::Char('D') if self.selected_task().is_some() => {
                self.overlay = Overlay::ConfirmDelete;
            }
            InputKey::Char('K') => {
                return Some(self.emit(pomotui_protocol::Command::Skip));
            }
            InputKey::Char('X') => {
                return Some(self.emit(pomotui_protocol::Command::Stop));
            }
            InputKey::Char(' ') => return self.toggle_session(),
            InputKey::Enter => return Some(self.start_selected_focus()),
            _ => {}
        }
        None
    }

    fn handle_overlay_key(&mut self, key: InputKey) -> Option<Action> {
        if key == InputKey::Escape {
            self.overlay = Overlay::None;
            self.input.clear();
            return None;
        }
        match self.overlay {
            Overlay::Palette => match key {
                InputKey::Char('j') | InputKey::Down => {
                    self.palette_index = (self.palette_index + 1).min(PALETTE_ITEMS.len() - 1);
                    None
                }
                InputKey::Char('k') | InputKey::Up => {
                    self.palette_index = self.palette_index.saturating_sub(1);
                    None
                }
                InputKey::Enter => self.run_palette_item(),
                InputKey::Char('?') => {
                    self.overlay = Overlay::Help;
                    None
                }
                _ => None,
            },
            Overlay::Settings => {
                if matches!(key, InputKey::Char('t') | InputKey::Left | InputKey::Right) {
                    self.theme = match self.theme {
                        Theme::VermilionPaperLight => Theme::VermilionPaperDark,
                        Theme::VermilionPaperDark => Theme::VermilionPaperLight,
                    };
                }
                if key == InputKey::Char('g') {
                    self.language = match self.language {
                        Language::English => Language::SimplifiedChinese,
                        Language::SimplifiedChinese => Language::English,
                    };
                    return Some(Action::SetLanguage(self.language));
                }
                None
            }
            Overlay::Help | Overlay::None => None,
            Overlay::CreateTask | Overlay::RenameTask => match key {
                InputKey::Char(value) if !value.is_control() => {
                    self.input.push(value);
                    None
                }
                InputKey::Backspace => {
                    self.input.pop();
                    None
                }
                InputKey::Enter => self.submit_text_entry(),
                _ => None,
            },
            Overlay::ConfirmDelete => match key {
                InputKey::Char('y' | 'Y') | InputKey::Enter => {
                    let command = self
                        .selected_task()
                        .map(|task| pomotui_protocol::Command::TaskDelete { id: task.id });
                    self.overlay = Overlay::None;
                    command.map(|command| self.emit(command))
                }
                InputKey::Char('n' | 'N') => {
                    self.overlay = Overlay::None;
                    None
                }
                _ => None,
            },
        }
    }

    fn move_task(&mut self, forward: bool) {
        let last = self
            .snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.tasks.len().saturating_sub(1));
        self.selected_task = if forward {
            self.selected_task.saturating_add(1).min(last)
        } else {
            self.selected_task.saturating_sub(1)
        };
    }

    fn selected_task(&self) -> Option<&pomotui_protocol::TaskSummary> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.tasks.get(self.selected_task))
    }

    fn begin_text_entry(&mut self, overlay: Overlay) {
        self.input = if overlay == Overlay::RenameTask {
            self.selected_task()
                .map_or_else(String::new, |task| task.title.clone())
        } else {
            String::new()
        };
        self.overlay = overlay;
    }

    fn submit_text_entry(&mut self) -> Option<Action> {
        let title = self.input.trim().to_owned();
        if title.is_empty() {
            self.message = Some(
                text(
                    self.language,
                    "Task title cannot be empty",
                    "任务标题不能为空",
                )
                .into(),
            );
            return None;
        }
        let command = match self.overlay {
            Overlay::CreateTask => pomotui_protocol::Command::TaskCreate { title },
            Overlay::RenameTask => {
                let id = self.selected_task()?.id;
                pomotui_protocol::Command::TaskRename { id, title }
            }
            _ => return None,
        };
        self.overlay = Overlay::None;
        self.input.clear();
        Some(self.emit(command))
    }

    fn emit(&mut self, command: pomotui_protocol::Command) -> Action {
        self.overlay = Overlay::None;
        Action::Command(command)
    }

    fn start_selected_focus(&mut self) -> Action {
        let task_id = self.selected_task().map(|task| task.id);
        self.emit(pomotui_protocol::Command::Start {
            kind: SessionKind::Focus,
            task_id,
        })
    }

    fn toggle_session(&mut self) -> Option<Action> {
        let command = match self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.state.as_str())
        {
            Some("running") => pomotui_protocol::Command::Pause,
            Some("paused") => pomotui_protocol::Command::Resume,
            Some("pending") => {
                return Some(
                    self.emit(pomotui_protocol::Command::Start {
                        kind: self
                            .snapshot
                            .as_ref()
                            .map_or(SessionKind::Focus, |snapshot| snapshot.kind.clone()),
                        task_id: self.snapshot.as_ref().and_then(|snapshot| {
                            (snapshot.kind == SessionKind::Focus)
                                .then(|| self.selected_task().map(|task| task.id))
                                .flatten()
                        }),
                    }),
                );
            }
            _ => return None,
        };
        Some(self.emit(command))
    }

    fn complete_or_reopen_selected(&mut self) -> Option<Action> {
        let task = self.selected_task()?;
        let command = if task.completed {
            pomotui_protocol::Command::TaskReopen { id: task.id }
        } else {
            pomotui_protocol::Command::TaskComplete { id: task.id }
        };
        Some(self.emit(command))
    }

    fn run_palette_item(&mut self) -> Option<Action> {
        match PALETTE_ITEMS[self.palette_index].command {
            PaletteCommand::Toggle => self.toggle_session(),
            PaletteCommand::Stop => Some(self.emit(pomotui_protocol::Command::Stop)),
            PaletteCommand::Skip => Some(self.emit(pomotui_protocol::Command::Skip)),
            PaletteCommand::StartFocus => Some(self.start_selected_focus()),
            PaletteCommand::StartFocusWithoutTask => {
                Some(self.emit(pomotui_protocol::Command::Start {
                    kind: SessionKind::Focus,
                    task_id: None,
                }))
            }
            PaletteCommand::StartShortBreak => Some(self.emit(pomotui_protocol::Command::Start {
                kind: SessionKind::ShortBreak,
                task_id: None,
            })),
            PaletteCommand::StartLongBreak => Some(self.emit(pomotui_protocol::Command::Start {
                kind: SessionKind::LongBreak,
                task_id: None,
            })),
            PaletteCommand::CreateTask => {
                self.begin_text_entry(Overlay::CreateTask);
                None
            }
            PaletteCommand::RenameTask => {
                self.begin_text_entry(Overlay::RenameTask);
                None
            }
            PaletteCommand::CompleteTask => self.complete_or_reopen_selected(),
            PaletteCommand::DeleteTask => {
                self.overlay = Overlay::ConfirmDelete;
                None
            }
            PaletteCommand::Today => {
                self.view = View::Today;
                self.overlay = Overlay::None;
                None
            }
            PaletteCommand::History => {
                self.view = View::History;
                self.overlay = Overlay::None;
                None
            }
            PaletteCommand::Settings => {
                self.overlay = Overlay::Settings;
                None
            }
            PaletteCommand::Help => {
                self.overlay = Overlay::Help;
                None
            }
        }
    }

    pub fn mouse_click(&mut self, _x: u16, y: u16) -> Option<Action> {
        if self.overlay != Overlay::None {
            self.overlay = Overlay::None;
            return None;
        }
        if y == 0 {
            self.view = View::Dashboard;
            None
        } else if self.view == View::Dashboard && (4..=10).contains(&y) {
            let index = usize::from(y.saturating_sub(4));
            let last = self
                .snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.tasks.len().saturating_sub(1));
            self.selected_task = index.min(last);
            None
        } else if self.view == View::Dashboard && y >= if self.narrow { 12 } else { 14 } {
            self.toggle_session()
        } else {
            None
        }
    }

    pub fn begin_completion(&mut self, animation: animation::Animation) {
        self.completion = Some(CompletionPlayback {
            animation,
            elapsed_ms: 0,
        });
    }

    pub fn animation_tick(&mut self, elapsed_ms: u64) {
        let Some(playback) = &mut self.completion else {
            return;
        };
        playback.elapsed_ms = playback.elapsed_ms.saturating_add(elapsed_ms);
        if playback.animation.frame(playback.elapsed_ms).1 {
            self.completion = None;
        }
    }
}

#[derive(Clone, Copy)]
enum PaletteCommand {
    Toggle,
    Stop,
    Skip,
    StartFocus,
    StartFocusWithoutTask,
    StartShortBreak,
    StartLongBreak,
    CreateTask,
    RenameTask,
    CompleteTask,
    DeleteTask,
    Today,
    History,
    Settings,
    Help,
}

struct PaletteItem {
    label: &'static str,
    hint: &'static str,
    command: PaletteCommand,
}

const PALETTE_ITEMS: [PaletteItem; 15] = [
    PaletteItem {
        label: "Start / pause / resume Current Session",
        hint: "Space",
        command: PaletteCommand::Toggle,
    },
    PaletteItem {
        label: "Stop Current Session",
        hint: "X",
        command: PaletteCommand::Stop,
    },
    PaletteItem {
        label: "Skip Current Session",
        hint: "K",
        command: PaletteCommand::Skip,
    },
    PaletteItem {
        label: "Start Focus with selected Task",
        hint: "Enter",
        command: PaletteCommand::StartFocus,
    },
    PaletteItem {
        label: "Start Focus without a Task",
        hint: "",
        command: PaletteCommand::StartFocusWithoutTask,
    },
    PaletteItem {
        label: "Start Short Break",
        hint: "",
        command: PaletteCommand::StartShortBreak,
    },
    PaletteItem {
        label: "Start Long Break",
        hint: "",
        command: PaletteCommand::StartLongBreak,
    },
    PaletteItem {
        label: "Create task…",
        hint: "n",
        command: PaletteCommand::CreateTask,
    },
    PaletteItem {
        label: "Rename task…",
        hint: "r",
        command: PaletteCommand::RenameTask,
    },
    PaletteItem {
        label: "Complete task / reopen task",
        hint: "c",
        command: PaletteCommand::CompleteTask,
    },
    PaletteItem {
        label: "Delete task…",
        hint: "D",
        command: PaletteCommand::DeleteTask,
    },
    PaletteItem {
        label: "Open Today summary",
        hint: "",
        command: PaletteCommand::Today,
    },
    PaletteItem {
        label: "Open Session History",
        hint: "",
        command: PaletteCommand::History,
    },
    PaletteItem {
        label: "Open Settings",
        hint: "s",
        command: PaletteCommand::Settings,
    },
    PaletteItem {
        label: "Open Help",
        hint: "?",
        command: PaletteCommand::Help,
    },
];

fn palette_label(item: &PaletteItem, language: Language) -> &'static str {
    if language == Language::English {
        return item.label;
    }
    match item.command {
        PaletteCommand::Toggle => "开始 / 暂停 / 继续当前时段",
        PaletteCommand::Stop => "停止当前时段",
        PaletteCommand::Skip => "跳过当前时段",
        PaletteCommand::StartFocus => "用所选任务开始专注",
        PaletteCommand::StartFocusWithoutTask => "无任务开始专注",
        PaletteCommand::StartShortBreak => "开始短休息",
        PaletteCommand::StartLongBreak => "开始长休息",
        PaletteCommand::CreateTask => "新建任务…",
        PaletteCommand::RenameTask => "重命名任务…",
        PaletteCommand::CompleteTask => "完成 / 重新打开任务",
        PaletteCommand::DeleteTask => "删除任务…",
        PaletteCommand::Today => "打开今日汇总",
        PaletteCommand::History => "打开时段历史",
        PaletteCommand::Settings => "打开设置",
        PaletteCommand::Help => "打开帮助",
    }
}

const fn next_view(view: View) -> View {
    match view {
        View::Dashboard => View::Today,
        View::Today => View::History,
        View::History => View::Dashboard,
    }
}

const fn previous_view(view: View) -> View {
    match view {
        View::Dashboard => View::History,
        View::History => View::Today,
        View::Today => View::Dashboard,
    }
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    app.narrow = area.width < 74;
    let colors = colors(app.theme);
    frame.render_widget(
        Block::default().style(Style::default().bg(colors.background).fg(colors.text)),
        area,
    );
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(8),
        Constraint::Length(if app.narrow { 2 } else { 3 }),
    ])
    .split(area);
    header(frame, rows[0], app, colors);
    match app.view {
        View::Dashboard => dashboard(frame, rows[1], app, colors),
        View::Today => today_view(frame, rows[1], app.snapshot.as_ref(), colors, app.language),
        View::History => history_view(frame, rows[1], app.snapshot.as_ref(), colors, app.language),
    }
    footer(frame, rows[2], app, colors);
    render_overlay(frame, area, app, colors);
    if let Some(playback) = &app.completion {
        let (art, _) = playback.animation.frame(playback.elapsed_ms);
        let modal = centered(area, area.width.saturating_sub(8).min(58), 12);
        frame.render_widget(Clear, modal);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    art,
                    Style::default()
                        .fg(colors.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(text(
                    app.language,
                    "Completed Round recorded",
                    "已记录完成轮次",
                )),
                Line::from(Span::styled(
                    text(
                        app.language,
                        "The next Pending Session waits for Start",
                        "下一时段等待手动开始",
                    ),
                    Style::default().fg(colors.muted),
                )),
            ])
            .alignment(Alignment::Center)
            .block(
                panel(text(app.language, "SESSION COMPLETE", "时段完成"), colors)
                    .border_style(Style::default().fg(colors.accent)),
            ),
            modal,
        );
    }
    if let Some(warning) = &app.warning {
        frame.render_widget(
            Paragraph::new(warning.as_str()).style(Style::default().fg(Color::Rgb(201, 166, 107))),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        );
    }
}

fn header(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let session = app.snapshot.as_ref().map_or(
        text(app.language, "RECONNECTING", "正在重连"),
        |snapshot| session_heading(snapshot, app.language),
    );
    let title = if area.width >= 72 {
        format!(
            " POMOTUI  •  {session}  │  {}",
            match app.theme {
                Theme::VermilionPaperLight => "Vermilion Paper Light",
                Theme::VermilionPaperDark => "Vermilion Paper Dark",
            }
        )
    } else {
        format!(" POMOTUI  •  {session}")
    };
    frame.render_widget(
        Paragraph::new(title).style(
            Style::default()
                .fg(colors.text)
                .bg(colors.surface)
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
}

fn dashboard(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    if app.narrow {
        let rows = Layout::vertical([Constraint::Length(8), Constraint::Min(10)]).split(inner);
        tasks_panel(
            frame,
            rows[0],
            app.snapshot.as_ref(),
            app.selected_task,
            colors,
            true,
            app.language,
        );
        timer_panel(
            frame,
            rows[1],
            app.snapshot.as_ref(),
            colors,
            true,
            app.language,
        );
    } else {
        let rows = Layout::vertical([Constraint::Length(10), Constraint::Min(12)]).split(inner);
        let top = Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(rows[0]);
        tasks_panel(
            frame,
            top[0],
            app.snapshot.as_ref(),
            app.selected_task,
            colors,
            false,
            app.language,
        );
        today_panel(frame, top[1], app.snapshot.as_ref(), colors, app.language);
        timer_panel(
            frame,
            rows[1],
            app.snapshot.as_ref(),
            colors,
            false,
            app.language,
        );
    }
}

#[derive(Clone, Copy)]
struct Colors {
    background: Color,
    surface: Color,
    text: Color,
    muted: Color,
    accent: Color,
    gold: Color,
    good: Color,
    border: Color,
}

fn session_colors(snapshot: Option<&Snapshot>, mut colors: Colors) -> Colors {
    if let Some(snapshot) = snapshot {
        colors.accent = if matches!(snapshot.state.as_str(), "paused" | "pending") {
            colors.gold
        } else if snapshot.kind != SessionKind::Focus {
            colors.good
        } else {
            colors.accent
        };
    }
    colors
}

fn tasks_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: Option<&Snapshot>,
    selected: usize,
    colors: Colors,
    narrow: bool,
    language: Language,
) {
    let Some(snapshot) = snapshot else {
        frame.render_widget(
            Paragraph::new(text(
                language,
                "Waiting for Timer Service",
                "正在等待计时服务",
            ))
            .block(panel(text(language, "TASKS", "任务"), colors))
            .style(Style::default().fg(colors.muted)),
            area,
        );
        return;
    };
    let visible = usize::from(area.height.saturating_sub(2));
    let items = snapshot
        .tasks
        .iter()
        .take(visible)
        .enumerate()
        .map(|(index, task)| {
            let marker = if index == selected { "› " } else { "  " };
            let status = if task.completed { "✓ " } else { "" };
            let time = clock(task.focus_seconds);
            let width = usize::from(area.width.saturating_sub(if narrow { 13 } else { 16 }));
            let title = truncate(&task.title, width);
            let line = format!("{marker}{status}{title:<width$} {time}");
            let style = if index == selected {
                Style::default()
                    .fg(colors.background)
                    .bg(colors.accent)
                    .add_modifier(Modifier::BOLD)
            } else if task.completed {
                Style::default()
                    .fg(colors.muted)
                    .add_modifier(Modifier::CROSSED_OUT)
            } else {
                Style::default().fg(colors.text)
            };
            ListItem::new(line).style(style)
        })
        .collect::<Vec<_>>();
    let items = if items.is_empty() {
        vec![
            ListItem::new(text(
                language,
                "No Tasks · press n to create one",
                "暂无任务 · 按 n 新建",
            ))
            .style(Style::default().fg(colors.muted)),
        ]
    } else {
        items
    };
    frame.render_widget(
        List::new(items).block(panel(
            text(
                language,
                "TASKS  ↑↓ select · Enter start · n new · : all actions",
                "任务  ↑↓ 选择 · Enter 开始 · n 新建 · : 全部操作",
            ),
            colors,
        )),
        area,
    );
}

fn today_lines(
    snapshot: Option<&Snapshot>,
    colors: Colors,
    language: Language,
) -> Vec<Line<'static>> {
    let Some(snapshot) = snapshot else {
        return vec![Line::from(text(
            language,
            "Waiting for Timer Service",
            "正在等待计时服务",
        ))];
    };
    let touched = snapshot
        .tasks
        .iter()
        .filter(|task| task.focus_seconds > 0)
        .count();
    vec![
        Line::from(Span::styled(
            human_duration(snapshot.today.focus_seconds),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(text(language, "focus today", "今日专注")),
        Line::from(""),
        Line::from(format!(
            "{} {}",
            snapshot.today.completed_rounds,
            text(language, "Completed Rounds", "已完成轮次")
        )),
        Line::from(format!(
            "{touched} {}",
            text(language, "Tasks touched", "个任务")
        )),
        Line::from(vec![
            Span::styled(
                trend(snapshot.today.seven_day_focus_seconds),
                Style::default().fg(colors.gold),
            ),
            Span::raw(text(language, "  7-day trend", "  7 天趋势")),
        ]),
        Line::from(Span::styled(
            format!(
                "{}  ·  {} {}",
                text(language, "M T W T F S S", "一 二 三 四 五 六 日"),
                text(language, "avg", "平均"),
                human_duration(snapshot.today.average_focus_seconds)
            ),
            Style::default().fg(colors.muted),
        )),
    ]
}

fn today_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: Option<&Snapshot>,
    colors: Colors,
    language: Language,
) {
    frame.render_widget(
        Paragraph::new(today_lines(snapshot, colors, language))
            .block(panel(text(language, "TODAY", "今日"), colors))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn today_view(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: Option<&Snapshot>,
    colors: Colors,
    language: Language,
) {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(
        Paragraph::new(today_lines(snapshot, colors, language))
            .block(panel(
                text(
                    language,
                    "TODAY · DAILY & 7-DAY SUMMARY",
                    "今日 · 当日与 7 天汇总",
                ),
                colors,
            ))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn trend(values: [u64; 7]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let maximum = values.into_iter().max().unwrap_or(0);
    values
        .into_iter()
        .map(|value| {
            let index = value.saturating_mul(7).checked_div(maximum).unwrap_or(0);
            BARS[usize::try_from(index).unwrap_or(7)]
        })
        .collect()
}

fn history_view(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: Option<&Snapshot>,
    colors: Colors,
    language: Language,
) {
    let Some(snapshot) = snapshot else {
        frame.render_widget(
            Paragraph::new(text(
                language,
                "CURRENT  ·  reconnecting",
                "当前 · 正在重连",
            ))
            .block(panel(text(language, "SESSION FLOW", "时段流程"), colors))
            .style(Style::default().fg(colors.muted)),
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );
        return;
    };
    let mut lines = vec![Line::from(Span::styled(
        text(language, "PAST", "过去"),
        Style::default().fg(colors.muted),
    ))];
    if snapshot.recent_history.is_empty() {
        lines.push(Line::from(text(
            language,
            "  No completed Sessions yet",
            "  尚无已完成时段",
        )));
    } else {
        lines.extend(
            snapshot
                .recent_history
                .iter()
                .map(|item| Line::from(format!("  ✓ {item}"))),
        );
    }
    lines.extend([
        Line::from("│"),
        Line::from(vec![
            Span::styled(
                text(language, "● CURRENT  ", "● 当前  "),
                Style::default()
                    .fg(session_colors(Some(snapshot), colors).accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{} · {} · {}",
                session_heading(snapshot, language),
                snapshot.state.to_uppercase(),
                clock(snapshot.remaining_seconds)
            )),
        ]),
        Line::from("│"),
        Line::from(vec![
            Span::styled(
                text(language, "○ NEXT     ", "○ 下一时段  "),
                Style::default().fg(colors.gold),
            ),
            Span::raw(snapshot.next_kind.as_ref().map_or_else(
                || {
                    text(
                        language,
                        "Cycle decision follows this Session",
                        "按专注循环决定",
                    )
                    .into()
                },
                |kind| {
                    format!(
                        "{} · {}",
                        kind_label(kind, language),
                        text(language, "waits for Start", "等待开始")
                    )
                },
            )),
        ]),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(
                text(
                    language,
                    "SESSION FLOW · PAST / CURRENT / NEXT",
                    "时段流程 · 过去 / 当前 / 下一时段",
                ),
                colors,
            ))
            .wrap(Wrap { trim: false }),
        area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }),
    );
}

fn timer_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: Option<&Snapshot>,
    colors: Colors,
    narrow: bool,
    language: Language,
) {
    let Some(snapshot) = snapshot else {
        disconnected_timer_panel(frame, area, colors, language);
        return;
    };
    let state_colors = session_colors(Some(snapshot), colors);
    let heading = session_heading(snapshot, language);
    let task = snapshot.current_task.as_deref().unwrap_or(text(
        language,
        "No Task selected",
        "未选择任务",
    ));
    let task_time = snapshot
        .current_task_id
        .and_then(|id| snapshot.tasks.iter().find(|task| task.id == id))
        .map_or(0, |task| task.focus_seconds);
    let title = if narrow {
        format!(
            "{heading}  ·  {} {}/{}",
            text(language, "Round", "轮次"),
            snapshot.completed_rounds.saturating_add(1),
            snapshot.rounds_per_cycle
        )
    } else {
        heading.to_owned()
    };
    let remaining = clock(snapshot.remaining_seconds);
    let lines = if narrow || area.height < 16 {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                remaining,
                Style::default()
                    .fg(state_colors.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                task.to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!(
                    "{}  {}",
                    text(language, "Task time", "任务时间"),
                    clock(task_time)
                ),
                Style::default().fg(colors.muted),
            )),
            Line::from(next_session_line(snapshot, language)),
        ]
    } else {
        let mut lines = vec![Line::from("")];
        lines.extend(big_clock(&remaining).into_iter().map(|row| {
            Line::from(Span::styled(
                row,
                Style::default()
                    .fg(state_colors.accent)
                    .add_modifier(Modifier::BOLD),
            ))
        }));
        lines.extend([
            Line::from(""),
            Line::from(Span::styled(
                task.to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(format!(
                "{}  {}",
                text(language, "Task time", "任务时间"),
                clock(task_time)
            )),
            Line::from(Span::styled(
                format!(
                    "{} {} / {}  •  {}",
                    text(language, "Round", "轮次"),
                    snapshot.completed_rounds.saturating_add(1),
                    snapshot.rounds_per_cycle,
                    next_session_line(snapshot, language)
                ),
                Style::default().fg(colors.muted),
            )),
        ]);
        lines
    };
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(panel(&title, colors)),
        area,
    );
    timer_progress(frame, area, snapshot, state_colors, colors);
}

fn disconnected_timer_panel(frame: &mut Frame<'_>, area: Rect, colors: Colors, language: Language) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                text(language, "RECONNECTING", "正在重连"),
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(text(
                language,
                "Timer Service unavailable",
                "计时服务不可用",
            )),
        ])
        .alignment(Alignment::Center)
        .block(panel(text(language, "CURRENT SESSION", "当前时段"), colors)),
        area,
    );
}

fn timer_progress(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: &Snapshot,
    state_colors: Colors,
    colors: Colors,
) {
    if area.height <= 9 {
        return;
    }
    let elapsed = snapshot
        .planned_seconds
        .saturating_sub(snapshot.remaining_seconds);
    let percentage = elapsed
        .saturating_mul(100)
        .checked_div(snapshot.planned_seconds)
        .unwrap_or(0)
        .min(100);
    let percentage = u32::try_from(percentage).unwrap_or(100);
    let ratio = f64::from(percentage) / 100.0;
    let gauge_area = Rect::new(
        area.x.saturating_add(3),
        area.bottom().saturating_sub(3),
        area.width.saturating_sub(6),
        1,
    );
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .label(format!("{percentage}%"))
            .gauge_style(Style::default().fg(state_colors.accent).bg(colors.surface)),
        gauge_area,
    );
}

fn footer(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let view = match app.view {
        View::Dashboard => text(app.language, "DASHBOARD", "仪表盘"),
        View::Today => text(app.language, "TODAY", "今日"),
        View::History => text(app.language, "HISTORY", "历史"),
    };
    let keys = if app.narrow {
        text(
            app.language,
            "h/l views · j/k Tasks · Space toggle · : commands · ? help",
            "h/l 视图 · j/k 任务 · Space 切换 · : 命令 · ? 帮助",
        )
    } else {
        text(
            app.language,
            "h/l or ←/→ views  j/k Tasks  Enter start  Space toggle  n new Task  : commands  ? help  q quit",
            "h/l 或 ←/→ 切换视图  j/k 任务  Enter 开始  Space 切换  n 新建  : 命令  ? 帮助  q 退出",
        )
    };
    let mut lines = vec![Line::from(Span::styled(
        format!("  ◀  {view}  ▶  "),
        Style::default()
            .fg(colors.background)
            .bg(colors.gold)
            .add_modifier(Modifier::BOLD),
    ))];
    if area.height > 1 {
        lines.push(Line::from(Span::styled(
            keys,
            Style::default().fg(colors.muted),
        )));
    }
    if area.height > 2 {
        lines.push(Line::from(Span::styled(
            app.message.as_deref().unwrap_or(text(
                app.language,
                "Timer Service connected",
                "计时服务已连接",
            )),
            Style::default().fg(colors.muted),
        )));
    }
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

fn render_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    if app.overlay == Overlay::None {
        return;
    }
    let width = area.width.saturating_sub(4).min(76);
    let height = match app.overlay {
        Overlay::Palette => area.height.saturating_sub(4).min(21),
        Overlay::Help => area.height.saturating_sub(4).min(20),
        Overlay::Settings => 17.min(area.height.saturating_sub(2)),
        Overlay::CreateTask | Overlay::RenameTask | Overlay::ConfirmDelete => {
            7.min(area.height.saturating_sub(2))
        }
        Overlay::None => return,
    };
    let modal = if app.narrow {
        area
    } else {
        centered(area, width, height)
    };
    frame.render_widget(Clear, modal);
    match app.overlay {
        Overlay::Palette => palette_overlay(frame, modal, app, colors),
        Overlay::Help => help_overlay(frame, modal, app, colors),
        Overlay::Settings => settings_overlay(frame, modal, app, colors),
        Overlay::CreateTask | Overlay::RenameTask => text_entry_overlay(frame, modal, app, colors),
        Overlay::ConfirmDelete => confirm_delete_overlay(frame, modal, app, colors),
        Overlay::None => {}
    }
}

fn palette_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let visible = usize::from(area.height.saturating_sub(4));
    let start = app.palette_index.saturating_sub(visible.saturating_sub(1));
    let items = PALETTE_ITEMS
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(index, item)| {
            let marker = if index == app.palette_index {
                "› "
            } else {
                "  "
            };
            let hint_width = 8;
            let label_width = usize::from(area.width.saturating_sub(16));
            let line = format!(
                "{marker}{:<label_width$}{:>hint_width$}",
                truncate(palette_label(item, app.language), label_width),
                item.hint
            );
            let style = if index == app.palette_index {
                Style::default()
                    .fg(colors.background)
                    .bg(colors.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text)
            };
            ListItem::new(line).style(style)
        })
        .collect::<Vec<_>>();
    let block = panel(text(app.language, "COMMAND PALETTE", "命令面板"), colors)
        .border_style(Style::default().fg(colors.accent))
        .title_bottom(text(
            app.language,
            " ↑↓ choose · Enter run · Esc close ",
            " ↑↓ 选择 · Enter 执行 · Esc 关闭 ",
        ));
    frame.render_widget(List::new(items).block(block), area);
}

fn help_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let lines = if app.language == Language::SimplifiedChinese {
        vec![
            Line::from(Span::styled(
                "导航",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  h/l 或 ←/→  切换仪表盘、今日、历史"),
            Line::from("  j/k 或 ↑/↓  选择任务                    q  退出"),
            Line::from(""),
            Line::from(Span::styled(
                "时段",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  Enter  用所选任务开始专注    Space  开始/暂停/继续"),
            Line::from("  X      停止当前时段          K      跳过当前时段"),
            Line::from(""),
            Line::from(Span::styled(
                "任务",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  n 新建   r 重命名   c 完成/重新打开   D 删除"),
            Line::from(""),
            Line::from(Span::styled(
                "工具",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  : 命令面板   s 设置   Esc 关闭浮层"),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "NAVIGATION",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  h/l or ←/→  switch Dashboard, Today, History"),
            Line::from("  j/k or ↑/↓  select a Task                 q  quit"),
            Line::from(""),
            Line::from(Span::styled(
                "SESSIONS",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  Enter  start Focus with selected Task    Space  start/pause/resume"),
            Line::from("  X      stop Current Session              K      skip Current Session"),
            Line::from(""),
            Line::from(Span::styled(
                "TASKS",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  n  Create task     r  Rename task       c  Complete/reopen task"),
            Line::from("  D  Delete task (confirmation required)"),
            Line::from(""),
            Line::from(Span::styled(
                "TOOLS",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  :  executable command palette            s  settings"),
            Line::from("  Esc closes any overlay · Mouse mirrors visible primary targets"),
        ]
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                panel(
                    text(app.language, "HELP · KEYBOARD & MOUSE", "帮助 · 键盘与鼠标"),
                    colors,
                )
                .border_style(Style::default().fg(colors.accent))
                .title_bottom(text(app.language, " Esc close ", " Esc 关闭 ")),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn settings_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let theme = match app.theme {
        Theme::VermilionPaperLight => "Vermilion Paper Light",
        Theme::VermilionPaperDark => "Vermilion Paper Dark",
    };
    let language = match app.language {
        Language::English => "English",
        Language::SimplifiedChinese => "简体中文",
    };
    let lines = vec![
        Line::from(Span::styled(
            text(app.language, "APPEARANCE", "外观"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "  {}                    {theme}",
            text(app.language, "Theme", "主题")
        )),
        Line::from(text(
            app.language,
            "  t or ←/→                 preview theme",
            "  t 或 ←/→                  预览主题",
        )),
        Line::from(format!(
            "  {}                 {language}",
            text(app.language, "Language", "语言")
        )),
        Line::from(text(
            app.language,
            "  g                         switch and save language",
            "  g                         切换并保存语言",
        )),
        Line::from(""),
        Line::from(Span::styled(
            text(app.language, "CONFIGURATION", "配置"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(text(
            app.language,
            "  Session durations, cycle, reminder, sound and animation",
            "  时段长度、循环、提醒、声音和动画",
        )),
        Line::from(text(
            app.language,
            "  are loaded from ~/.config/pomotui/config.toml.",
            "  从 ~/.config/pomotui/config.toml 加载。",
        )),
        Line::from(""),
        Line::from(Span::styled(
            text(
                app.language,
                "Theme changes preview this TUI only; language is saved.",
                "主题仅在本次界面预览；语言会保存。",
            ),
            Style::default().fg(colors.muted),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                panel(text(app.language, "SETTINGS", "设置"), colors)
                    .border_style(Style::default().fg(colors.accent))
                    .title_bottom(text(app.language, " Esc close ", " Esc 关闭 ")),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn text_entry_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let title = if app.overlay == Overlay::CreateTask {
        text(app.language, "CREATE TASK", "新建任务")
    } else {
        text(app.language, "RENAME TASK", "重命名任务")
    };
    let cursor = format!("{}█", app.input);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(text(app.language, "Task title", "任务标题")),
            Line::from(Span::styled(
                cursor,
                Style::default()
                    .fg(colors.text)
                    .bg(colors.surface)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                text(
                    app.language,
                    "Enter save · Esc cancel",
                    "Enter 保存 · Esc 取消",
                ),
                Style::default().fg(colors.muted),
            )),
        ])
        .block(panel(title, colors).border_style(Style::default().fg(colors.accent))),
        area,
    );
}

fn confirm_delete_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let task = app
        .selected_task()
        .map_or(text(app.language, "selected Task", "所选任务"), |task| {
            task.title.as_str()
        });
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{} “{task}”？",
                text(app.language, "Delete", "删除")
            )),
            Line::from(text(
                app.language,
                "Session History will be preserved.",
                "时段历史将会保留。",
            )),
            Line::from(Span::styled(
                text(
                    app.language,
                    "y / Enter delete · n / Esc cancel",
                    "y / Enter 删除 · n / Esc 取消",
                ),
                Style::default().fg(colors.muted),
            )),
        ])
        .block(
            panel(text(app.language, "CONFIRM DELETE", "确认删除"), colors)
                .border_style(Style::default().fg(colors.accent)),
        ),
        area,
    );
}

const fn colors(theme: Theme) -> Colors {
    match theme {
        Theme::VermilionPaperLight => Colors {
            background: Color::Rgb(247, 243, 235),
            surface: Color::Rgb(235, 228, 216),
            text: Color::Rgb(33, 29, 26),
            muted: Color::Rgb(105, 96, 88),
            accent: Color::Rgb(159, 45, 36),
            gold: Color::Rgb(155, 116, 56),
            good: Color::Rgb(55, 112, 79),
            border: Color::Rgb(175, 160, 145),
        },
        Theme::VermilionPaperDark => Colors {
            background: Color::Rgb(17, 16, 15),
            surface: Color::Rgb(33, 29, 26),
            text: Color::Rgb(247, 243, 235),
            muted: Color::Rgb(170, 157, 145),
            accent: Color::Rgb(214, 107, 95),
            gold: Color::Rgb(201, 166, 107),
            good: Color::Rgb(112, 177, 132),
            border: Color::Rgb(84, 73, 65),
        },
    }
}

fn panel(title: &str, colors: Colors) -> Block<'_> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.border))
        .style(Style::default().fg(colors.text).bg(colors.background))
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn session_heading(snapshot: &Snapshot, language: Language) -> &'static str {
    if snapshot.state == "paused" {
        text(language, "PAUSED FOCUS", "专注已暂停")
    } else if snapshot.state == "pending" {
        text(language, "PENDING SESSION", "时段待开始")
    } else {
        match snapshot.kind {
            SessionKind::Focus => text(language, "FOCUS SESSION", "专注时段"),
            SessionKind::ShortBreak => text(language, "SHORT BREAK", "短休息"),
            SessionKind::LongBreak => text(language, "LONG BREAK", "长休息"),
        }
    }
}

fn next_session_line(snapshot: &Snapshot, language: Language) -> String {
    snapshot.next_kind.as_ref().map_or_else(
        || {
            text(
                language,
                "Next Session follows the Focus Cycle",
                "下一时段按专注循环决定",
            )
            .into()
        },
        |kind| {
            format!(
                "{}: {} · {}",
                text(language, "Next", "下一时段"),
                kind_label(kind, language),
                text(language, "waits for Start", "等待开始")
            )
        },
    )
}

fn clock(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn human_duration(seconds: u64) -> String {
    let hours = seconds / 3_600;
    let minutes = seconds % 3_600 / 60;
    if hours == 0 {
        format!("{minutes}m")
    } else {
        format!("{hours}h {minutes:02}m")
    }
}

fn truncate(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        return value.to_owned();
    }
    if maximum <= 1 {
        return "…".chars().take(maximum).collect();
    }
    let mut output = value.chars().take(maximum - 1).collect::<String>();
    output.push('…');
    output
}

fn big_clock(value: &str) -> Vec<String> {
    const DIGITS: [[&str; 5]; 10] = [
        ["█████", "██ ██", "██ ██", "██ ██", "█████"],
        ["  ██ ", " ███ ", "  ██ ", "  ██ ", "█████"],
        ["█████", "   ██", "█████", "██   ", "█████"],
        ["█████", "   ██", " ████", "   ██", "█████"],
        ["██ ██", "██ ██", "█████", "   ██", "   ██"],
        ["█████", "██   ", "█████", "   ██", "█████"],
        ["█████", "██   ", "█████", "██ ██", "█████"],
        ["█████", "   ██", "  ██ ", " ██  ", "██   "],
        ["█████", "██ ██", "█████", "██ ██", "█████"],
        ["█████", "██ ██", "█████", "   ██", "█████"],
    ];
    let mut rows = vec![String::new(); 5];
    for character in value.chars() {
        let glyph = if character == ':' {
            ["     ", "  █  ", "     ", "  █  ", "     "]
        } else {
            DIGITS[character.to_digit(10).unwrap_or(0) as usize]
        };
        for (row, part) in rows.iter_mut().zip(glyph) {
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(part);
        }
    }
    rows
}

#[must_use]
pub const fn kind_label(kind: &SessionKind, language: Language) -> &'static str {
    match kind {
        SessionKind::Focus => text(language, "Focus", "专注"),
        SessionKind::ShortBreak => text(language, "Short Break", "短休息"),
        SessionKind::LongBreak => text(language, "Long Break", "长休息"),
    }
}

const fn text(language: Language, english: &'static str, chinese: &'static str) -> &'static str {
    match language {
        Language::English => english,
        Language::SimplifiedChinese => chinese,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn snapshot(state: &str, kind: SessionKind) -> Snapshot {
        Snapshot {
            state: state.into(),
            kind,
            remaining_seconds: 90,
            planned_seconds: 1_500,
            current_task: Some("Ship Pomotui".into()),
            current_task_id: Some(1),
            completed_rounds: 2,
            rounds_per_cycle: 4,
            next_kind: Some(SessionKind::ShortBreak),
            tasks: vec![pomotui_protocol::TaskSummary {
                id: 1,
                title: "Ship Pomotui".into(),
                completed: false,
                focus_seconds: 1_500,
            }],
            today: pomotui_protocol::TodaySummary {
                focus_seconds: 3_000,
                completed_rounds: 2,
                seven_day_focus_seconds: [0, 600, 1_200, 900, 1_500, 2_400, 3_000],
                average_focus_seconds: 1_371,
            },
            recent_history: vec!["Focus Completed 1500s".into()],
        }
    }

    #[test]
    fn wide_and_narrow_dashboard_render_all_session_states_and_themes() {
        for (width, height) in [(100, 32), (60, 24)] {
            for theme in [Theme::VermilionPaperLight, Theme::VermilionPaperDark] {
                for (state, kind) in [
                    ("running", SessionKind::Focus),
                    ("running", SessionKind::ShortBreak),
                    ("running", SessionKind::LongBreak),
                    ("paused", SessionKind::Focus),
                    ("pending", SessionKind::Focus),
                    ("completed", SessionKind::Focus),
                ] {
                    let mut terminal =
                        Terminal::new(TestBackend::new(width, height)).expect("terminal");
                    let mut app = App::new(Some(snapshot(state, kind)), theme);
                    terminal
                        .draw(|frame| render(frame, &mut app))
                        .expect("draw");
                    let text = terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>();
                    assert!(text.contains(session_heading(
                        app.snapshot.as_ref().expect("snapshot"),
                        app.language,
                    )));
                    assert!(text.contains("POMOTUI"));
                }
            }
        }
    }

    #[test]
    fn large_countdown_digits_use_a_balanced_five_column_canvas() {
        let one = big_clock("1");
        assert_eq!(
            one,
            ["  ██ ", " ███ ", "  ██ ", "  ██ ", "█████"],
            "the large 1 must have a base and shoulder instead of a thin stroke"
        );
        assert!(
            big_clock("25:00")
                .iter()
                .all(|row| row.chars().count() == 29),
            "five glyphs and their separators must have stable geometry"
        );
    }

    #[test]
    fn disconnected_and_navigation_paths_are_explicit() {
        let mut app = App::new(None, Theme::VermilionPaperDark);
        assert_eq!(
            app.key('K'),
            Some(Action::Command(pomotui_protocol::Command::Skip))
        );
        app.key('l');
        assert_eq!(app.view, View::Today);
        app.key('s');
        assert_eq!(app.overlay, Overlay::Settings);
        let original_theme = app.theme;
        app.key('t');
        assert_ne!(app.theme, original_theme);
        app.key('\u{1b}');
        app.overlay = Overlay::Palette;
        app.palette_index = 2;
        assert_eq!(
            app.handle_key(InputKey::Enter),
            Some(Action::Command(pomotui_protocol::Command::Skip))
        );
        assert_eq!(app.mouse_click(2, 5), None);
        app.view = View::Dashboard;
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("disconnected draw");
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(text.contains("RECONNECTING"));
        assert!(text.contains("Timer Service unavailable"));
    }

    #[test]
    fn settings_and_semantic_colors_are_responsive() {
        let mut wide = Terminal::new(TestBackend::new(100, 32)).expect("wide");
        let mut app = App::new(
            Some(snapshot("paused", SessionKind::Focus)),
            Theme::VermilionPaperDark,
        );
        app.overlay = Overlay::Settings;
        wide.draw(|frame| render(frame, &mut app))
            .expect("draw wide");
        assert!(!app.narrow);
        assert_eq!(wide.backend().buffer()[(0, 0)].symbol(), " ");
        assert!(
            wide.backend()
                .buffer()
                .content()
                .iter()
                .any(|cell| cell.symbol().contains('S'))
        );

        let mut narrow = Terminal::new(TestBackend::new(60, 24)).expect("narrow");
        narrow
            .draw(|frame| render(frame, &mut app))
            .expect("draw narrow");
        assert!(app.narrow);
        assert_eq!(narrow.backend().buffer()[(0, 0)].symbol(), "┌");

        app.view = View::Dashboard;
        app.overlay = Overlay::None;
        narrow
            .draw(|frame| render(frame, &mut app))
            .expect("paused draw");
        assert!(
            narrow
                .backend()
                .buffer()
                .content()
                .iter()
                .any(|cell| cell.fg == Color::Rgb(201, 166, 107))
        );
    }

    #[test]
    fn completion_animation_is_frontend_only_and_pending_remains_pending() {
        let mut app = App::new(
            Some(snapshot("pending", SessionKind::ShortBreak)),
            Theme::VermilionPaperDark,
        );
        app.begin_completion(animation::built_in());
        app.animation_tick(90);
        assert_eq!(app.snapshot.as_ref().expect("snapshot").state, "pending");
        assert_eq!(app.key('x'), None);
    }

    #[test]
    fn escape_closes_command_palette() {
        let mut app = App::new(
            Some(snapshot("pending", SessionKind::Focus)),
            Theme::VermilionPaperDark,
        );
        app.key(':');
        assert_eq!(app.overlay, Overlay::Palette);

        app.key('\u{1b}');

        assert_eq!(app.overlay, Overlay::None);
    }

    #[test]
    fn command_palette_exposes_task_management() {
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).expect("terminal");
        let mut app = App::new(
            Some(snapshot("pending", SessionKind::Focus)),
            Theme::VermilionPaperDark,
        );
        app.overlay = Overlay::Palette;
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("palette draw");
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();

        assert!(text.contains("Create task"));
        assert!(text.contains("Rename task"));
        assert!(text.contains("Complete task"));
        assert!(text.contains("Delete task"));
    }

    #[test]
    fn help_is_structured_by_navigation_sessions_and_tasks() {
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).expect("terminal");
        let mut app = App::new(
            Some(snapshot("pending", SessionKind::Focus)),
            Theme::VermilionPaperDark,
        );
        app.overlay = Overlay::Help;
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("help draw");
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();

        assert!(text.contains("NAVIGATION"));
        assert!(text.contains("SESSIONS"));
        assert!(text.contains("TASKS"));
    }

    #[test]
    fn simplified_chinese_dashboard_settings_and_help_are_discoverable() {
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).expect("terminal");
        let mut app = App::new(
            Some(snapshot("pending", SessionKind::Focus)),
            Theme::VermilionPaperDark,
        );
        app.language = Language::SimplifiedChinese;
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("dashboard");
        let dashboard = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(dashboard.contains("待"));
        assert!(dashboard.contains("专"));
        assert!(dashboard.contains("下"));

        app.overlay = Overlay::Settings;
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("settings");
        let settings = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(settings.contains("简"));
        assert!(settings.contains("保"));
        assert_eq!(
            app.handle_key(InputKey::Char('g')),
            Some(Action::SetLanguage(Language::English))
        );

        app.language = Language::SimplifiedChinese;
        app.overlay = Overlay::Help;
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("help");
        let help = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(help.contains("导"));
        assert!(help.contains("时"));
        assert!(help.contains("任"));
    }

    #[test]
    fn task_workflows_emit_the_same_protocol_commands_as_the_cli() {
        let mut app = App::new(
            Some(snapshot("pending", SessionKind::Focus)),
            Theme::VermilionPaperDark,
        );

        app.key('n');
        for character in "New task".chars() {
            app.handle_key(InputKey::Char(character));
        }
        assert_eq!(
            app.handle_key(InputKey::Enter),
            Some(Action::Command(pomotui_protocol::Command::TaskCreate {
                title: "New task".into()
            }))
        );

        app.key('r');
        app.input.clear();
        for character in "Renamed".chars() {
            app.handle_key(InputKey::Char(character));
        }
        assert_eq!(
            app.handle_key(InputKey::Enter),
            Some(Action::Command(pomotui_protocol::Command::TaskRename {
                id: 1,
                title: "Renamed".into()
            }))
        );
        assert_eq!(
            app.key('c'),
            Some(Action::Command(pomotui_protocol::Command::TaskComplete {
                id: 1
            }))
        );

        app.key('D');
        assert_eq!(app.overlay, Overlay::ConfirmDelete);
        assert_eq!(
            app.key('y'),
            Some(Action::Command(pomotui_protocol::Command::TaskDelete {
                id: 1
            }))
        );
    }
}
