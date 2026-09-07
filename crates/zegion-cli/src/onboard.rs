use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use zegion_core::config::{ChannelConfig, Config, ProviderConfig};

/// Launch the ratatui onboarding wizard and write the resulting config to `path`.
pub async fn run(path: &str) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let terminal = ratatui::init();
    let result = run_app(terminal).await;
    ratatui::restore();
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;

    let cfg = result?.ok_or_else(|| anyhow::anyhow!("onboarding cancelled"))?;
    cfg.save(path).await?;
    println!("Config written to {path}. Start the agent with:  zegion run");
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Providers,
    Models,
    Channels,
    Review,
    Done,
}

const STEP_TITLES: [&str; 6] = [
    "Welcome",
    "Providers",
    "Models",
    "Channels",
    "Review",
    "Done",
];

struct App {
    step: Step,
    providers: Vec<ProviderRow>,
    channels: Vec<ChannelRow>,
    default_provider: String,
    default_model: String,
    editing: bool,
    buffer: String,
    list_index: usize,
}

struct ProviderRow {
    name: &'static str,
    env: &'static str,
    enabled: bool,
}

struct ChannelRow {
    name: &'static str,
    enabled: bool,
    fields: Vec<(&'static str, String)>,
}

impl App {
    fn new() -> Self {
        Self {
            step: Step::Welcome,
            providers: vec![
                ProviderRow {
                    name: "openrouter",
                    env: "OPENROUTER_API_KEY",
                    enabled: true,
                },
                ProviderRow {
                    name: "openai",
                    env: "OPENAI_API_KEY",
                    enabled: false,
                },
                ProviderRow {
                    name: "anthropic",
                    env: "ANTHROPIC_API_KEY",
                    enabled: false,
                },
                ProviderRow {
                    name: "google",
                    env: "GOOGLE_API_KEY",
                    enabled: false,
                },
            ],
            channels: vec![
                ChannelRow {
                    name: "telegram",
                    enabled: false,
                    fields: vec![("bot_token", "${TELEGRAM_BOT_TOKEN}".into())],
                },
                ChannelRow {
                    name: "discord",
                    enabled: false,
                    fields: vec![("bot_token", "${DISCORD_BOT_TOKEN}".into())],
                },
                ChannelRow {
                    name: "slack",
                    enabled: false,
                    fields: vec![
                        ("bot_token", "${SLACK_BOT_TOKEN}".into()),
                        ("app_token", "${SLACK_APP_TOKEN}".into()),
                    ],
                },
                ChannelRow {
                    name: "whatsapp",
                    enabled: false,
                    fields: vec![
                        ("phone_number_id", "${WA_PHONE_NUMBER_ID}".into()),
                        ("access_token", "${WA_ACCESS_TOKEN}".into()),
                        ("verify_token", "zegion-verify".into()),
                    ],
                },
            ],
            default_provider: "openrouter".into(),
            default_model: "google/gemini-2.0-flash-001".into(),
            editing: false,
            buffer: String::new(),
            list_index: 0,
        }
    }

    fn build_config(&self) -> Config {
        let mut cfg = Config::default();
        cfg.models.default_provider = self.default_provider.clone();
        cfg.models.default_model = self.default_model.clone();

        for p in self.providers.iter().filter(|p| p.enabled) {
            let entry = ProviderConfig {
                api_key: Some(format!("${{{}}}", p.env)),
                ..Default::default()
            };
            cfg.providers.insert(p.name.to_string(), entry);
        }

        cfg.channels.insert(
            "cli".into(),
            ChannelConfig {
                enabled: true,
                ..Default::default()
            },
        );
        cfg.channels.insert(
            "gateway".into(),
            ChannelConfig {
                enabled: true,
                extra: [(
                    "bind".to_string(),
                    toml::Value::String("127.0.0.1:8787".into()),
                )]
                .into_iter()
                .collect(),
            },
        );

        for ch in self.channels.iter().filter(|c| c.enabled) {
            let extra = ch
                .fields
                .iter()
                .map(|(k, v)| (k.to_string(), toml::Value::String(v.clone())))
                .collect();
            cfg.channels.insert(
                ch.name.to_string(),
                ChannelConfig {
                    enabled: true,
                    extra,
                },
            );
        }
        cfg
    }
}

async fn run_app(mut terminal: DefaultTerminal) -> Result<Option<Config>> {
    let mut app = App::new();
    let mut events = EventStream::new();

    loop {
        terminal.draw(|f| draw(f, &app))?;

        let event = match events.next().await {
            Some(Ok(e)) => e,
            _ => continue,
        };
        let Event::Key(key) = event else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if app.editing {
            handle_edit(&mut app, key.code);
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            KeyCode::Enter | KeyCode::Char(' ') => handle_select(&mut app),
            KeyCode::Char('e') => {
                if app.step == Step::Models {
                    app.buffer = app.default_model.clone();
                    app.editing = true;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => move_sel(&mut app, -1),
            KeyCode::Down | KeyCode::Char('j') => move_sel(&mut app, 1),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('n') => next_step(&mut app),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('p') => prev_step(&mut app),
            _ => {}
        }

        if app.step == Step::Done {
            return Ok(Some(app.build_config()));
        }
    }
}

fn next_step(app: &mut App) {
    app.step = match app.step {
        Step::Welcome => Step::Providers,
        Step::Providers => Step::Models,
        Step::Models => Step::Channels,
        Step::Channels => Step::Review,
        Step::Review => Step::Done,
        Step::Done => Step::Done,
    };
    app.list_index = 0;
}

fn prev_step(app: &mut App) {
    app.step = match app.step {
        Step::Welcome => Step::Welcome,
        Step::Providers => Step::Welcome,
        Step::Models => Step::Providers,
        Step::Channels => Step::Models,
        Step::Review => Step::Channels,
        Step::Done => Step::Review,
    };
    app.list_index = 0;
}

fn move_sel(app: &mut App, dir: i32) {
    let len = match app.step {
        Step::Providers => app.providers.len(),
        Step::Channels => app.channels.len(),
        _ => return,
    };
    let cur = app.list_index as i32 + dir;
    app.list_index = cur.rem_euclid(len as i32) as usize;
}

fn handle_select(app: &mut App) {
    match app.step {
        Step::Welcome => next_step(app),
        Step::Providers => {
            let row = &mut app.providers[app.list_index];
            row.enabled = !row.enabled;
        }
        Step::Channels => {
            let row = &mut app.channels[app.list_index];
            row.enabled = !row.enabled;
        }
        Step::Models => {
            app.buffer = app.default_model.clone();
            app.editing = true;
        }
        Step::Review => next_step(app),
        Step::Done => {}
    }
}

fn handle_edit(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            if !app.buffer.trim().is_empty() {
                app.default_model = app.buffer.trim().to_string();
            }
            app.editing = false;
            app.buffer.clear();
        }
        KeyCode::Esc => {
            app.editing = false;
            app.buffer.clear();
        }
        KeyCode::Backspace => {
            app.buffer.pop();
        }
        KeyCode::Char(c) => app.buffer.push(c),
        _ => {}
    }
}

fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(f, chunks[0], app);
    draw_body(f, chunks[1], app);
    draw_footer(f, chunks[2], app);

    if app.editing {
        draw_edit_popup(f, area, app);
    }
}

fn step_index(step: Step) -> usize {
    match step {
        Step::Welcome => 0,
        Step::Providers => 1,
        Step::Models => 2,
        Step::Channels => 3,
        Step::Review => 4,
        Step::Done => 5,
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let idx = step_index(app.step);
    let mut spans = vec![Span::styled(
        " Zegion Onboarding  ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    for (i, title) in STEP_TITLES.iter().enumerate() {
        let style = if i == idx {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else if i < idx {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(format!(" {title} "), style));
        if i < STEP_TITLES.len() - 1 {
            spans.push(Span::raw(">"));
        }
    }
    let bar = Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::ALL));
    f.render_widget(bar, area);
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(format!(" {} ", STEP_TITLES[step_index(app.step)]))
        .borders(Borders::ALL);
    match app.step {
        Step::Welcome => {
            let text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Welcome to Zegion setup.",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("This wizard generates a zegion.toml for you."),
                Line::from(""),
                Line::from(
                    "- Secrets stay as ${ENV} placeholders; keys are never written to disk.",
                ),
                Line::from("- Toggle items with Space, move with Up/Down or j/k."),
                Line::from("- Tab/Right advances, BackTab/Left goes back, q quits."),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter to begin.",
                    Style::default().fg(Color::Green),
                )),
            ];
            f.render_widget(
                Paragraph::new(text).block(block).wrap(Wrap { trim: true }),
                area,
            );
        }
        Step::Providers => {
            let items: Vec<ListItem> = app
                .providers
                .iter()
                .map(|p| {
                    let mark = if p.enabled { "[x]" } else { "[ ]" };
                    ListItem::new(format!("{mark} {:<12} (${{{}}})", p.name, p.env))
                })
                .collect();
            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            let mut state = ListState::default();
            state.select(Some(app.list_index));
            f.render_stateful_widget(list, area, &mut state);
        }
        Step::Models => {
            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("Default provider:  "),
                    Span::styled(&app.default_provider, Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("Default model:     "),
                    Span::styled(&app.default_model, Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Press 'e' or Space to edit the model, Tab to continue.",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            f.render_widget(
                Paragraph::new(text).block(block).wrap(Wrap { trim: true }),
                area,
            );
        }
        Step::Channels => {
            let items: Vec<ListItem> = app
                .channels
                .iter()
                .map(|c| {
                    let mark = if c.enabled { "[x]" } else { "[ ]" };
                    let detail = c
                        .fields
                        .iter()
                        .map(|(k, _)| *k)
                        .collect::<Vec<_>>()
                        .join(", ");
                    ListItem::new(format!("{mark} {:<10} ({detail})", c.name))
                })
                .collect();
            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            let mut state = ListState::default();
            state.select(Some(app.list_index));
            f.render_stateful_widget(list, area, &mut state);
        }
        Step::Review => {
            let cfg = app.build_config();
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Summary",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!(
                    "Providers: {}",
                    cfg.providers.keys().cloned().collect::<Vec<_>>().join(", ")
                )),
                Line::from(format!(
                    "Model:     {}/{}",
                    cfg.models.default_provider, cfg.models.default_model
                )),
                Line::from(format!(
                    "Channels:  {}",
                    cfg.channels.keys().cloned().collect::<Vec<_>>().join(", ")
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Tab/Enter to write the config, or BackTab to go back.",
                    Style::default().fg(Color::Green),
                )),
            ];
            f.render_widget(
                Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
                area,
            );
        }
        Step::Done => {
            let text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Done! Writing config...",
                    Style::default().fg(Color::Green),
                )),
            ];
            f.render_widget(Paragraph::new(text).block(block), area);
        }
    }
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let hint = match app.step {
        Step::Welcome => "Enter: begin   q: quit",
        Step::Providers | Step::Channels => {
            "Space: toggle   Up/Down: move   Tab: next   BackTab: back   q: quit"
        }
        Step::Models => "e: edit   Tab: next   BackTab: back   q: quit",
        Step::Review => "Tab/Enter: write config   BackTab: back   q: quit",
        Step::Done => "",
    };
    let footer = Paragraph::new(hint)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}

fn draw_edit_popup(f: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(60, 5, area);
    f.render_widget(Clear, popup);
    let input = Paragraph::new(app.buffer.as_str()).block(
        Block::default()
            .title(" Default model (Enter to save, Esc to cancel) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(input, popup);
    f.set_cursor_position((popup.x + 1 + app.buffer.len() as u16, popup.y + 1));
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(r.width), height.min(r.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_config_roundtrips_to_toml() {
        let app = App::new();
        let cfg = app.build_config();
        let text = cfg.to_toml().expect("serializes");
        assert!(text.contains("[providers.openrouter]"));
        assert!(text.contains("OPENROUTER_API_KEY"));
        assert!(text.contains("[channels.cli]"));
        assert!(text.contains("[channels.gateway]"));
        let tmp = std::env::temp_dir().join("zegion-onboard-test.toml");
        std::fs::write(&tmp, &text).unwrap();
        let loaded = Config::load(&tmp).await.expect("reloads");
        assert!(loaded.providers.contains_key("openrouter"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn enabled_channels_are_serialized() {
        let mut app = App::new();
        app.channels[0].enabled = true;
        let cfg = app.build_config();
        let tg = &cfg.channels["telegram"];
        assert!(tg.enabled);
        assert_eq!(
            tg.extra.get("bot_token").and_then(|v| v.as_str()),
            Some("${TELEGRAM_BOT_TOKEN}")
        );
    }
}
