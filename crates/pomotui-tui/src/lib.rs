pub mod animation;
pub mod config;

use pomotui_protocol::{SessionKind, Snapshot};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    VermilionPaperLight,
    VermilionPaperDark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Dashboard,
    Today,
    History,
    Settings,
    Palette,
    Help,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Skip,
    ToggleSession,
    Stop,
}

pub struct App {
    pub snapshot: Option<Snapshot>,
    pub theme: Theme,
    pub view: View,
    pub selected_task: usize,
    pub narrow: bool,
    pub warning: Option<String>,
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
            view: View::Dashboard,
            selected_task: 0,
            narrow: false,
            warning: None,
            completion: None,
        }
    }

    pub fn key(&mut self, key: char) -> Option<Action> {
        if self.view == View::Palette {
            return match key {
                '1' => Some(Action::ToggleSession),
                '2' => Some(Action::Stop),
                '3' => Some(Action::Skip),
                _ => None,
            };
        }
        if self.view == View::Settings && key == 't' {
            self.theme = match self.theme {
                Theme::VermilionPaperLight => Theme::VermilionPaperDark,
                Theme::VermilionPaperDark => Theme::VermilionPaperLight,
            };
            return None;
        }
        match key {
            'j' | '↓' => {
                let last = self
                    .snapshot
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.tasks.len().saturating_sub(1));
                self.selected_task = self.selected_task.saturating_add(1).min(last);
            }
            'k' | '↑' => self.selected_task = self.selected_task.saturating_sub(1),
            'h' => self.view = previous_view(self.view),
            'l' => self.view = next_view(self.view),
            ':' => self.view = View::Palette,
            '?' => self.view = View::Help,
            's' => self.view = View::Settings,
            'K' => return Some(Action::Skip),
            ' ' => return Some(Action::ToggleSession),
            'X' => return Some(Action::Stop),
            _ => {}
        }
        None
    }

    pub fn mouse_click(&mut self, _x: u16, y: u16) -> Option<Action> {
        if y == 0 {
            self.view = View::Dashboard;
            None
        } else {
            Some(Action::ToggleSession)
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

const fn next_view(view: View) -> View {
    match view {
        View::Dashboard => View::Today,
        View::Today => View::History,
        _ => View::Dashboard,
    }
}

const fn previous_view(view: View) -> View {
    match view {
        View::Dashboard => View::History,
        View::History => View::Today,
        _ => View::Dashboard,
    }
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    app.narrow = area.width < 80;
    let palette = palette(app.theme);
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.background).fg(palette.text)),
        area,
    );
    match app.view {
        View::Dashboard => dashboard(frame, area, app, palette),
        View::Today => {
            let body = today_body(app.snapshot.as_ref());
            panel(frame, area, "Today", &body, palette);
        }
        View::History => {
            let body = history_body(app.snapshot.as_ref());
            panel(frame, area, "Session History", &body, palette);
        }
        View::Settings => panel(
            frame,
            if app.narrow {
                area
            } else {
                centered(area, 70, 80)
            },
            "Settings",
            "Durations · Theme (t preview) · Keys · Reminder · Sound · Animation",
            palette,
        ),
        View::Palette => panel(
            frame,
            centered(area, 60, 60),
            "Command Palette",
            "1 Start/Pause/Resume · 2 Stop · 3 Skip",
            palette,
        ),
        View::Help => panel(
            frame,
            centered(area, 70, 70),
            "Help",
            "j/k Tasks · h/l Views · Space Toggle · X Stop · K Skip · : Commands · s Settings",
            palette,
        ),
    }
    if let Some(playback) = &app.completion {
        let (art, _) = playback.animation.frame(playback.elapsed_ms);
        panel(
            frame,
            centered(area, 50, 50),
            "Session Complete",
            art,
            palette,
        );
    }
    if let Some(warning) = &app.warning {
        frame.render_widget(
            Paragraph::new(warning.as_str()).style(Style::default().fg(Color::Rgb(201, 166, 107))),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        );
    }
}

fn dashboard(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);
    if app.narrow {
        let body = tasks_body(app.snapshot.as_ref(), app.selected_task);
        panel(frame, chunks[0], "Tasks", &body, palette);
    } else {
        let top = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[0]);
        let tasks = tasks_body(app.snapshot.as_ref(), app.selected_task);
        let today = today_body(app.snapshot.as_ref());
        panel(frame, top[0], "Tasks", &tasks, palette);
        panel(frame, top[1], "Today", &today, palette);
    }
    let body = app.snapshot.as_ref().map_or_else(
        || "RECONNECTING\nTimer Service unavailable".into(),
        |snapshot| {
            let task = snapshot.current_task.as_ref().map_or_else(
                || "Task: none".into(),
                |title| {
                    let seconds = snapshot
                        .tasks
                        .iter()
                        .find(|task| Some(task.id) == snapshot.current_task_id)
                        .map_or(0, |task| task.focus_seconds);
                    format!(
                        "Task: {title} · attributed {:02}:{:02}",
                        seconds / 60,
                        seconds % 60
                    )
                },
            );
            format!(
                "{}\n{:02}:{:02}\n{}\n{:?} · round {}/{}\nNext: {:?}",
                snapshot.state.to_uppercase(),
                snapshot.remaining_seconds / 60,
                snapshot.remaining_seconds % 60,
                task,
                snapshot.kind,
                snapshot.completed_rounds,
                snapshot.rounds_per_cycle,
                snapshot.next_kind
            )
        },
    );
    panel(
        frame,
        chunks[1],
        "Current Session",
        &body,
        session_palette(app.snapshot.as_ref(), palette),
    );
    frame.render_widget(
        Paragraph::new(Line::from(
            "Space toggle · X stop · K skip · h/l views · : commands · ? help · mouse",
        )),
        chunks[2],
    );
}

#[derive(Clone, Copy)]
struct Palette {
    background: Color,
    text: Color,
    accent: Color,
}

fn session_palette(snapshot: Option<&Snapshot>, mut palette: Palette) -> Palette {
    if let Some(snapshot) = snapshot {
        palette.accent = if matches!(snapshot.state.as_str(), "paused" | "pending") {
            Color::Rgb(201, 166, 107)
        } else if snapshot.kind != SessionKind::Focus {
            Color::Rgb(85, 139, 99)
        } else {
            palette.accent
        };
    }
    palette
}

fn tasks_body(snapshot: Option<&Snapshot>, selected: usize) -> String {
    let Some(snapshot) = snapshot else {
        return "Waiting for Timer Service".into();
    };
    if snapshot.tasks.is_empty() {
        return "No Tasks\nj/k to select".into();
    }
    snapshot
        .tasks
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, task)| {
            format!(
                "{} {} {} · {:02}:{:02}",
                if index == selected { ">" } else { " " },
                if task.completed { "✓" } else { "•" },
                task.title,
                task.focus_seconds / 60,
                task.focus_seconds % 60
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn today_body(snapshot: Option<&Snapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "Waiting for Timer Service".into();
    };
    format!(
        "Focus {:02}:{:02} · Rounds {}\n7d {} · avg {:02}:{:02}",
        snapshot.today.focus_seconds / 60,
        snapshot.today.focus_seconds % 60,
        snapshot.today.completed_rounds,
        trend(snapshot.today.seven_day_focus_seconds),
        snapshot.today.average_focus_seconds / 60,
        snapshot.today.average_focus_seconds % 60
    )
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

fn history_body(snapshot: Option<&Snapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "CURRENT · reconnecting".into();
    };
    format!(
        "PAST\n{}\nCURRENT · {} {:?}\nNEXT · {:?}",
        snapshot.recent_history.join("\n"),
        snapshot.state,
        snapshot.kind,
        snapshot.next_kind
    )
}

const fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::VermilionPaperLight => Palette {
            background: Color::Rgb(247, 243, 235),
            text: Color::Rgb(33, 29, 26),
            accent: Color::Rgb(159, 45, 36),
        },
        Theme::VermilionPaperDark => Palette {
            background: Color::Rgb(17, 16, 15),
            text: Color::Rgb(247, 243, 235),
            accent: Color::Rgb(214, 107, 95),
        },
    }
}

fn panel(frame: &mut Frame<'_>, area: Rect, title: &str, body: &str, palette: Palette) {
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.accent)),
        ),
        area,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - height) / 2),
        Constraint::Percentage(height),
        Constraint::Min(0),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - width) / 2),
        Constraint::Percentage(width),
        Constraint::Min(0),
    ])
    .split(vertical[1])[1]
}

#[must_use]
pub const fn kind_label(kind: &SessionKind) -> &'static str {
    match kind {
        SessionKind::Focus => "Focus",
        SessionKind::ShortBreak => "Short Break",
        SessionKind::LongBreak => "Long Break",
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
                    assert!(text.contains("Current Session"));
                    assert!(text.contains(&state.to_uppercase()));
                }
            }
        }
    }

    #[test]
    fn disconnected_and_navigation_paths_are_explicit() {
        let mut app = App::new(None, Theme::VermilionPaperDark);
        assert_eq!(app.key('K'), Some(Action::Skip));
        app.key('l');
        assert_eq!(app.view, View::Today);
        app.key('s');
        assert_eq!(app.view, View::Settings);
        let original_theme = app.theme;
        app.key('t');
        assert_ne!(app.theme, original_theme);
        app.view = View::Palette;
        assert_eq!(app.key('3'), Some(Action::Skip));
        assert_eq!(app.mouse_click(2, 5), Some(Action::ToggleSession));
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
        app.view = View::Settings;
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
}
