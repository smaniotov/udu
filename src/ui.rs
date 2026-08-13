use crate::app::{App, Screen, ServiceModal, SettingsTab};
use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use std::path::Path;

const YELLOW: Color = Color::Yellow;
const ACCENT: Color = Color::Yellow;
const WHITE: Color = Color::White;
const GRAY: Color = Color::Gray;
const DIM: Color = Color::DarkGray;
const GREEN: Color = Color::Green;
const BLACK: Color = Color::Black;

const BANNER: [&str; 5] = [
    "██   ██  ██████   ██   ██",
    "██   ██  ██   ██  ██   ██",
    "██   ██  ██   ██  ██   ██",
    "██   ██  ██   ██  ██   ██",
    " █████   ██████    █████ ",
];

pub fn draw(frame: &mut Frame, app: &mut App) {
    if let Some(modal) = app.service_modal {
        draw_service_modal(frame, app, modal);
        return;
    }

    match app.screen {
        Screen::Launcher => draw_launcher(frame, app),
        Screen::Settings => draw_settings(frame, app),
    }
}

fn draw_service_modal(frame: &mut Frame, app: &App, modal: ServiceModal) {
    match modal {
        ServiceModal::InstallConsent => draw_install_consent(frame, app),
        ServiceModal::ConfirmUninstall => draw_uninstall_confirm(frame),
    }
}

fn draw_install_consent(frame: &mut Frame, app: &App) {
    let area = frame
        .area()
        .centered(Constraint::Percentage(88), Constraint::Length(18));
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(YELLOW))
        .title(Line::from(Span::styled(
            " udu wants to install a system service ",
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, area);

    let inner = area.inner(Margin::new(2, 1));
    let unit_path = unit_file_display_path();
    let executable = resolved_executable_display();
    let config_path = app.config_path.display();

    let lines = vec![
        Line::from(""),
        Line::from("This installs and ENABLES a service that starts at"),
        Line::from("every login and reads your keyboard via /dev/input:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {unit_path}"),
            Style::default().fg(WHITE),
        )),
        Line::from(Span::styled(
            format!("  ExecStart={executable} --service \\"),
            Style::default().fg(WHITE),
        )),
        Line::from(Span::styled(
            format!("            --config {config_path}"),
            Style::default().fg(WHITE),
        )),
        Line::from(""),
        Line::from("You can remove it later with the [U] key in the TUI."),
        Line::from(""),
        Line::from(vec![
            Span::raw("        "),
            Span::styled(
                "[s]",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" install        "),
            Span::styled(
                "[n]",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" not now"),
        ]),
    ];
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn draw_uninstall_confirm(frame: &mut Frame) {
    let area = frame
        .area()
        .centered(Constraint::Percentage(64), Constraint::Length(7));
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(YELLOW))
        .title(Line::from(Span::styled(
            " remove the udu service? ",
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, area);

    let inner = area.inner(Margin::new(2, 1));
    let lines = vec![
        Line::from("This stops the backend and removes the systemd unit."),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[y]",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" remove        "),
            Span::styled(
                "[n]",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" cancel"),
        ]),
    ];
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn unit_file_display_path() -> String {
    let Some(config_dir) = dirs::config_dir() else {
        return format!("~/.config/systemd/user/{}", crate::service::SERVICE_NAME);
    };

    config_dir
        .join("systemd/user")
        .join(crate::service::SERVICE_NAME)
        .display()
        .to_string()
}

fn resolved_executable_display() -> String {
    let Ok(executable) = std::env::current_exe() else {
        return String::from("udu");
    };

    executable
        .canonicalize()
        .unwrap_or(executable)
        .display()
        .to_string()
}

fn draw_launcher(frame: &mut Frame, app: &mut App) {
    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .areas(frame.area());

    draw_header(frame, app, header_area);

    let body_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));
    frame.render_widget(body_block, body_area);

    let body = body_area.inner(Margin::new(1, 1));
    let [
        _top_pad,
        banner_area,
        _mid_pad,
        search_area,
        list_label_area,
        list_area,
        _bottom_pad,
    ] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(body);

    draw_banner(frame, banner_area);
    draw_search(frame, app, search_area);
    draw_list_label(frame, list_label_area);
    draw_pack_list(frame, app, list_area);

    draw_footer(frame, app, footer_area);
}

const WIDE_HEADER_HINTS: [(&str, &str); 6] = [
    ("[↑↓]", " browse  "),
    ("[Enter]", " select  "),
    ("[Tab]", " settings  "),
    ("[+/-]", " volume  "),
    ("[?]", " about  "),
    ("[q]", " quit"),
];

const NARROW_HEADER_HINTS: [(&str, &str); 6] = [
    ("[↑↓]", "  "),
    ("[Enter]", "  "),
    ("[Tab]", " settings  "),
    ("[+/-]", " vol  "),
    ("[?]", " about  "),
    ("[q]", " quit"),
];

const WIDE_UNINSTALL_HINT: (&str, &str) = ("[U]", " remove service  ");
const NARROW_UNINSTALL_HINT: (&str, &str) = ("[U]", " service  ");

fn header_hints(installed: bool, width: u16) -> Vec<(&'static str, &'static str)> {
    let wide = hints_with_uninstall(&WIDE_HEADER_HINTS, WIDE_UNINSTALL_HINT, installed);

    if hints_width(&wide) <= width {
        return wide;
    }

    hints_with_uninstall(&NARROW_HEADER_HINTS, NARROW_UNINSTALL_HINT, installed)
}

fn hints_with_uninstall(
    base: &[(&'static str, &'static str); 6],
    uninstall: (&'static str, &'static str),
    installed: bool,
) -> Vec<(&'static str, &'static str)> {
    let (navigation, closing) = base.split_at(4);

    if !installed {
        return base.to_vec();
    }

    navigation
        .iter()
        .copied()
        .chain(std::iter::once(uninstall))
        .chain(closing.iter().copied())
        .collect()
}

fn hints_width(hints: &[(&str, &str)]) -> u16 {
    let total: usize = hints
        .iter()
        .map(|(key, label)| key.chars().count() + label.chars().count())
        .sum();

    u16::try_from(total).unwrap_or(u16::MAX)
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let inner_width = area.width.saturating_sub(2);
    let spans: Vec<Span> = header_hints(app.service_installed(), inner_width)
        .into_iter()
        .flat_map(|(key, label)| {
            [
                Span::styled(key, Style::default().fg(ACCENT)),
                Span::raw(label),
            ]
        })
        .collect();

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)),
    );
    frame.render_widget(paragraph, area);
}

fn draw_banner(frame: &mut Frame, area: Rect) {
    let lines: Vec<Line> = BANNER
        .iter()
        .map(|line| {
            let spans = line
                .chars()
                .map(|c| {
                    if c == ' ' {
                        Span::raw(" ")
                    } else {
                        Span::styled(
                            c.to_string(),
                            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                        )
                    }
                })
                .collect::<Vec<Span>>();
            Line::from(spans)
        })
        .collect();
    let banner = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(banner, area);
}

fn draw_search(frame: &mut Frame, app: &mut App, area: Rect) {
    let width = area.width.min(60);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let search_area = Rect::new(x, area.y, width, area.height);

    let prompt = format!("/ {}", app.search_query);
    let color = if app.search_query.is_empty() {
        DIM
    } else {
        WHITE
    };
    let border = if app.search_query.is_empty() {
        DIM
    } else {
        YELLOW
    };
    let paragraph = Paragraph::new(Line::from(Span::styled(prompt, Style::default().fg(color))))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        );
    frame.render_widget(paragraph, search_area);
}

fn draw_list_label(frame: &mut Frame, area: Rect) {
    let width = area.width.min(60);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let label_area = Rect::new(x, area.y, width, area.height);
    let paragraph = Paragraph::new(Line::from(Span::styled(
        "  soundpacks",
        Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(paragraph, label_area);
}

fn draw_pack_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let packs: Vec<(String, usize)> = app
        .visible_packs()
        .into_iter()
        .map(|pack| (pack.name.clone(), pack.mapping_count))
        .collect();
    let selected = app.list_state.selected();

    if packs.is_empty() {
        let message = if app.search_query.is_empty() {
            "No soundpacks found"
        } else {
            "No soundpacks match your search"
        };
        let paragraph = Paragraph::new(Line::from(Span::styled(
            message,
            Style::default().fg(YELLOW),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let items = packs
        .iter()
        .enumerate()
        .map(|(index, (name, count))| {
            if selected == Some(index) {
                ListItem::new(Line::from(vec![
                    Span::styled("› ", Style::default().fg(BLACK).bg(YELLOW)),
                    Span::styled(
                        name,
                        Style::default()
                            .fg(BLACK)
                            .bg(YELLOW)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {count} mapping(s)"),
                        Style::default().fg(BLACK).bg(YELLOW),
                    ),
                ]))
            } else {
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(name, Style::default().fg(WHITE)),
                    Span::styled(format!("  {count} mapping(s)"), Style::default().fg(DIM)),
                ]))
            }
        })
        .collect::<Vec<_>>();

    let list = List::new(items).highlight_symbol("");
    let width = area.width.min(60);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let list_area = Rect::new(x, area.y, width, area.height);
    frame.render_stateful_widget(list, list_area, &mut app.list_state);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));
    frame.render_widget(block, area);

    let inner = area.inner(Margin::new(1, 1));
    let [volume_area, service_area, status_area] = Layout::horizontal([
        Constraint::Length(20),
        Constraint::Length(24),
        Constraint::Fill(1),
    ])
    .areas(inner);

    let volume = Paragraph::new(Line::from(vec![
        Span::styled("volume", Style::default().fg(GRAY)),
        Span::raw("  "),
        Span::styled(
            format!("{:.1}", app.config.volume),
            Style::default().fg(WHITE),
        ),
    ]));
    frame.render_widget(volume, volume_area);

    let service = Paragraph::new(Line::from(service_state_span(app)));
    frame.render_widget(service, service_area);

    let status = Paragraph::new(Line::from(Span::styled(
        &app.status,
        Style::default().fg(GRAY),
    )));
    frame.render_widget(status, status_area);
}

fn service_state_span(app: &App) -> Span<'static> {
    if app.service_installed() {
        Span::styled("service: armed", Style::default().fg(GREEN))
    } else {
        Span::styled("service: not installed", Style::default().fg(YELLOW))
    }
}

fn draw_settings(frame: &mut Frame, app: &mut App) {
    let area = frame
        .area()
        .centered(Constraint::Percentage(78), Constraint::Percentage(82));

    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(YELLOW))
        .title(Line::from(Span::styled(
            " settings ",
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, area);

    let inner = area.inner(Margin::new(1, 1));
    let [tabs_area, body_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    draw_settings_tabs(frame, app, tabs_area);
    draw_settings_body(frame, app, body_area);
    draw_settings_hint(frame, app, hint_area);
}

fn draw_settings_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let tabs = [
        ("General", SettingsTab::General),
        ("Audio", SettingsTab::Audio),
        ("Devices", SettingsTab::Devices),
        ("About", SettingsTab::About),
    ];
    let mut spans = Vec::new();
    for (index, (label, tab)) in tabs.iter().enumerate() {
        let active = *tab == app.settings_tab;
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        let text = if active {
            format!("● {label}")
        } else {
            format!("  {label}")
        };
        spans.push(Span::styled(
            text,
            if active {
                Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(GRAY)
            },
        ));
    }
    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

fn draw_settings_body(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.settings_tab {
        SettingsTab::General => draw_general_settings(frame, app, area),
        SettingsTab::Audio => draw_audio_settings(frame, app, area),
        SettingsTab::Devices => draw_devices_settings(frame, app, area),
        SettingsTab::About => draw_about_settings(frame, app, area),
    }
}

fn draw_general_settings(frame: &mut Frame, app: &App, area: Rect) {
    draw_setting_rows(frame, app, &crate::app::GENERAL_SETTINGS, area);
}

fn draw_audio_settings(frame: &mut Frame, app: &App, area: Rect) {
    draw_setting_rows(frame, app, &crate::app::AUDIO_SETTINGS, area);
}

fn draw_setting_rows(frame: &mut Frame, app: &App, rows: &[crate::app::SettingRow], area: Rect) {
    let items = rows
        .iter()
        .enumerate()
        .map(|(index, row)| setting_row_item(app, row, index == app.settings_index))
        .collect::<Vec<_>>();
    let list = List::new(items);
    frame.render_widget(list, area);
}

fn setting_row_item(app: &App, row: &crate::app::SettingRow, active: bool) -> ListItem<'static> {
    let label_style = if active {
        Style::default().fg(WHITE).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(WHITE)
    };
    let value_style = match row.tone(&app.config) {
        crate::app::SettingTone::On => Style::default().fg(GREEN),
        crate::app::SettingTone::Off => Style::default().fg(DIM),
        crate::app::SettingTone::Accent => Style::default().fg(ACCENT),
        crate::app::SettingTone::Plain => Style::default().fg(WHITE),
    };

    ListItem::new(Line::from(vec![
        Span::styled(format!("  {:<20}", row.label), label_style),
        Span::styled(row.value_text(&app.config), value_style),
    ]))
}

fn draw_devices_settings(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.devices.is_empty() {
        let paragraph = Paragraph::new(Line::from(Span::styled(
            "No keyboard devices found",
            Style::default().fg(YELLOW),
        )));
        frame.render_widget(paragraph, area);
        return;
    }

    let items = app
        .devices
        .iter()
        .enumerate()
        .map(|(index, device)| {
            let active = app.device_list_state.selected() == Some(index);
            ListItem::new(Line::from(vec![
                Span::styled(
                    if active { "› " } else { "  " },
                    Style::default().fg(YELLOW),
                ),
                Span::styled(
                    &device.name,
                    if active {
                        Style::default().fg(WHITE).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(WHITE)
                    },
                ),
                Span::styled(
                    format!("  {}", device.path.display()),
                    Style::default().fg(DIM),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(items);
    frame.render_stateful_widget(list, area, &mut app.device_list_state);
}

const COMPACT_BANNER: [&str; 3] = [
    "██   ██  ██████   ██   ██",
    "██   ██  ██   ██  ██   ██",
    " █████   ██████    █████ ",
];

const ABOUT_TAGLINE: &str = "Mechanical keyboard sounds for Linux";
const ABOUT_ORIGIN: &str = "ùdù — the Igbo pot drum";
const FACT_LABEL_WIDTH: usize = 12;

fn home_relative(path: &Path) -> String {
    let Some(home) = dirs::home_dir() else {
        return path.display().to_string();
    };

    match path.strip_prefix(&home) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

fn centered_line(text: &str, width: u16, style: Style) -> Line<'static> {
    let padding = (usize::from(width).saturating_sub(text.chars().count())) / 2;

    Line::from(vec![
        Span::raw(" ".repeat(padding)),
        Span::styled(text.to_string(), style),
    ])
}

fn fact_line(label: &str, value: String, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{label:<FACT_LABEL_WIDTH$}"),
            Style::default().fg(GRAY),
        ),
        Span::styled(value, value_style),
    ])
}

fn pack_count_text(count: usize) -> String {
    if count == 1 {
        return String::from("1 pack found");
    }

    format!("{count} packs found")
}

fn about_facts(app: &App) -> Vec<Line<'static>> {
    let roots = app.config.soundpack_roots.first().map_or_else(
        || String::from("none configured"),
        |root| home_relative(root),
    );
    let pack_count = app.packs.len();
    let service = if app.service_installed() {
        (String::from("udu.service"), GREEN, " ● active")
    } else {
        (String::from("udu.service"), DIM, " ○ not installed")
    };
    let (service_name, service_color, service_state) = service;

    vec![
        fact_line("soundpacks", roots, Style::default().fg(WHITE)),
        fact_line("", pack_count_text(pack_count), Style::default().fg(DIM)),
        fact_line(
            "config",
            home_relative(&app.config_path),
            Style::default().fg(WHITE),
        ),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{:<FACT_LABEL_WIDTH$}", "service"),
                Style::default().fg(GRAY),
            ),
            Span::styled(service_name, Style::default().fg(WHITE)),
            Span::styled(service_state, Style::default().fg(service_color)),
        ]),
        fact_line(
            "version",
            String::from(env!("CARGO_PKG_VERSION")),
            Style::default().fg(DIM),
        ),
    ]
}

fn draw_about_settings(frame: &mut Frame, app: &App, area: Rect) {
    let logo = COMPACT_BANNER.iter().map(|row| {
        centered_line(
            row,
            area.width,
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        )
    });
    let identity = [
        Line::from(""),
        centered_line(ABOUT_TAGLINE, area.width, Style::default().fg(WHITE)),
        centered_line(ABOUT_ORIGIN, area.width, Style::default().fg(DIM)),
        Line::from(""),
    ];
    let permissions = [
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::raw("udu reads /dev/input directly. No keyboard listed?"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::raw("Run "),
            Span::styled("sudo usermod -aG input $USER", Style::default().fg(ACCENT)),
            Span::raw(" and log back in."),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Never run udu as root.",
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let text: Vec<Line> = std::iter::once(Line::from(""))
        .chain(logo)
        .chain(identity)
        .chain(about_facts(app))
        .chain(permissions)
        .collect();
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_settings_hint(frame: &mut Frame, app: &App, area: Rect) {
    let hint = match app.settings_tab {
        SettingsTab::General => "[↑↓] move    [Enter] toggle    [Tab] next tab    [Esc] close",
        SettingsTab::Audio => "[↑↓] move    [←/→] adjust    [Tab] next tab    [Esc] close",
        SettingsTab::Devices => "[↑↓] move    [Enter] activate    [Tab] next tab    [Esc] close",
        SettingsTab::About => "[Tab] next tab    [Esc] close",
    };
    let paragraph = Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled(hint, Style::default().fg(GRAY)),
    ]));
    frame.render_widget(paragraph, area);
}

pub fn handle_event(app: &mut App, event: Event) -> Result<()> {
    let Event::Key(key) = event else {
        return Ok(());
    };

    if key.kind != KeyEventKind::Press {
        return Ok(());
    }

    if let Some(modal) = app.service_modal {
        return handle_service_modal_key(app, modal, key);
    }

    match app.screen {
        Screen::Launcher => handle_launcher_key(app, key),
        Screen::Settings => handle_settings_key(app, key),
    }
}

fn handle_service_modal_key(app: &mut App, modal: ServiceModal, key: KeyEvent) -> Result<()> {
    match modal {
        ServiceModal::InstallConsent => handle_install_consent_key(app, key),
        ServiceModal::ConfirmUninstall => handle_uninstall_confirm_key(app, key),
    }
}

fn handle_install_consent_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('s') => app.grant_service_consent(),
        KeyCode::Char('n') => {
            app.decline_service_consent();
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_uninstall_confirm_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => app.confirm_uninstall_service(),
        KeyCode::Char('n') | KeyCode::Esc => {
            app.cancel_uninstall_service();
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_launcher_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up => app.select_previous(),
        KeyCode::Down => app.select_next(),
        KeyCode::Enter => app.activate_selected()?,
        KeyCode::Tab => app.open_settings(),
        KeyCode::Backspace => app.backspace_search(),
        KeyCode::Esc => app.clear_search(),
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => app.quit(),
        KeyCode::Char('s') => app.open_settings(),
        KeyCode::Char('?') => {
            app.open_settings();
            app.settings_tab = SettingsTab::About;
        }
        KeyCode::Char('x') => app.toggle_sound()?,
        KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_volume(5.0)?,
        KeyCode::Char('-') => app.adjust_volume(-5.0)?,
        KeyCode::Char('1') => app.apply_preset(crate::config::VOLUME_SOFT)?,
        KeyCode::Char('2') => app.apply_preset(crate::config::VOLUME_BALANCED)?,
        KeyCode::Char('3') => app.apply_preset(crate::config::VOLUME_LOUD)?,
        KeyCode::Char('p') => app.preview_selected()?,
        KeyCode::Char('r') => app.refresh()?,
        KeyCode::Char('U') => app.request_uninstall_confirmation(),
        KeyCode::Char(c) if c.is_ascii_graphic() && c != ' ' => app.type_search(c),
        KeyCode::Char(' ') => app.type_search(' '),
        _ => {}
    }

    Ok(())
}

fn handle_settings_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_settings(),
        KeyCode::Tab => app.cycle_settings_tab(true),
        KeyCode::BackTab => app.cycle_settings_tab(false),
        KeyCode::Up => app.select_previous(),
        KeyCode::Down => app.select_next(),
        KeyCode::Enter => app.activate_selected()?,
        KeyCode::Left if app.settings_tab == SettingsTab::Audio => {
            app.adjust_audio_setting(-1.0)?
        }
        KeyCode::Right if app.settings_tab == SettingsTab::Audio => {
            app.adjust_audio_setting(1.0)?
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{draw, handle_event, header_hints, hints_width};
    use crate::app::{App, Screen, ServiceModal, SettingsTab};
    use crate::config::AppConfig;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::fs;

    fn buffer_text(app: &mut App) -> String {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal.draw(|frame| draw(frame, app)).expect("draw app");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    fn test_app(name: &str) -> (App, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("udu-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create test directory");
        let app = App::new(root.join("config.json"), AppConfig::default()).expect("create app");
        (app, root)
    }

    fn sample_packs() -> Vec<crate::soundpack::Soundpack> {
        vec![
            crate::soundpack::Soundpack {
                name: String::from("nk-cream"),
                path: std::path::PathBuf::from("/packs/nk-cream"),
                mapping_count: 128,
            },
            crate::soundpack::Soundpack {
                name: String::from("membrane-60"),
                path: std::path::PathBuf::from("/packs/membrane-60"),
                mapping_count: 96,
            },
        ]
    }

    #[test]
    fn header_keeps_help_and_quit_reachable_at_the_minimum_terminal_width() {
        const MINIMUM_INNER_WIDTH: u16 = 78;

        for installed in [true, false] {
            let hints = header_hints(installed, MINIMUM_INNER_WIDTH);

            assert!(
                hints_width(&hints) <= MINIMUM_INNER_WIDTH,
                "header must fit an 80 column terminal when installed={installed}"
            );
            assert!(hints.iter().any(|(key, _)| *key == "[q]"));
            assert!(hints.iter().any(|(key, _)| *key == "[?]"));
        }
    }

    #[test]
    fn header_offers_the_uninstall_key_only_while_the_service_is_installed() {
        let wide = u16::MAX;

        assert!(
            header_hints(true, wide)
                .iter()
                .any(|(key, _)| *key == "[U]")
        );
        assert!(
            !header_hints(false, wide)
                .iter()
                .any(|(key, _)| *key == "[U]")
        );
    }

    #[test]
    fn renders_the_launcher_with_banner_search_and_volume() {
        let (mut app, root) = test_app("launcher");
        app.packs = sample_packs();
        app.list_state.select(Some(0));
        app.config.volume = 50.0;
        app.status = String::from("playing nk-cream");

        let text = buffer_text(&mut app);

        assert!(
            text.contains("██   ██"),
            "banner should render the UDU brand"
        );
        assert!(text.contains("/ "), "search field should render");
        assert!(text.contains("nk-cream"), "soundpack should render");
        assert!(text.contains("50.0"), "volume gauge label should render");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn typing_a_query_filters_the_pack_list() {
        let (mut app, root) = test_app("filter");
        app.packs = sample_packs();

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)),
        )
        .expect("type search");
        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)),
        )
        .expect("type search");

        assert_eq!(app.search_query, "me");
        assert_eq!(app.visible_packs().len(), 1);
        assert_eq!(
            app.selected_pack().map(|p| p.name.as_str()),
            Some("membrane-60")
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn backspace_clears_the_search_query() {
        let (mut app, root) = test_app("backspace");
        app.type_search('c');

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        )
        .expect("backspace");

        assert!(app.search_query.is_empty());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn s_opens_settings_and_esc_closes_it() {
        let (mut app, root) = test_app("settings");

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
        )
        .expect("open settings");
        assert_eq!(app.screen, Screen::Settings);

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        )
        .expect("close settings");
        assert_eq!(app.screen, Screen::Launcher);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn tab_cycles_through_settings_tabs() {
        let (mut app, root) = test_app("tabs");
        app.open_settings();
        assert_eq!(app.settings_tab, SettingsTab::General);

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        )
        .expect("cycle tab");
        assert_eq!(app.settings_tab, SettingsTab::Audio);

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        )
        .expect("cycle tab");
        assert_eq!(app.settings_tab, SettingsTab::Devices);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn ctrl_c_quits_from_the_launcher() {
        let (mut app, root) = test_app("quit");

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        )
        .expect("handle ctrl-c");

        assert!(app.should_quit);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn pressing_n_on_the_install_consent_modal_declines_it() {
        let (mut app, root) = test_app("decline-modal");
        app.service_modal = Some(ServiceModal::InstallConsent);

        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
        )
        .expect("decline consent");

        assert_eq!(app.service_modal, None);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn about_shows_the_configured_soundpack_root_rather_than_a_fixed_default() {
        let (mut app, root) = test_app("about");
        app.screen = Screen::Settings;
        app.settings_tab = SettingsTab::About;
        app.config.soundpack_roots = vec![root.join("packs-from-config")];

        let text = buffer_text(&mut app);

        assert!(
            text.contains("packs-from-config"),
            "the About tab must show the root the user actually configured"
        );
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn install_consent_modal_shows_the_required_wording_and_the_real_config_path() {
        let (mut app, root) = test_app("consent");
        app.service_modal = Some(ServiceModal::InstallConsent);
        let config_path = app.config_path.display().to_string();

        let text = buffer_text(&mut app);

        assert!(text.contains("udu wants to install a system service"));
        assert!(text.contains("reads your keyboard via /dev/input"));
        assert!(text.contains("You can remove it later with the [U] key"));
        assert!(text.contains("[s]"), "the install key should render");
        assert!(text.contains("[n]"), "the decline key should render");
        assert!(
            text.contains(&config_path),
            "the real config path should render instead of a placeholder"
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn uninstall_confirmation_modal_offers_confirm_and_cancel() {
        let (mut app, root) = test_app("uninstall-confirm");
        app.service_modal = Some(ServiceModal::ConfirmUninstall);

        let text = buffer_text(&mut app);

        assert!(text.contains("remove the udu service?"));
        assert!(text.contains("[y]"));
        assert!(text.contains("[n]"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn footer_always_shows_a_service_state_indicator() {
        let (mut app, root) = test_app("service-indicator");
        app.packs = sample_packs();
        app.list_state.select(Some(0));

        let text = buffer_text(&mut app);

        assert!(text.contains("service:"));
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
