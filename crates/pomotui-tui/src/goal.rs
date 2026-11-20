use chrono::{Datelike, Months, NaiveDate, TimeDelta};
use pomotui_protocol::{Command, DailyChainGoalSettings, DailyChainGoalSnapshot};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::{Colors, InputKey, Language, panel, text};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Field {
    Quota,
    Start,
    End,
    TimeZone,
    Disable,
}

enum EditError {
    Quota,
    Dates,
    TimeZone,
    Service(String),
}

pub(super) struct Editor {
    field: Field,
    quota: String,
    start: NaiveDate,
    today: NaiveDate,
    use_default_start: bool,
    end: Option<NaiveDate>,
    time_zone: String,
    active: bool,
    can_disable: bool,
    error: Option<EditError>,
}

impl Editor {
    pub(super) fn new(goal: &DailyChainGoalSnapshot) -> Option<Self> {
        let upcoming = goal
            .scheduled
            .as_ref()
            .and_then(|change| change.settings.as_ref());
        let settings = upcoming.or(goal.active.as_ref());
        let configured_start = settings
            .and_then(|settings| settings.start_date.as_deref())
            .and_then(parse_date);
        let start = configured_start.or_else(|| {
            let date = if settings.is_some() {
                &goal.date
            } else {
                &goal.device_local_date
            };
            parse_date(date)?.succ_opt()
        })?;
        Some(Self {
            field: Field::Quota,
            quota: settings.map_or_else(|| "1".into(), |goal| goal.quota.to_string()),
            start,
            today: parse_date(&goal.date).unwrap_or(start),
            use_default_start: configured_start.is_none(),
            end: settings
                .and_then(|settings| settings.end_date.as_deref())
                .and_then(parse_date),
            time_zone: settings.map_or_else(
                || goal.device_time_zone.clone(),
                |goal| goal.time_zone.clone(),
            ),
            active: goal.active.is_some(),
            can_disable: settings.is_some(),
            error: None,
        })
    }

    pub(super) fn key(&mut self, key: InputKey) -> Option<Command> {
        self.error = None;
        match key {
            InputKey::Tab => self.next_field(),
            InputKey::Enter if self.field == Field::Disable => {
                return Some(Command::ConfigureDailyChainGoal { settings: None });
            }
            InputKey::Enter => {
                return match self.settings() {
                    Ok(settings) => Some(Command::ConfigureDailyChainGoal {
                        settings: Some(settings),
                    }),
                    Err(error) => {
                        self.error = Some(error);
                        None
                    }
                };
            }
            _ => match self.field {
                Field::Quota => edit(&mut self.quota, key, |c| c.is_ascii_digit()),
                Field::Start | Field::End => self.edit_date(key),
                Field::TimeZone => {
                    edit(&mut self.time_zone, key, |c| {
                        c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+')
                    });
                }
                Field::Disable => {}
            },
        }
        None
    }

    pub(super) fn show_service_error(&mut self, message: String) {
        self.error = Some(EditError::Service(message));
    }

    fn next_field(&mut self) {
        self.field = match self.field {
            Field::Quota if self.active => Field::End,
            Field::Quota => Field::Start,
            Field::Start => Field::End,
            Field::End if !self.active => Field::TimeZone,
            Field::End | Field::TimeZone if self.can_disable => Field::Disable,
            Field::End | Field::TimeZone | Field::Disable => Field::Quota,
        };
    }

    fn edit_date(&mut self, key: InputKey) {
        if self.field == Field::End && key == InputKey::Backspace {
            self.end = None;
            return;
        }
        let date = self.selected_date();
        let moved = match key {
            InputKey::Char(' ') => Some(date),
            InputKey::Left | InputKey::Char('h') => date.checked_sub_signed(TimeDelta::days(1)),
            InputKey::Right | InputKey::Char('l') => date.checked_add_signed(TimeDelta::days(1)),
            InputKey::Up | InputKey::Char('k') => date.checked_sub_signed(TimeDelta::days(7)),
            InputKey::Down | InputKey::Char('j') => date.checked_add_signed(TimeDelta::days(7)),
            InputKey::Char('[') => date.checked_sub_months(Months::new(1)),
            InputKey::Char(']') => date.checked_add_months(Months::new(1)),
            _ => None,
        };
        if let Some(moved) = moved.filter(|date| (1..=9999).contains(&date.year())) {
            if self.field == Field::Start {
                self.start = moved;
                self.use_default_start = false;
            } else {
                self.end = Some(moved);
            }
        }
    }

    fn selected_date(&self) -> NaiveDate {
        if self.field == Field::Start {
            self.start
        } else {
            self.end.unwrap_or_else(|| self.start.max(self.today))
        }
    }

    fn settings(&self) -> Result<DailyChainGoalSettings, EditError> {
        let quota = self.quota.parse().map_err(|_| EditError::Quota)?;
        if quota == 0 {
            return Err(EditError::Quota);
        }
        if !self.use_default_start && self.end.is_some_and(|end| end < self.start) {
            return Err(EditError::Dates);
        }
        if self.time_zone.is_empty() {
            return Err(EditError::TimeZone);
        }
        Ok(DailyChainGoalSettings {
            quota,
            start_date: (!self.use_default_start && !self.active).then(|| self.start.to_string()),
            end_date: self.end.map(|end| end.to_string()),
            time_zone: self.time_zone.clone(),
        })
    }
}

fn parse_date(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

fn edit(value: &mut String, key: InputKey, allows: impl FnOnce(char) -> bool) {
    match key {
        InputKey::Char(c) if allows(c) => value.push(c),
        InputKey::Backspace => {
            value.pop();
        }
        InputKey::KillToStart => value.clear(),
        _ => {}
    }
}

fn fields(editor: &Editor, colors: Colors, language: Language) -> Vec<Line<'static>> {
    let row = |field, label, value| {
        let selected = editor.field == field;
        Line::from(Span::styled(
            format!("{} {label}  {value}", if selected { "›" } else { " " }),
            if selected {
                Style::default()
                    .fg(colors.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text)
            },
        ))
    };
    let start = if editor.use_default_start {
        text(
            language,
            "Tomorrow in goal time zone",
            "按目标时区从明天开始",
        )
        .into()
    } else {
        editor.start.to_string()
    };
    let mut lines = vec![
        row(
            Field::Quota,
            text(language, "Rounds per day", "每日轮次"),
            editor.quota.clone(),
        ),
        row(
            Field::Start,
            text(language, "Start date", "开始日期"),
            start,
        ),
        row(
            Field::End,
            text(language, "End date", "结束日期"),
            editor.end.map_or_else(
                || text(language, "Keep enabled", "持续启用").into(),
                |date| date.to_string(),
            ),
        ),
        row(
            Field::TimeZone,
            text(language, "Time zone", "时区"),
            editor.time_zone.clone(),
        ),
    ];
    if editor.can_disable {
        lines.push(row(
            Field::Disable,
            text(language, "Disable goal", "关闭每日保链"),
            if editor.active {
                text(language, "from tomorrow", "明天生效")
            } else {
                ""
            }
            .into(),
        ));
    }
    lines
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    editor: &Editor,
    colors: Colors,
    language: Language,
) {
    let mut lines = fields(editor, colors, language);
    if matches!(editor.field, Field::Start | Field::End) {
        let date = editor.selected_date();
        lines.push(Line::from(text(
            language,
            "Arrows move · [/] month",
            "方向键移动 · [/] 翻月",
        )));
        if editor.field == Field::End {
            lines.push(Line::from(text(
                language,
                "Space selects · Backspace keeps enabled",
                "Space 选中 · Backspace 设为持续启用",
            )));
        }
        lines.extend(calendar(date, colors, language));
    } else if editor.field == Field::TimeZone {
        lines.push(Line::from(text(
            language,
            "IANA name, e.g. Asia/Shanghai · Ctrl+U clears",
            "输入时区名称，如 Asia/Shanghai · Ctrl+U 清空",
        )));
    } else if editor.active {
        lines.push(Line::from(text(
            language,
            "Changes take effect tomorrow.",
            "修改从明天生效。",
        )));
    }
    if let Some(error) = &editor.error {
        lines.insert(
            0,
            Line::from(Span::styled(
                match error {
                    EditError::Quota => text(
                        language,
                        "Enter a positive round count.",
                        "请输入大于零的轮次。",
                    ),
                    EditError::Dates => text(
                        language,
                        "End date must be on or after start date.",
                        "结束日期须在开始日期当天或之后。",
                    ),
                    EditError::TimeZone => text(language, "Enter a time zone.", "请输入时区。"),
                    EditError::Service(message) => message,
                },
                Style::default().fg(colors.gold),
            )),
        );
    }
    let controls = if editor.field == Field::Disable {
        text(
            language,
            " Tab next · Enter disable · Esc cancel ",
            " Tab 下一项 · Enter 关闭目标 · Esc 取消 ",
        )
    } else {
        text(
            language,
            " Tab next · Enter save · Esc cancel ",
            " Tab 下一项 · Enter 保存 · Esc 取消 ",
        )
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                panel(text(language, "DAILY CHAIN GOAL", "每日保链"), colors)
                    .title_bottom(controls),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn calendar(date: NaiveDate, colors: Colors, language: Language) -> Vec<Line<'static>> {
    let first = date.with_day(1).expect("first day of the month");
    let offset = first.weekday().num_days_from_monday();
    let mut lines = vec![
        Line::from(date.format("%Y-%m").to_string()),
        Line::from(text(
            language,
            "Mo Tu We Th Fr Sa Su",
            "一 二 三 四 五 六 日",
        )),
    ];
    for week in 0_u32..6 {
        let mut row = Vec::new();
        for weekday in 0..7 {
            let cell = (week * 7 + weekday + 1).checked_sub(offset);
            let cell = cell.and_then(|day| first.with_day(day));
            let label = cell.map_or_else(|| "  ".into(), |day| format!("{:2}", day.day()));
            let style = if cell == Some(date) {
                Style::default()
                    .fg(colors.background)
                    .bg(colors.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text)
            };
            row.push(Span::styled(label, style));
            row.push(Span::raw(" "));
        }
        lines.push(Line::from(row));
    }
    lines
}

pub(super) fn status(goal: &DailyChainGoalSnapshot, language: Language) -> Option<String> {
    let current = goal
        .active
        .as_ref()
        .map(|active| format!("{}/{}", goal.achieved, active.quota));
    let upcoming = goal.scheduled.as_ref().map(|change| {
        let effective_date = change.effective_date.as_str();
        change.settings.as_ref().map_or_else(
            || match language {
                Language::English => format!("off from {effective_date}"),
                Language::SimplifiedChinese => format!("{effective_date} 起关闭"),
            },
            |settings| {
                let starts_on = settings
                    .start_date
                    .as_deref()
                    .map_or(effective_date, |date| date.max(effective_date));
                match language {
                    Language::English => format!("{}/day from {starts_on}", settings.quota),
                    Language::SimplifiedChinese => {
                        format!("{starts_on} 起每日 {} 轮", settings.quota)
                    }
                }
            },
        )
    });
    match (current, upcoming) {
        (Some(current), Some(upcoming)) => Some(format!("{current} · {upcoming}")),
        (current, upcoming) => current.or(upcoming),
    }
}

pub(super) fn status_line(
    goal: &DailyChainGoalSnapshot,
    language: Language,
) -> Option<Line<'static>> {
    status(goal, language).map(|status| {
        Line::from(format!(
            "{}  {status}",
            text(language, "Daily Chain Goal", "每日保链")
        ))
    })
}
