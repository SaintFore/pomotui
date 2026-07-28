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
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    VermilionPaperLight,
    VermilionPaperDark,
    RanPaperLight,
    RanPaperDark,
}

impl Theme {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Vermilion Paper Light" => Some(Self::VermilionPaperLight),
            "Vermilion Paper Dark" => Some(Self::VermilionPaperDark),
            "Ran Paper Light" => Some(Self::RanPaperLight),
            "Ran Paper Dark" => Some(Self::RanPaperDark),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VermilionPaperLight => "Vermilion Paper Light",
            Self::VermilionPaperDark => "Vermilion Paper Dark",
            Self::RanPaperLight => "Ran Paper Light",
            Self::RanPaperDark => "Ran Paper Dark",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::VermilionPaperLight => Self::VermilionPaperDark,
            Self::VermilionPaperDark => Self::RanPaperLight,
            Self::RanPaperLight => Self::RanPaperDark,
            Self::RanPaperDark => Self::VermilionPaperLight,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ColorOverrides {
    pub background: Option<Color>,
    pub surface: Option<Color>,
    pub text: Option<Color>,
    pub muted: Option<Color>,
    pub accent: Option<Color>,
    pub gold: Option<Color>,
    pub good: Option<Color>,
    pub border: Option<Color>,
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
    Review,
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
    ConfirmTaskSwitch,
    ConfirmHistoryDelete,
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
    AltSpace,
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
    pub history_cursor: usize,
    pub history_offset: usize,
    pub visual_anchor: Option<usize>,
    pub marked_history: std::collections::BTreeSet<u64>,
    pub color_overrides: ColorOverrides,
    pub pending_g: bool,
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
            history_cursor: 0,
            history_offset: 0,
            visual_anchor: None,
            marked_history: std::collections::BTreeSet::new(),
            color_overrides: ColorOverrides {
                background: None,
                surface: None,
                text: None,
                muted: None,
                accent: None,
                gold: None,
                good: None,
                border: None,
            },
            pending_g: false,
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
            InputKey::Char('g') => {
                if self.pending_g {
                    self.jump_first();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            InputKey::Char('G') => {
                self.pending_g = false;
                self.jump_last();
            }
            InputKey::Char('u') => {
                self.pending_g = false;
                self.page_move(false);
            }
            InputKey::Char('d') => {
                self.pending_g = false;
                self.page_move(true);
            }
            InputKey::Char('v') if self.view == View::History => {
                self.visual_anchor = self
                    .visual_anchor
                    .map_or(Some(self.history_cursor), |_| None);
            }
            InputKey::Char('j') | InputKey::Down => {
                if self.view == View::History {
                    self.scroll_history(true);
                } else {
                    self.move_task(true);
                }
            }
            InputKey::Char('k') | InputKey::Up => {
                if self.view == View::History {
                    self.scroll_history(false);
                } else {
                    self.move_task(false);
                }
            }
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
            InputKey::Char('D') => {
                if self.view == View::History && !self.history_ids_for_action().is_empty() {
                    self.overlay = Overlay::ConfirmHistoryDelete;
                } else if self.selected_task().is_some() {
                    self.overlay = Overlay::ConfirmDelete;
                }
            }
            InputKey::Char('K') => {
                return Some(self.emit(pomotui_protocol::Command::Skip));
            }
            InputKey::Char('X') => {
                return Some(self.emit(pomotui_protocol::Command::Stop));
            }
            InputKey::Char(' ') | InputKey::AltSpace if self.view == View::History => {
                self.toggle_history_mark();
            }
            InputKey::Char(' ') => return self.toggle_session(),
            InputKey::Enter => return self.select_current_task(),
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
                    self.theme = self.theme.next();
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
            Overlay::ConfirmTaskSwitch => match key {
                InputKey::Char('y' | 'Y') | InputKey::Enter => {
                    let command =
                        self.selected_task()
                            .map(|task| pomotui_protocol::Command::TaskSelect {
                                id: task.id,
                                stop_current: true,
                            });
                    self.overlay = Overlay::None;
                    command.map(|command| self.emit(command))
                }
                InputKey::Char('n' | 'N') => {
                    self.overlay = Overlay::None;
                    None
                }
                _ => None,
            },
            Overlay::ConfirmHistoryDelete => match key {
                InputKey::Char('y' | 'Y') | InputKey::Enter => {
                    let ids = self.history_ids_for_action();
                    self.overlay = Overlay::None;
                    self.visual_anchor = None;
                    self.marked_history.clear();
                    Some(self.emit(pomotui_protocol::Command::HistoryDelete { ids }))
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

    fn scroll_history(&mut self, forward: bool) {
        let last = self.snapshot.as_ref().map_or(0, |snapshot| {
            snapshot.recent_history.len().saturating_sub(1)
        });
        self.history_cursor = if forward {
            self.history_cursor.saturating_add(1).min(last)
        } else {
            self.history_cursor.saturating_sub(1)
        };
    }

    fn jump_first(&mut self) {
        if self.view == View::History {
            self.history_cursor = 0;
        } else {
            self.selected_task = 0;
        }
    }

    fn jump_last(&mut self) {
        if self.view == View::History {
            self.history_cursor = self.snapshot.as_ref().map_or(0, |snapshot| {
                snapshot.recent_history.len().saturating_sub(1)
            });
        } else {
            self.selected_task = self
                .snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.tasks.len().saturating_sub(1));
        }
    }

    fn page_move(&mut self, forward: bool) {
        for _ in 0..5 {
            if self.view == View::History {
                self.scroll_history(forward);
            } else {
                self.move_task(forward);
            }
        }
    }

    fn history_ids_for_action(&self) -> Vec<u64> {
        if !self.marked_history.is_empty() {
            return self.marked_history.iter().copied().collect();
        }
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let anchor = self.visual_anchor.unwrap_or(self.history_cursor);
        let start = anchor.min(self.history_cursor);
        let end = anchor.max(self.history_cursor);
        snapshot
            .recent_history
            .get(start..=end)
            .unwrap_or_default()
            .iter()
            .map(|record| record.id)
            .collect()
    }

    fn toggle_history_mark(&mut self) {
        let Some(id) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.recent_history.get(self.history_cursor))
            .map(|record| record.id)
        else {
            return;
        };
        if !self.marked_history.remove(&id) {
            self.marked_history.insert(id);
        }
    }

    fn selected_task(&self) -> Option<&pomotui_protocol::TaskSummary> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.tasks.get(self.selected_task))
    }

    fn select_current_task(&mut self) -> Option<Action> {
        let id = self.selected_task()?.id;
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.current_task_id == Some(id))
        {
            return None;
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| matches!(snapshot.state.as_str(), "running" | "paused"))
        {
            self.overlay = Overlay::ConfirmTaskSwitch;
            None
        } else {
            Some(self.emit(pomotui_protocol::Command::TaskSelect {
                id,
                stop_current: false,
            }))
        }
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
            PaletteCommand::Review => {
                self.view = View::Review;
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
    Review,
    History,
    Settings,
    Help,
}

struct PaletteItem {
    label: &'static str,
    hint: &'static str,
    command: PaletteCommand,
}

const PALETTE_ITEMS: [PaletteItem; 16] = [
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
        label: "Open History review",
        hint: "",
        command: PaletteCommand::Review,
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
        PaletteCommand::Review => "打开历史复盘",
        PaletteCommand::History => "打开时段历史",
        PaletteCommand::Settings => "打开设置",
        PaletteCommand::Help => "打开帮助",
    }
}

const fn next_view(view: View) -> View {
    match view {
        View::Dashboard => View::Today,
        View::Today => View::Review,
        View::Review => View::History,
        View::History => View::Dashboard,
    }
}

const fn previous_view(view: View) -> View {
    match view {
        View::Dashboard => View::History,
        View::History => View::Review,
        View::Review => View::Today,
        View::Today => View::Dashboard,
    }
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    app.narrow = area.width < 74;
    let colors = colors(app.theme, app.color_overrides);
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
    if app.view == View::History {
        let visible = history_visible_records(rows[1], app.narrow);
        let total = app
            .snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.recent_history.len());
        app.history_cursor = app.history_cursor.min(total.saturating_sub(1));
        let valid_ids = app.snapshot.as_ref().map(|snapshot| {
            snapshot
                .recent_history
                .iter()
                .map(|record| record.id)
                .collect::<std::collections::BTreeSet<_>>()
        });
        app.marked_history
            .retain(|id| valid_ids.as_ref().is_some_and(|ids| ids.contains(id)));
        if app.history_cursor < app.history_offset {
            app.history_offset = app.history_cursor;
        } else if app.history_cursor >= app.history_offset.saturating_add(visible) {
            app.history_offset = app.history_cursor.saturating_add(1).saturating_sub(visible);
        }
        app.history_offset = app.history_offset.min(total.saturating_sub(visible));
    }
    header(frame, rows[0], app, colors);
    match app.view {
        View::Dashboard => dashboard(frame, rows[1], app, colors),
        View::Today => today_view(frame, rows[1], app.snapshot.as_ref(), colors, app.language),
        View::Review => review_view(frame, rows[1], app.snapshot.as_ref(), colors, app.language),
        View::History => history_view(
            frame,
            rows[1],
            app.snapshot.as_ref(),
            colors,
            app.language,
            app.history_offset,
            app.history_cursor,
            app.visual_anchor,
            &app.marked_history,
        ),
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
        format!(" POMOTUI  •  {session}  │  {}", app.theme.name())
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
            let line = format!("{marker}{status}{} {time}", pad_display(&title, width));
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
                "TASKS  ↑↓ move · Enter select · Space start · n new · : actions",
                "任务  ↑↓ 移动 · Enter 选择 · Space 开始 · n 新建 · : 操作",
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
    let touched = snapshot.today.task_focus.len();
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
        Paragraph::new(today_report_lines(snapshot, colors, language))
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

fn today_report_lines(
    snapshot: Option<&Snapshot>,
    colors: Colors,
    language: Language,
) -> Vec<Line<'static>> {
    let Some(snapshot) = snapshot else {
        return vec![
            Line::from(""),
            Line::from(Span::styled(
                text(language, "RECONNECTING", "正在重连"),
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(text(
                language,
                "Today report is waiting for the Timer Service.",
                "今日报告正在等待计时服务。",
            )),
        ];
    };
    let mut lines = vec![
        Line::from(Span::styled(
            text(language, "TODAY AT A GLANCE", "今日概览"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "  {}  {}    │    {}  {}    │    {}  {}",
            human_duration(snapshot.today.focus_seconds),
            text(language, "Focus time", "专注时间"),
            snapshot.today.completed_rounds,
            text(language, "Completed Rounds", "完成轮次"),
            snapshot.today.task_focus.len(),
            text(language, "Tasks touched", "涉及任务")
        )),
        Line::from(""),
        Line::from(Span::styled(
            text(language, "7-DAY FOCUS", "7 天专注"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    let maximum = snapshot
        .today
        .seven_day_focus_seconds
        .into_iter()
        .max()
        .unwrap_or(0);
    for index in 0..7 {
        let seconds = snapshot.today.seven_day_focus_seconds[index];
        let label = if index == 6 {
            text(language, "Today", "今天").to_owned()
        } else {
            snapshot.today.seven_day_dates[index]
                .get(5..)
                .unwrap_or(&snapshot.today.seven_day_dates[index])
                .to_owned()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {label:<5}  "), Style::default().fg(colors.muted)),
            Span::styled(
                report_bar(seconds, maximum, 24),
                Style::default().fg(colors.accent),
            ),
            Span::raw(format!("  {}", human_duration(seconds))),
        ]));
    }
    lines.extend([
        Line::from(""),
        Line::from(Span::styled(
            text(language, "TODAY BY TASK", "今日任务贡献"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
    ]);
    lines.extend(today_task_lines(snapshot, colors, language));
    lines
}

fn today_task_lines(snapshot: &Snapshot, colors: Colors, language: Language) -> Vec<Line<'static>> {
    if snapshot.today.task_focus.is_empty() {
        return vec![Line::from(text(
            language,
            "  No Focus time recorded today.",
            "  今天尚未记录专注时间。",
        ))];
    }
    let maximum = snapshot.today.task_focus[0].focus_seconds;
    snapshot
        .today
        .task_focus
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let title = task
                .task_title
                .as_deref()
                .unwrap_or(text(language, "No Task", "无任务"));
            Line::from(vec![
                Span::styled(
                    format!("  {:02}  ", index + 1),
                    Style::default().fg(colors.muted),
                ),
                Span::styled(
                    format!("{:<24}", truncate(title, 24)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    report_bar(task.focus_seconds, maximum, 12),
                    Style::default().fg(colors.good),
                ),
                Span::raw(format!("  {}", human_duration(task.focus_seconds))),
            ])
        })
        .collect()
}

fn report_bar(value: u64, maximum: u64, width: usize) -> String {
    if maximum == 0 {
        return "·".repeat(width);
    }
    let filled = usize::try_from(value.saturating_mul(width as u64) / maximum)
        .unwrap_or(width)
        .min(width);
    format!("{}{}", "█".repeat(filled), "·".repeat(width - filled))
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

#[allow(clippy::too_many_lines)]
fn review_view(
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
                "Review is waiting for the Timer Service.",
                "复盘正在等待计时服务。",
            ))
            .block(panel(text(language, "REVIEW", "复盘"), colors)),
            area,
        );
        return;
    };
    let mut tasks = std::collections::BTreeMap::<String, u64>::new();
    let mut completed = 0_usize;
    let mut stopped = 0_usize;
    let mut skipped = 0_usize;
    let mut focus = 0_u64;
    let mut breaks = 0_u64;
    for record in &snapshot.recent_history {
        match record.outcome.as_str() {
            "Completed" => completed += 1,
            "Stopped" => stopped += 1,
            "Skipped" => skipped += 1,
            _ => {}
        }
        if record.kind == SessionKind::Focus {
            focus = focus.saturating_add(record.actual_seconds);
            let task = record
                .task_title
                .clone()
                .unwrap_or_else(|| text(language, "No Task", "无任务").into());
            let total = tasks.entry(task).or_default();
            *total = total.saturating_add(record.actual_seconds);
        } else {
            breaks = breaks.saturating_add(record.actual_seconds);
        }
    }
    let mut ranked = tasks.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let maximum = ranked.first().map_or(1, |item| item.1.max(1));
    let rhythm = snapshot
        .recent_history
        .iter()
        .rev()
        .take(40)
        .map(|record| match record.kind {
            SessionKind::Focus => '■',
            SessionKind::ShortBreak => '·',
            SessionKind::LongBreak => '◆',
        })
        .collect::<String>();
    let mut lines = vec![
        Line::from(Span::styled(
            text(language, "WHAT YOU DID", "做了什么"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{} {}  ·  {} {}",
            human_duration(focus),
            text(language, "focus", "专注"),
            human_duration(breaks),
            text(language, "breaks", "休息")
        )),
        Line::from(vec![
            Span::styled(
                trend(snapshot.today.seven_day_focus_seconds),
                Style::default().fg(colors.gold),
            ),
            Span::raw(format!(
                "  {} {}",
                text(language, "7-day · avg", "7 天 · 平均"),
                human_duration(snapshot.today.average_focus_seconds)
            )),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            text(language, "FOCUS BY TASK", "各任务专注"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    if ranked.is_empty() {
        lines.push(Line::from(text(
            language,
            "No Focus Sessions yet",
            "尚无专注时段",
        )));
    } else {
        lines.extend(ranked.into_iter().take(6).map(|(task, seconds)| {
            let bars = usize::try_from(seconds.saturating_mul(18) / maximum)
                .unwrap_or(18)
                .max(1);
            Line::from(format!(
                "  {} {}  {}",
                "█".repeat(bars),
                human_duration(seconds),
                task
            ))
        }));
    }
    lines.extend([
        Line::from(""),
        Line::from(Span::styled(
            text(language, "SESSION OUTCOMES", "时段结果"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{} {completed}  {} {stopped}  {} {skipped}",
            text(language, "Completed", "完成"),
            text(language, "Stopped", "停止"),
            text(language, "Skipped", "跳过")
        )),
        Line::from(""),
        Line::from(Span::styled(
            text(language, "FOCUS / BREAK RHYTHM", "专注 / 休息节奏"),
            Style::default()
                .fg(colors.gold)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(if rhythm.is_empty() {
            text(language, "No Sessions yet", "尚无时段").into()
        } else {
            rhythm
        }),
        Line::from(Span::styled(
            text(
                language,
                "■ Focus  · Short Break  ◆ Long Break",
                "■ 专注  · 短休息  ◆ 长休息",
            ),
            Style::default().fg(colors.muted),
        )),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(text(language, "HISTORY REVIEW", "历史复盘"), colors))
            .wrap(Wrap { trim: false }),
        area,
    );
}

#[allow(clippy::too_many_arguments)]
fn history_view(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: Option<&Snapshot>,
    colors: Colors,
    language: Language,
    offset: usize,
    cursor: usize,
    visual_anchor: Option<usize>,
    marked: &std::collections::BTreeSet<u64>,
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
    let compact = area.width < 72;
    let visible = history_visible_records(area, compact);
    let lines = history_view_lines(
        snapshot,
        colors,
        language,
        compact,
        offset,
        visible,
        cursor,
        visual_anchor,
        marked,
    );
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn history_view_lines(
    snapshot: &Snapshot,
    colors: Colors,
    language: Language,
    compact: bool,
    offset: usize,
    visible: usize,
    cursor: usize,
    visual_anchor: Option<usize>,
    marked: &std::collections::BTreeSet<u64>,
) -> Vec<Line<'static>> {
    let total = snapshot.recent_history.len();
    let first = offset.saturating_add(1).min(total);
    let last = offset.saturating_add(visible).min(total);
    let selected_count = if marked.is_empty() {
        visual_anchor.map_or(1, |anchor| anchor.abs_diff(cursor).saturating_add(1))
    } else {
        marked.len()
    };
    let heading = if !marked.is_empty() {
        format!(
            "{} · {} {selected_count} · {}",
            text(language, "PAST", "过去"),
            text(language, "MARKED", "已选"),
            text(language, "D delete", "D 删除")
        )
    } else if visual_anchor.is_some() {
        format!(
            "{} · {} {selected_count} · {}",
            text(language, "PAST", "过去"),
            text(language, "VISUAL", "多选"),
            text(language, "D delete", "D 删除")
        )
    } else if total > visible {
        format!(
            "{} · {first}–{last} / {total} · {}",
            text(language, "PAST", "过去"),
            text(language, "↑↓ scroll", "↑↓ 滚动")
        )
    } else {
        text(language, "PAST", "过去").into()
    };
    let mut lines = vec![Line::from(Span::styled(
        heading,
        Style::default().fg(colors.muted),
    ))];
    if snapshot.recent_history.is_empty() {
        lines.push(Line::from(text(
            language,
            "  No completed Sessions yet",
            "  尚无已完成时段",
        )));
    } else {
        if !compact {
            lines.push(Line::from(Span::styled(
                text(
                    language,
                    "  TYPE        RESULT      TASK                            TIME",
                    "  类型        结果        任务                            时长",
                ),
                Style::default().fg(colors.muted),
            )));
        }
        lines.extend(
            snapshot
                .recent_history
                .iter()
                .skip(offset)
                .take(visible)
                .enumerate()
                .flat_map(|(relative, item)| {
                    let index = offset + relative;
                    let selected = visual_anchor.is_some_and(|anchor| {
                        (anchor.min(cursor)..=anchor.max(cursor)).contains(&index)
                    });
                    history_lines(
                        item,
                        colors,
                        language,
                        compact,
                        index == cursor,
                        selected || marked.contains(&item.id),
                    )
                }),
        );
    }
    lines.extend([
        Line::from(""),
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
    lines
}

fn history_visible_records(area: Rect, compact: bool) -> usize {
    let content_height = usize::from(area.height.saturating_sub(4));
    let fixed_lines = if compact { 6 } else { 7 };
    let lines_per_record = if compact { 2 } else { 1 };
    content_height
        .saturating_sub(fixed_lines)
        .checked_div(lines_per_record)
        .unwrap_or(0)
        .max(1)
}

fn history_lines(
    item: &pomotui_protocol::RecentSessionSummary,
    colors: Colors,
    language: Language,
    compact: bool,
    cursor: bool,
    marked: bool,
) -> Vec<Line<'static>> {
    let task = match (&item.kind, item.task_title.as_deref()) {
        (SessionKind::Focus, Some(title)) => title.to_owned(),
        (SessionKind::Focus, None) => text(language, "No Task", "无任务").into(),
        _ => text(language, "Break · no Task", "休息 · 无任务").into(),
    };
    let kind = kind_label(&item.kind, language);
    let outcome = outcome_label(&item.outcome, language);
    let duration = human_duration_precise(item.actual_seconds);
    let indicator = if cursor {
        "› "
    } else if marked {
        "✓ "
    } else {
        "  "
    };
    let cursor_text = contrasting_text(colors.accent);
    let cursor_style = if cursor {
        Style::default()
            .fg(cursor_text)
            .bg(colors.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    if compact {
        vec![
            Line::from(vec![
                Span::styled(
                    format!("{indicator}{}", pad_display(kind, 10)),
                    Style::default()
                        .fg(if cursor {
                            cursor_text
                        } else {
                            session_color_for_kind(&item.kind, colors)
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("{}  {duration}", pad_display(outcome, 10))),
            ])
            .style(cursor_style),
            Line::from(format!("    {task}")).style(cursor_style),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled(
                    format!("{indicator}{}", pad_display(kind, 10)),
                    Style::default()
                        .fg(if cursor {
                            cursor_text
                        } else {
                            session_color_for_kind(&item.kind, colors)
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "{}{}{}",
                    pad_display(outcome, 12),
                    pad_display(&task, 32),
                    duration
                )),
            ])
            .style(cursor_style),
        ]
    }
}

fn contrasting_text(background: Color) -> Color {
    match background {
        Color::Rgb(red, green, blue) => {
            let luminance = u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114;
            if luminance >= 150_000 {
                Color::Black
            } else {
                Color::White
            }
        }
        Color::White
        | Color::Gray
        | Color::Yellow
        | Color::LightYellow
        | Color::LightGreen
        | Color::LightCyan => Color::Black,
        _ => Color::White,
    }
}

fn session_color_for_kind(kind: &SessionKind, colors: Colors) -> Color {
    if *kind == SessionKind::Focus {
        colors.accent
    } else {
        colors.good
    }
}

fn outcome_label(outcome: &str, language: Language) -> &'static str {
    match outcome {
        "Completed" => text(language, "Completed", "已完成"),
        "Stopped" => text(language, "Stopped", "已停止"),
        "Skipped" => text(language, "Skipped", "已跳过"),
        _ => text(language, "Recorded", "已记录"),
    }
}

fn human_duration_precise(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        clock(seconds)
    }
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
        View::Review => text(app.language, "REVIEW", "复盘"),
        View::History => text(app.language, "HISTORY", "历史"),
    };
    let keys = if app.view == View::History {
        text(
            app.language,
            "j/k move · Space mark · v range · gg/G ends · u/d page · D delete",
            "j/k 移动 · Space 勾选 · v 连选 · gg/G 首尾 · u/d 翻页 · D 删除",
        )
    } else if app.narrow {
        text(
            app.language,
            "h/l views · j/k Tasks · Space toggle · : commands · ? help",
            "h/l 视图 · j/k 任务 · Space 切换 · : 命令 · ? 帮助",
        )
    } else {
        text(
            app.language,
            "h/l or ←/→ views  j/k Tasks  Enter select  Space start/toggle  n new  : commands  ? help  q quit",
            "h/l 或 ←/→ 切换视图  j/k 任务  Enter 选择  Space 开始/切换  n 新建  : 命令  ? 帮助  q 退出",
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
        Overlay::CreateTask
        | Overlay::RenameTask
        | Overlay::ConfirmDelete
        | Overlay::ConfirmTaskSwitch
        | Overlay::ConfirmHistoryDelete => 7.min(area.height.saturating_sub(2)),
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
        Overlay::ConfirmTaskSwitch => confirm_task_switch_overlay(frame, modal, app, colors),
        Overlay::ConfirmHistoryDelete => confirm_history_delete_overlay(frame, modal, app, colors),
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
            Line::from("  Enter  确认所选任务          Space  开始/暂停/继续"),
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
            Line::from("  h/l or ←/→  switch Dashboard, Today, Review, History"),
            Line::from("  j/k move  gg/G ends  u/d page  Space mark  v range  q quit"),
            Line::from(""),
            Line::from(Span::styled(
                "SESSIONS",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  Enter  confirm selected Task             Space  start/pause/resume"),
            Line::from("  X      stop Current Session              K      skip Current Session"),
            Line::from(""),
            Line::from(Span::styled(
                "TASKS",
                Style::default()
                    .fg(colors.gold)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  n  Create task     r  Rename task       c  Complete/reopen task"),
            Line::from("  D  Delete task / selected History (confirmation required)"),
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
    let theme = app.theme.name();
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
        Line::from(settings_row(text(app.language, "Theme", "主题"), theme)),
        Line::from(settings_row(
            text(app.language, "t or ←/→", "t 或 ←/→"),
            text(app.language, "preview theme", "预览主题"),
        )),
        Line::from(settings_row(
            text(app.language, "Language", "语言"),
            language,
        )),
        Line::from(settings_row(
            "g",
            text(app.language, "switch and save language", "切换并保存语言"),
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

fn confirm_task_switch_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let task = app
        .selected_task()
        .map_or(text(app.language, "selected Task", "所选任务"), |task| {
            task.title.as_str()
        });
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{} “{task}”？",
                text(
                    app.language,
                    "Stop this Session and switch to",
                    "停止当前时段并切换到"
                )
            )),
            Line::from(text(
                app.language,
                "Elapsed focus time will remain in Session History.",
                "已经专注的时间会保留在时段历史中。",
            )),
            Line::from(Span::styled(
                text(
                    app.language,
                    "y / Enter switch · n / Esc cancel",
                    "y / Enter 切换 · n / Esc 取消",
                ),
                Style::default().fg(colors.muted),
            )),
        ])
        .block(
            panel(
                text(app.language, "CONFIRM TASK SWITCH", "确认切换任务"),
                colors,
            )
            .border_style(Style::default().fg(colors.accent)),
        ),
        area,
    );
}

fn confirm_history_delete_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, colors: Colors) {
    let count = app.history_ids_for_action().len();
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{} {count} {}？",
                text(app.language, "Delete", "删除"),
                text(app.language, "Session History entries", "条时段历史记录")
            )),
            Line::from(text(
                app.language,
                "Review totals and charts will be recalculated.",
                "复盘总计和图表将重新计算。",
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
            panel(text(app.language, "DELETE HISTORY", "删除历史"), colors)
                .border_style(Style::default().fg(colors.accent)),
        ),
        area,
    );
}

fn colors(theme: Theme, overrides: ColorOverrides) -> Colors {
    let base = match theme {
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
        Theme::RanPaperLight => Colors {
            background: Color::Rgb(232, 223, 201),
            surface: Color::Rgb(198, 187, 165),
            text: Color::Rgb(33, 31, 26),
            muted: Color::Rgb(112, 103, 91),
            accent: Color::Rgb(166, 35, 31),
            gold: Color::Rgb(184, 135, 34),
            good: Color::Rgb(49, 92, 120),
            border: Color::Rgb(166, 154, 132),
        },
        Theme::RanPaperDark => Colors {
            background: Color::Rgb(17, 16, 15),
            surface: Color::Rgb(37, 33, 28),
            text: Color::Rgb(221, 210, 185),
            muted: Color::Rgb(156, 149, 136),
            accent: Color::Rgb(214, 74, 60),
            gold: Color::Rgb(216, 173, 67),
            good: Color::Rgb(101, 145, 173),
            border: Color::Rgb(86, 78, 67),
        },
    };
    Colors {
        background: overrides.background.unwrap_or(base.background),
        surface: overrides.surface.unwrap_or(base.surface),
        text: overrides.text.unwrap_or(base.text),
        muted: overrides.muted.unwrap_or(base.muted),
        accent: overrides.accent.unwrap_or(base.accent),
        gold: overrides.gold.unwrap_or(base.gold),
        good: overrides.good.unwrap_or(base.good),
        border: overrides.border.unwrap_or(base.border),
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
    if UnicodeWidthStr::width(value) <= maximum {
        return value.to_owned();
    }
    if maximum <= 1 {
        return "…".repeat(maximum);
    }
    let mut output = String::new();
    let content_width = maximum - 1;
    for character in value.chars() {
        let next_width = character.width().unwrap_or(0);
        if UnicodeWidthStr::width(output.as_str()) + next_width > content_width {
            break;
        }
        output.push(character);
    }
    output.push('…');
    output
}

fn pad_display(value: &str, width: usize) -> String {
    let value = truncate(value, width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(value.as_str()));
    format!("{value}{}", " ".repeat(padding))
}

fn settings_row(label: &str, value: &str) -> String {
    format!("  {}{value}", pad_display(label, 28))
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
            durable_health: pomotui_protocol::DurableHealth {
                state: pomotui_protocol::DurableHealthState::Healthy,
                last_successful_commit: None,
                error: None,
            },
            tasks: vec![pomotui_protocol::TaskSummary {
                id: 1,
                title: "Ship Pomotui".into(),
                completed: false,
                focus_seconds: 1_500,
            }],
            today: Box::new(pomotui_protocol::TodaySummary {
                focus_seconds: 3_000,
                completed_rounds: 2,
                seven_day_focus_seconds: [0, 600, 1_200, 900, 1_500, 2_400, 3_000],
                seven_day_dates: [
                    "2026-07-21".into(),
                    "2026-07-22".into(),
                    "2026-07-23".into(),
                    "2026-07-24".into(),
                    "2026-07-25".into(),
                    "2026-07-26".into(),
                    "2026-07-27".into(),
                ],
                average_focus_seconds: 1_371,
                task_focus: vec![
                    pomotui_protocol::TaskFocusSummary {
                        task_title: Some("Ship Pomotui".into()),
                        focus_seconds: 2_400,
                    },
                    pomotui_protocol::TaskFocusSummary {
                        task_title: None,
                        focus_seconds: 600,
                    },
                ],
            }),
            recent_history: vec![
                pomotui_protocol::RecentSessionSummary {
                    id: 1,
                    kind: SessionKind::Focus,
                    outcome: "Completed".into(),
                    actual_seconds: 1_500,
                    task_title: Some("Ship Pomotui".into()),
                },
                pomotui_protocol::RecentSessionSummary {
                    id: 1,
                    kind: SessionKind::ShortBreak,
                    outcome: "Completed".into(),
                    actual_seconds: 300,
                    task_title: None,
                },
            ],
        }
    }

    #[test]
    fn wide_and_narrow_dashboard_render_all_session_states_and_themes() {
        for (width, height) in [(100, 32), (60, 24)] {
            for theme in [
                Theme::VermilionPaperLight,
                Theme::VermilionPaperDark,
                Theme::RanPaperLight,
                Theme::RanPaperDark,
            ] {
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
    fn history_shows_task_attribution_no_task_and_break_records() {
        for (language, expected) in [
            (
                Language::English,
                ["Ship Pomotui", "No Task", "Break · no Task"],
            ),
            (Language::SimplifiedChinese, ["Ship Pomotui", "无", "休"]),
        ] {
            let mut value = snapshot("pending", SessionKind::Focus);
            value.recent_history.insert(
                1,
                pomotui_protocol::RecentSessionSummary {
                    id: 1,
                    kind: SessionKind::Focus,
                    outcome: "Stopped".into(),
                    actual_seconds: 42,
                    task_title: None,
                },
            );
            let mut app = App::new(Some(value), Theme::VermilionPaperDark);
            app.language = language;
            app.view = View::History;
            let mut terminal = Terminal::new(TestBackend::new(100, 32)).expect("terminal");
            terminal
                .draw(|frame| render(frame, &mut app))
                .expect("history");
            let rendered = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>();
            for label in expected {
                assert!(rendered.contains(label), "missing {label}: {rendered}");
            }
        }
    }

    fn symbol_column(terminal: &Terminal<TestBackend>, symbol: &str) -> Option<u16> {
        let area = terminal.backend().buffer().area;
        (0..area.height).find_map(|y| {
            (0..area.width).find(|&x| {
                (x..area.width)
                    .map(|cell_x| terminal.backend().buffer()[(cell_x, y)].symbol())
                    .collect::<String>()
                    .starts_with(symbol)
            })
        })
    }

    fn symbol_columns(terminal: &Terminal<TestBackend>, symbol: &str) -> Vec<u16> {
        let area = terminal.backend().buffer().area;
        (0..area.height)
            .flat_map(|y| {
                (0..area.width).filter(move |&x| {
                    (x..area.width)
                        .map(|cell_x| terminal.backend().buffer()[(cell_x, y)].symbol())
                        .collect::<String>()
                        .starts_with(symbol)
                })
            })
            .collect()
    }

    #[test]
    fn wide_character_task_titles_keep_times_in_one_column() {
        let mut value = snapshot("pending", SessionKind::Focus);
        value.tasks = vec![
            pomotui_protocol::TaskSummary {
                id: 1,
                title: "cnn".into(),
                completed: false,
                focus_seconds: 13_500,
            },
            pomotui_protocol::TaskSummary {
                id: 2,
                title: "写日记".into(),
                completed: false,
                focus_seconds: 0,
            },
        ];
        let mut app = App::new(Some(value), Theme::VermilionPaperDark);
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("dashboard");

        let first = symbol_column(&terminal, "225:00");
        let second = symbol_column(&terminal, "00:00");
        assert!(first.is_some() && second.is_some());
        assert_eq!(first, second);
    }

    #[test]
    fn simplified_chinese_settings_values_share_a_column() {
        let mut app = App::new(
            Some(snapshot("pending", SessionKind::Focus)),
            Theme::VermilionPaperLight,
        );
        app.language = Language::SimplifiedChinese;
        app.overlay = Overlay::Settings;
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("settings");

        let language_column = symbol_column(&terminal, "简").expect("language value");
        assert!(symbol_columns(&terminal, "V").contains(&language_column));
    }

    #[test]
    fn simplified_chinese_history_uses_stable_columns() {
        let mut value = snapshot("pending", SessionKind::Focus);
        value.recent_history = vec![
            pomotui_protocol::RecentSessionSummary {
                id: 1,
                kind: SessionKind::Focus,
                outcome: "Stopped".into(),
                actual_seconds: 3,
                task_title: Some("写日记".into()),
            },
            pomotui_protocol::RecentSessionSummary {
                id: 1,
                kind: SessionKind::ShortBreak,
                outcome: "Skipped".into(),
                actual_seconds: 0,
                task_title: None,
            },
        ];
        let mut app = App::new(Some(value), Theme::VermilionPaperDark);
        app.language = Language::SimplifiedChinese;
        app.view = View::History;
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("history");

        assert_eq!(
            symbol_column(&terminal, "停"),
            symbol_column(&terminal, "跳")
        );
        let first = symbol_column(&terminal, "3s");
        let second = symbol_column(&terminal, "0s");
        assert!(first.is_some() && second.is_some());
        assert_eq!(first, second);
    }

    #[test]
    fn selecting_a_pending_task_rebinds_without_starting() {
        let mut value = snapshot("pending", SessionKind::Focus);
        value.tasks.push(pomotui_protocol::TaskSummary {
            id: 2,
            title: "Write journal".into(),
            completed: false,
            focus_seconds: 0,
        });
        let mut app = App::new(Some(value), Theme::VermilionPaperDark);

        assert_eq!(app.handle_key(InputKey::Down), None);
        assert_eq!(app.selected_task, 1);
        assert_eq!(
            app.handle_key(InputKey::Enter),
            Some(Action::Command(pomotui_protocol::Command::TaskSelect {
                id: 2,
                stop_current: false,
            }))
        );
        assert_eq!(
            app.handle_key(InputKey::Char(' ')),
            Some(Action::Command(pomotui_protocol::Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(2),
            }))
        );
    }

    #[test]
    fn history_navigation_scrolls_to_older_records() {
        let mut value = snapshot("pending", SessionKind::Focus);
        value.recent_history = (0..10)
            .map(|index| pomotui_protocol::RecentSessionSummary {
                id: 1,
                kind: SessionKind::Focus,
                outcome: "Stopped".into(),
                actual_seconds: index,
                task_title: Some(format!("History Task {index}")),
            })
            .collect();
        let mut app = App::new(Some(value), Theme::VermilionPaperDark);
        app.view = View::History;
        let mut terminal = Terminal::new(TestBackend::new(60, 18)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("initial history");
        let initial = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(initial.contains("History Task 0"));

        for _ in 0..5 {
            assert_eq!(app.handle_key(InputKey::Down), None);
        }
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("scrolled history");
        let scrolled = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(!scrolled.contains("History Task 0"));
        assert!(scrolled.contains("History Task 5"));
        assert_eq!(app.selected_task, 0);
    }

    #[test]
    fn active_task_switch_and_visual_history_delete_require_confirmation() {
        let mut value = snapshot("running", SessionKind::Focus);
        value.tasks.push(pomotui_protocol::TaskSummary {
            id: 2,
            title: "Next".into(),
            completed: false,
            focus_seconds: 0,
        });
        value.recent_history = (1..=8)
            .map(|id| pomotui_protocol::RecentSessionSummary {
                id,
                kind: SessionKind::Focus,
                outcome: "Stopped".into(),
                actual_seconds: id,
                task_title: Some(format!("Task {id}")),
            })
            .collect();
        let mut app = App::new(Some(value), Theme::VermilionPaperDark);
        app.handle_key(InputKey::Down);
        assert_eq!(app.handle_key(InputKey::Enter), None);
        assert_eq!(app.overlay, Overlay::ConfirmTaskSwitch);
        assert_eq!(
            app.handle_key(InputKey::Enter),
            Some(Action::Command(pomotui_protocol::Command::TaskSelect {
                id: 2,
                stop_current: true,
            }))
        );

        app.view = View::History;
        app.handle_key(InputKey::Char('G'));
        assert_eq!(app.history_cursor, 7);
        app.handle_key(InputKey::Char('g'));
        app.handle_key(InputKey::Char('g'));
        assert_eq!(app.history_cursor, 0);
        app.handle_key(InputKey::Char('v'));
        app.handle_key(InputKey::Char('d'));
        assert_eq!(app.history_cursor, 5);
        app.handle_key(InputKey::Char('D'));
        assert_eq!(app.overlay, Overlay::ConfirmHistoryDelete);
        assert_eq!(
            app.handle_key(InputKey::Enter),
            Some(Action::Command(pomotui_protocol::Command::HistoryDelete {
                ids: vec![1, 2, 3, 4, 5, 6],
            }))
        );
    }

    #[test]
    fn review_renders_history_derived_charts() {
        let mut value = snapshot("pending", SessionKind::Focus);
        value.recent_history = vec![
            pomotui_protocol::RecentSessionSummary {
                id: 1,
                kind: SessionKind::Focus,
                outcome: "Completed".into(),
                actual_seconds: 1_500,
                task_title: Some("Write".into()),
            },
            pomotui_protocol::RecentSessionSummary {
                id: 2,
                kind: SessionKind::ShortBreak,
                outcome: "Completed".into(),
                actual_seconds: 300,
                task_title: None,
            },
        ];
        let mut app = App::new(Some(value), Theme::VermilionPaperDark);
        app.view = View::Review;
        let mut terminal = Terminal::new(TestBackend::new(90, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("review");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(rendered.contains("FOCUS BY TASK"));
        assert!(rendered.contains("SESSION OUTCOMES"));
        assert!(rendered.contains("Write"));
    }

    #[test]
    fn history_cursor_and_space_marks_support_disjoint_deletion() {
        let mut value = snapshot("pending", SessionKind::Focus);
        value.recent_history = (1..=5)
            .map(|id| pomotui_protocol::RecentSessionSummary {
                id,
                kind: SessionKind::Focus,
                outcome: "Stopped".into(),
                actual_seconds: id,
                task_title: Some(format!("Record {id}")),
            })
            .collect();
        let mut app = App::new(Some(value), Theme::VermilionPaperDark);
        app.view = View::History;

        app.handle_key(InputKey::Char(' '));
        app.handle_key(InputKey::Down);
        app.handle_key(InputKey::Down);
        app.handle_key(InputKey::AltSpace);
        assert_eq!(
            app.marked_history.iter().copied().collect::<Vec<_>>(),
            [1, 3]
        );

        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("history");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(rendered.contains('›'));
        assert!(rendered.contains('✓'));

        app.handle_key(InputKey::Char('D'));
        assert_eq!(app.overlay, Overlay::ConfirmHistoryDelete);
        assert_eq!(
            app.handle_key(InputKey::Enter),
            Some(Action::Command(pomotui_protocol::Command::HistoryDelete {
                ids: vec![1, 3]
            }))
        );
    }

    #[test]
    fn history_cursor_text_contrasts_with_every_theme_and_custom_accent() {
        let cases = [
            (Theme::VermilionPaperLight, None),
            (Theme::VermilionPaperDark, None),
            (Theme::RanPaperLight, None),
            (Theme::RanPaperDark, None),
            (Theme::RanPaperLight, Some(Color::Rgb(245, 220, 90))),
            (Theme::RanPaperDark, Some(Color::Rgb(20, 30, 40))),
        ];
        for (theme, accent) in cases {
            let mut app = App::new(Some(snapshot("pending", SessionKind::Focus)), theme);
            app.view = View::History;
            app.color_overrides.accent = accent;
            let palette = colors(theme, app.color_overrides);
            let expected = contrasting_text(palette.accent);
            let mut terminal = Terminal::new(TestBackend::new(100, 28)).expect("terminal");
            terminal
                .draw(|frame| render(frame, &mut app))
                .expect("history");

            let cursor_text = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .filter(|cell| cell.bg == palette.accent && !cell.symbol().trim().is_empty())
                .collect::<Vec<_>>();
            assert!(!cursor_text.is_empty(), "{theme:?}");
            assert!(
                cursor_text.iter().all(|cell| cell.fg == expected),
                "{theme:?}"
            );
        }
    }

    #[test]
    fn today_report_has_metrics_daily_values_and_task_contributions() {
        for (width, language, expected) in [
            (
                100,
                Language::English,
                ["TODAY AT A GLANCE", "7-DAY FOCUS", "TODAY BY TASK"],
            ),
            (60, Language::SimplifiedChinese, ["概", "天", "任"]),
        ] {
            let mut app = App::new(
                Some(snapshot("pending", SessionKind::Focus)),
                Theme::VermilionPaperDark,
            );
            app.language = language;
            app.view = View::Today;
            let mut terminal = Terminal::new(TestBackend::new(width, 32)).expect("terminal");
            terminal
                .draw(|frame| render(frame, &mut app))
                .expect("today");
            let rendered = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>();
            for label in expected {
                assert!(rendered.contains(label), "missing {label}: {rendered}");
            }
            assert!(rendered.contains("07-21"));
            assert!(rendered.contains("Ship Pomotui"));
            assert!(rendered.contains("50m"));
        }
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
