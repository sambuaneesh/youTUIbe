use crate::{
    app::{App, ClickAction, Hit, Modal, PlaybackState, SearchFormatChoice, Tab},
    model::{DownloadStatus, Job},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Gauge, List, ListItem, Padding, Paragraph, Row,
        Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Tabs, Wrap,
    },
};
use ratatui_image::StatefulImage;
use unicode_width::UnicodeWidthStr;

const BG: Color = Color::Rgb(7, 11, 17);
const PANEL: Color = Color::Rgb(13, 20, 29);
const PANEL_2: Color = Color::Rgb(20, 30, 43);
const TEXT: Color = Color::Rgb(230, 237, 243);
const MUTED: Color = Color::Rgb(126, 145, 164);
const CYAN: Color = Color::Rgb(94, 234, 212);
const GREEN: Color = Color::Rgb(134, 239, 172);
const ORANGE: Color = Color::Rgb(251, 191, 36);
const RED: Color = Color::Rgb(251, 113, 133);

pub fn render(frame: &mut Frame, app: &mut App) {
    frame.render_widget(
        Block::default().style(Style::default().bg(BG)),
        frame.area(),
    );
    app.hits.clear();
    let area = frame.area();
    if area.width < 72 || area.height < 22 {
        frame.render_widget(
            Paragraph::new("Tide needs a terminal at least 72×22.\nResize the window to continue.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(ORANGE))
                .block(panel(" Terminal too small ")),
            centered(54, 7, area),
        );
        return;
    }
    let root = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(2),
    ])
    .split(area);
    header(frame, app, root[0]);
    tabs(frame, app, root[1]);
    match app.tab {
        Tab::Download => download(frame, app, root[2]),
        Tab::Search => search(frame, app, root[2]),
        Tab::Queue => queue(frame, app, root[2]),
        Tab::History => history(frame, app, root[2]),
        Tab::Settings => settings(frame, app, root[2]),
        Tab::Logs => logs(frame, app, root[2]),
        Tab::Help => help(frame, app, root[2]),
    }
    footer(frame, app, root[3]);
    if app.modal.is_some() {
        modal(frame, app);
    }
}

fn header(frame: &mut Frame, app: &App, area: Rect) {
    let queued = app
        .jobs
        .iter()
        .filter(|j| j.status == DownloadStatus::Queued)
        .count();
    let active = app
        .jobs
        .iter()
        .filter(|j| {
            matches!(
                j.status,
                DownloadStatus::Downloading
                    | DownloadStatus::Processing
                    | DownloadStatus::Paused
                    | DownloadStatus::Inspecting
            )
        })
        .count();
    let done = app
        .jobs
        .iter()
        .filter(|j| j.status == DownloadStatus::Completed)
        .count();
    let mut spans = vec![
        Span::styled(
            "  ◉  TIDE",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  media, your way", Style::default().fg(MUTED)),
        Span::raw("      "),
        Span::styled(format!("● {active} active"), Style::default().fg(GREEN)),
        Span::styled("    ", Style::default()),
        Span::styled(format!("◆ {queued} queued"), Style::default().fg(ORANGE)),
        Span::styled("    ", Style::default()),
        Span::styled(format!("✓ {done} complete"), Style::default().fg(CYAN)),
    ];
    if app.playback_state != PlaybackState::Stopped {
        spans.push(Span::styled("    ♫ ", Style::default().fg(CYAN)));
        spans.push(Span::styled(
            format!(
                "{} · {}",
                if app.playback_state == PlaybackState::Paused {
                    "paused"
                } else {
                    "playing"
                },
                clip(&app.playback_title, 24)
            ),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ));
    }
    let line = Line::from(spans);
    frame.render_widget(
        Paragraph::new(line)
            .style(Style::default().bg(PANEL))
            .block(Block::default().padding(Padding::vertical(1))),
        area,
    );
}

fn tabs(frame: &mut Frame, app: &mut App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| Line::from(format!(" {} {} ", i + 1, tab_caption(*t))))
        .collect();
    frame.render_widget(
        Tabs::new(titles)
            .select(app.tab.index())
            .style(Style::default().fg(MUTED).bg(BG))
            .highlight_style(
                Style::default()
                    .fg(BG)
                    .bg(CYAN)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" "),
        area,
    );
    let mut x = area.x;
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, tab_caption(*tab));
        let width = UnicodeWidthStr::width(label.as_str()) as u16 + u16::from(i > 0);
        app.hits.push(Hit {
            rect: Rect::new(
                x,
                area.y,
                width.min(area.right().saturating_sub(x)),
                area.height,
            ),
            action: ClickAction::Tab(*tab),
        });
        x = x.saturating_add(width);
    }
}

fn download(frame: &mut Frame, app: &mut App, area: Rect) {
    let outer = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .split(area);
    let top = Layout::horizontal([
        Constraint::Min(30),
        Constraint::Length(13),
        Constraint::Length(16),
        Constraint::Length(13),
    ])
    .split(outer[0]);
    let url_text = if app.url.is_empty() {
        "Paste one or more video / playlist URLs".into()
    } else {
        app.url.clone()
    };
    let url_style = if app.url.is_empty() {
        Style::default().fg(MUTED)
    } else {
        Style::default().fg(TEXT)
    };
    frame.render_widget(
        Paragraph::new(url_text)
            .style(url_style)
            .block(panel(" URL  [/] edit ")),
        top[0],
    );
    app.hits.push(Hit {
        rect: top[0],
        action: ClickAction::Url,
    });
    button(frame, top[1], "Inspect", "i", CYAN);
    app.hits.push(Hit {
        rect: top[1],
        action: ClickAction::Inspect,
    });
    button(frame, top[2], "Add + view", "d", GREEN);
    app.hits.push(Hit {
        rect: top[2],
        action: ClickAction::AddStart,
    });
    button(frame, top[3], "Queue", "a", ORANGE);
    app.hits.push(Hit {
        rect: top[3],
        action: ClickAction::Add,
    });

    let cols = Layout::horizontal([Constraint::Length(29), Constraint::Min(42)]).split(outer[1]);
    let preset_data = &app.presets;
    let items: Vec<ListItem> = preset_data
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let marker = if i == app.preset_index { "●" } else { "○" };
            ListItem::new(Text::from(vec![
                Line::from(vec![
                    Span::styled(
                        format!("{marker} "),
                        Style::default().fg(if i == app.preset_index { CYAN } else { MUTED }),
                    ),
                    Span::styled(
                        p.name,
                        Style::default()
                            .fg(TEXT)
                            .add_modifier(if i == app.preset_index {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                ]),
                Line::styled(format!("   {}", p.summary), Style::default().fg(MUTED)),
            ]))
        })
        .collect();
    frame.render_widget(
        List::new(items)
            .block(panel(" Presets  [j/k] "))
            .highlight_style(Style::default().bg(PANEL_2)),
        cols[0],
    );
    for (i, _) in preset_data.iter().enumerate() {
        let y = cols[0].y + 1 + (i as u16 * 2);
        if y < cols[0].bottom() - 1 {
            app.hits.push(Hit {
                rect: Rect::new(cols[0].x + 1, y, cols[0].width - 2, 2),
                action: ClickAction::Preset(i),
            });
        }
    }

    let values = app.builder_values();
    let rows: Vec<Row> = values
        .iter()
        .enumerate()
        .map(|(i, (name, value))| {
            Row::new([Cell::from(*name), Cell::from(value.clone())]).style(
                if i == app.builder_index {
                    Style::default()
                        .fg(BG)
                        .bg(CYAN)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(TEXT)
                        .bg(if i % 2 == 0 { PANEL } else { PANEL_2 })
                },
            )
        })
        .collect();
    let mut state = TableState::default();
    state.select(Some(app.builder_index));
    let table = Table::new(
        rows,
        [Constraint::Percentage(42), Constraint::Percentage(58)],
    )
    .header(
        Row::new(["Option", "Value"])
            .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(panel(" Build  [↑↓] select  [←→/space] change  [e] edit "))
    .column_spacing(2);
    frame.render_stateful_widget(table, cols[1], &mut state);
    let offset = state.offset();
    for visible in 0..values.len().saturating_sub(offset) {
        let i = offset + visible;
        let y = cols[1].y + 2 + visible as u16;
        if y < cols[1].bottom() - 1 {
            app.hits.push(Hit {
                rect: Rect::new(cols[1].x + 1, y, cols[1].width - 2, 1),
                action: ClickAction::Builder(i),
            });
        }
    }

    let text = if app.inspecting {
        "◌ Inspecting with yt-dlp…".into()
    } else if let Some(info) = &app.metadata {
        format!(
            "✓ {}  ·  {}  ·  {}",
            info.title,
            info.uploader,
            duration(info.duration)
        )
    } else {
        "Tip: Inspect fetches title, duration and formats without downloading.  [c] command preview  [o] all options".into()
    };
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(if app.inspecting { ORANGE } else { MUTED }))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(PANEL_2))
                    .padding(Padding::horizontal(1)),
            ),
        outer[2],
    );
}

fn search(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::vertical([Constraint::Length(4), Constraint::Min(8)]).split(area);
    let query_row =
        Layout::horizontal([Constraint::Min(24), Constraint::Length(15)]).split(rows[0]);
    let query = if app.search_query.is_empty() {
        "Search YouTube without an API key".to_string()
    } else {
        app.search_query.clone()
    };
    frame.render_widget(
        Paragraph::new(query)
            .style(Style::default().fg(if app.search_query.is_empty() {
                MUTED
            } else {
                TEXT
            }))
            .block(panel(" Query  [/] edit ")),
        query_row[0],
    );
    app.hits.push(Hit {
        rect: query_row[0],
        action: ClickAction::SearchInput,
    });
    button(
        frame,
        query_row[1],
        if app.searching {
            "Searching…"
        } else {
            "Search"
        },
        "s",
        CYAN,
    );
    app.hits.push(Hit {
        rect: query_row[1],
        action: ClickAction::RunSearch,
    });

    let body =
        Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)]).split(rows[1]);
    let result_rows: Vec<Row> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            Row::new([
                Cell::from(format!("{}", index + 1)),
                Cell::from(result.title.clone()),
                Cell::from(result.uploader.clone()),
                Cell::from(duration(result.duration)),
            ])
            .style(if index == app.search_index {
                Style::default()
                    .fg(BG)
                    .bg(CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(TEXT)
                    .bg(if index % 2 == 0 { PANEL } else { PANEL_2 })
            })
        })
        .collect();
    let title = if app.searching {
        " Results  searching… ".to_string()
    } else if !app.search_error.is_empty() {
        " Search unavailable  •  direct downloads unaffected ".to_string()
    } else {
        format!(" Results  {} ", app.search_results.len())
    };
    let mut table_state = TableState::default().with_selected(app.search_index);
    let table = Table::new(
        result_rows,
        [
            Constraint::Length(3),
            Constraint::Percentage(55),
            Constraint::Percentage(30),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(["#", "Title", "Channel", "Length"])
            .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(panel(&title))
    .column_spacing(1);
    frame.render_stateful_widget(table, body[0], &mut table_state);
    let offset = table_state.offset();
    for visible in 0..app.search_results.len().saturating_sub(offset) {
        let index = offset + visible;
        let y = body[0].y + 2 + visible as u16;
        if y < body[0].bottom().saturating_sub(1) {
            app.hits.push(Hit {
                rect: Rect::new(body[0].x + 1, y, body[0].width.saturating_sub(2), 1),
                action: ClickAction::SearchResult(index),
            });
        }
    }

    let compact = body[1].height < 26;
    let preview = if compact {
        Layout::vertical([
            Constraint::Length(0),
            Constraint::Length(0),
            Constraint::Min(11),
            Constraint::Length(3),
        ])
        .split(body[1])
    } else {
        Layout::vertical([
            Constraint::Length(9),
            Constraint::Length(5),
            Constraint::Min(9),
            Constraint::Length(3),
        ])
        .split(body[1])
    };
    if !compact {
        let thumb_title = format!(" Preview  {}  ·  [v] cinema view ", app.thumbnail_renderer);
        let thumb_block = panel(&thumb_title);
        let thumb_inner = thumb_block.inner(preview[0]);
        frame.render_widget(thumb_block, preview[0]);
        if let Some(protocol) = app.thumbnail.as_mut() {
            frame.render_stateful_widget(StatefulImage::default(), thumb_inner, protocol);
        } else {
            frame.render_widget(
                Paragraph::new(app.thumbnail_status.clone())
                    .style(Style::default().fg(MUTED))
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true }),
                thumb_inner,
            );
        }
    }

    let details = app
        .search_results
        .get(app.search_index)
        .map(|result| {
            let live = if result.live_status.is_empty() || result.live_status == "not_live" {
                String::new()
            } else {
                format!("  •  {}", result.live_status.replace('_', " "))
            };
            format!(
                "{}\n{}  ·  {}  ·  {} views{}\nID  {}",
                result.title,
                result.uploader,
                duration(result.duration),
                count(result.view_count),
                live,
                result.id
            )
        })
        .unwrap_or_else(|| {
            if app.search_error.is_empty() {
                "Enter a query. Search failure never disables direct URL downloads.".into()
            } else {
                format!(
                    "{}\nDirect URL downloads, queues, and history are unaffected.",
                    app.search_error
                )
            }
        });
    if !compact {
        frame.render_widget(
            Paragraph::new(details)
                .style(Style::default().fg(TEXT))
                .block(panel(" Now selected "))
                .wrap(Wrap { trim: true }),
            preview[1],
        );
    }

    let tune = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(2),
    ])
    .split(preview[2]);
    let player = Layout::horizontal([
        Constraint::Min(18),
        Constraint::Length(13),
        Constraint::Length(10),
    ])
    .split(tune[0]);
    let player_status = match app.playback_state {
        PlaybackState::Stopped => "♫  Audio preview ready",
        PlaybackState::Loading => "◌  Connecting to stream…",
        PlaybackState::Playing => "▶  Playing selected audio",
        PlaybackState::Paused => "Ⅱ  Audio preview paused",
    };
    frame.render_widget(
        Paragraph::new(player_status)
            .style(Style::default().fg(match app.playback_state {
                PlaybackState::Playing => GREEN,
                PlaybackState::Loading => ORANGE,
                PlaybackState::Paused => CYAN,
                PlaybackState::Stopped => MUTED,
            }))
            .block(panel(" Listen first ")),
        player[0],
    );
    button(
        frame,
        player[1],
        if app.playback_state == PlaybackState::Paused {
            "Resume"
        } else if app.playback_state == PlaybackState::Playing {
            "Pause"
        } else {
            "Play"
        },
        "p",
        GREEN,
    );
    button(frame, player[2], "Stop", "x", RED);
    app.hits.push(Hit {
        rect: player[1],
        action: ClickAction::TogglePlayback,
    });
    app.hits.push(Hit {
        rect: player[2],
        action: ClickAction::StopPlayback,
    });

    let video_choices = app.search_video_choices.clone();
    let audio_choices = app.search_audio_choices.clone();
    quality_slider(
        frame,
        app,
        tune[1],
        "Video  [ / ]",
        &video_choices,
        app.search_video_index,
        true,
    );
    quality_slider(
        frame,
        app,
        tune[2],
        "Audio  { / }",
        &audio_choices,
        app.search_audio_index,
        false,
    );
    let subtitles = if app.search_details_loading {
        "◌  Reading formats and captions…".to_string()
    } else if !app.search_details_error.is_empty() {
        "Format details unavailable · current preset remains usable".to_string()
    } else if !app.search_has_subtitles() {
        "○  Subtitles unavailable".to_string()
    } else if app.search_subtitles {
        format!("●  Subtitles on · {}", app.search_subtitle_label())
    } else {
        format!(
            "○  Subtitles off · {} available",
            app.search_subtitle_label()
        )
    };
    let subtitle_color = if app.search_subtitles { GREEN } else { MUTED };
    frame.render_widget(
        Paragraph::new(subtitles)
            .style(Style::default().fg(subtitle_color))
            .block(panel(" Captions  [t] toggle "))
            .wrap(Wrap { trim: true }),
        tune[3],
    );
    app.hits.push(Hit {
        rect: tune[3],
        action: ClickAction::ToggleSearchSubtitles,
    });

    let actions = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(38),
        Constraint::Percentage(37),
    ])
    .split(preview[3]);
    button(frame, actions[0], "View", "v", CYAN);
    button(frame, actions[1], "Customize", "Enter", GREEN);
    button(frame, actions[2], "Download", "a", ORANGE);
    app.hits.push(Hit {
        rect: actions[0],
        action: ClickAction::ViewSearchThumbnail,
    });
    app.hits.push(Hit {
        rect: actions[1],
        action: ClickAction::UseSearchResult,
    });
    app.hits.push(Hit {
        rect: actions[2],
        action: ClickAction::QueueSearchResult,
    });
}

fn quality_slider(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    title: &str,
    choices: &[SearchFormatChoice],
    selected: usize,
    video: bool,
) {
    let choice = choices.get(selected.min(choices.len().saturating_sub(1)));
    let dynamic_title = choice.map_or_else(
        || format!(" {title} "),
        |choice| format!(" {title}  ·  {}  ·  {} ", choice.label, choice.detail),
    );
    let block = panel(&dynamic_title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if choices.len() <= 1 {
        frame.render_widget(
            Paragraph::new("◌ fetching available qualities…").style(Style::default().fg(MUTED)),
            inner,
        );
        return;
    }
    let width = inner.width.max(1) as usize;
    let mut track = vec!['─'; width];
    for index in 0..choices.len() {
        let position = slider_position(index, choices.len(), width);
        track[position] = if index == selected { '●' } else { '◆' };
    }
    let selected_position = slider_position(selected, choices.len(), width);
    for cell in track.iter_mut().take(selected_position) {
        if *cell == '─' {
            *cell = '━';
        }
    }
    frame.render_widget(
        Paragraph::new(track.into_iter().collect::<String>())
            .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        inner,
    );
    for index in 0..choices.len() {
        let start = slider_position(index, choices.len(), inner.width as usize) as u16;
        let next = if index + 1 < choices.len() {
            slider_position(index + 1, choices.len(), inner.width as usize) as u16
        } else {
            inner.width
        };
        app.hits.push(Hit {
            rect: Rect::new(
                inner.x + start.saturating_sub(1),
                inner.y,
                next.saturating_sub(start).saturating_add(2).max(1),
                inner.height.max(1),
            ),
            action: if video {
                ClickAction::SearchVideoQuality(index)
            } else {
                ClickAction::SearchAudioQuality(index)
            },
        });
    }
}

fn slider_position(index: usize, len: usize, width: usize) -> usize {
    if len <= 1 || width <= 1 {
        0
    } else {
        index.saturating_mul(width - 1) / (len - 1)
    }
}

fn queue(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Min(8), Constraint::Length(5)]).split(area);
    let jobs: Vec<Job> = app.queue_jobs().into_iter().cloned().collect();
    let offset = job_table(frame, chunks[0], &jobs, app.queue_index, true);
    for visible in 0..jobs.len().saturating_sub(offset) {
        let i = offset + visible;
        let y = chunks[0].y + 2 + visible as u16;
        if y < chunks[0].bottom() - 1 {
            app.hits.push(Hit {
                rect: Rect::new(chunks[0].x + 1, y, chunks[0].width - 2, 1),
                action: ClickAction::Queue(i),
            });
        }
    }
    if let Some(job) = jobs.get(app.queue_index) {
        let detail_cols = Layout::horizontal([
            Constraint::Min(24),
            Constraint::Length(14),
            Constraint::Length(12),
        ])
        .split(chunks[1]);
        let details = format!(
            "{}\n{}",
            job.url,
            if job.error.is_empty() {
                job.output_path.as_str()
            } else {
                job.error.as_str()
            }
        );
        frame.render_widget(
            Paragraph::new(details)
                .style(Style::default().fg(MUTED))
                .wrap(Wrap { trim: true })
                .block(panel(
                    " Selected  [p] pause/resume  [x] cancel  [c] command ",
                )),
            detail_cols[0],
        );
        let gauge_area = Rect::new(
            detail_cols[0].x + 2,
            detail_cols[0].bottom().saturating_sub(2),
            detail_cols[0].width.saturating_sub(4),
            1,
        );
        let ratio = (job.progress.percent / 100.0).clamp(0.0, 1.0);
        let eta = job
            .progress
            .eta
            .map(|s| format!("  ETA {}:{:02}", s / 60, s % 60))
            .unwrap_or_default();
        frame.render_widget(
            Gauge::default()
                .gauge_style(
                    Style::default()
                        .fg(CYAN)
                        .bg(PANEL_2)
                        .add_modifier(Modifier::BOLD),
                )
                .ratio(ratio)
                .label(format!(
                    "{:.1}%  {}/s{eta}",
                    job.progress.percent,
                    bytes(job.progress.speed as u64)
                )),
            gauge_area,
        );
        button(frame, detail_cols[1], "Pause", "p", ORANGE);
        app.hits.push(Hit {
            rect: detail_cols[1],
            action: ClickAction::PauseResume,
        });
        button(frame, detail_cols[2], "Cancel", "x", RED);
        app.hits.push(Hit {
            rect: detail_cols[2],
            action: ClickAction::Cancel,
        });
    } else {
        frame.render_widget(
            Paragraph::new("Queue is clear. Add URLs from the Download tab.")
                .style(Style::default().fg(MUTED))
                .alignment(Alignment::Center)
                .block(panel(" Queue ")),
            chunks[1],
        );
    }
}

fn history(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Min(8), Constraint::Length(5)]).split(area);
    let jobs: Vec<Job> = app.history_jobs().into_iter().cloned().collect();
    let offset = job_table(frame, chunks[0], &jobs, app.history_index, false);
    for visible in 0..jobs.len().saturating_sub(offset) {
        let i = offset + visible;
        let y = chunks[0].y + 2 + visible as u16;
        if y < chunks[0].bottom() - 1 {
            app.hits.push(Hit {
                rect: Rect::new(chunks[0].x + 1, y, chunks[0].width - 2, 1),
                action: ClickAction::History(i),
            });
        }
    }
    let detail = jobs
        .get(app.history_index)
        .map(|j| {
            format!(
                "{}\n{}",
                j.url,
                if j.error.is_empty() {
                    j.output_path.as_str()
                } else {
                    j.error.as_str()
                }
            )
        })
        .unwrap_or_else(|| "Completed, failed and cancelled jobs appear here.".into());
    let detail_cols =
        Layout::horizontal([Constraint::Min(24), Constraint::Length(14)]).split(chunks[1]);
    frame.render_widget(
        Paragraph::new(detail)
            .style(Style::default().fg(MUTED))
            .wrap(Wrap { trim: true })
            .block(panel(" Result  [r] retry  [R] force retry  [c] command ")),
        detail_cols[0],
    );
    if !jobs.is_empty() {
        button(frame, detail_cols[1], "Retry", "r", GREEN);
        app.hits.push(Hit {
            rect: detail_cols[1],
            action: ClickAction::Retry,
        });
    }
}

fn job_table(frame: &mut Frame, area: Rect, jobs: &[Job], selected: usize, queue: bool) -> usize {
    let rows: Vec<Row> = jobs
        .iter()
        .map(|j| {
            let status_color = match j.status {
                DownloadStatus::Completed => GREEN,
                DownloadStatus::Failed | DownloadStatus::Cancelled => RED,
                DownloadStatus::Paused => ORANGE,
                _ => CYAN,
            };
            Row::new([
                Cell::from(format!("{:?}", j.status)).style(Style::default().fg(status_color)),
                Cell::from(clip(&j.title, 45)),
                Cell::from(j.preset.clone()),
                Cell::from(if queue {
                    format!(
                        "{:>5.1}%  {}",
                        j.progress.percent,
                        bytes(j.progress.speed as u64) + "/s"
                    )
                } else {
                    j.short_time()
                }),
            ])
        })
        .collect();
    let title = if queue { " Active queue " } else { " History " };
    let table = Table::new(
        rows,
        [
            Constraint::Length(13),
            Constraint::Percentage(45),
            Constraint::Length(18),
            Constraint::Min(18),
        ],
    )
    .header(
        Row::new([
            "Status",
            "Title",
            "Preset",
            if queue { "Progress" } else { "Added" },
        ])
        .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(Style::default().bg(PANEL_2).add_modifier(Modifier::BOLD))
    .highlight_symbol("› ")
    .block(panel(title));
    let mut state = TableState::default().with_selected(selected);
    frame.render_stateful_widget(table, area, &mut state);
    state.offset()
}

fn settings(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols =
        Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(area);
    let values = app.setting_values();
    let rows: Vec<Row> = values
        .iter()
        .enumerate()
        .map(|(i, (name, value))| {
            let displayed = if i == 19 && !value.is_empty() && value != "none" {
                "configured (hidden)"
            } else {
                value.as_str()
            };
            Row::new([*name, displayed]).style(if i == app.settings_index {
                Style::default()
                    .fg(BG)
                    .bg(CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(TEXT)
                    .bg(if i % 2 == 0 { PANEL } else { PANEL_2 })
            })
        })
        .collect();
    let table = Table::new(rows, [Constraint::Length(23), Constraint::Min(20)])
        .header(
            Row::new(["Setting", "Value"])
                .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
        )
        .block(panel(" Settings  [e] edit  [←→] adjust  [s] save "));
    let mut state = TableState::default().with_selected(app.settings_index);
    frame.render_stateful_widget(table, cols[0], &mut state);
    let offset = state.offset();
    for visible in 0..values.len().saturating_sub(offset) {
        let i = offset + visible;
        let y = cols[0].y + 2 + visible as u16;
        if y < cols[0].bottom() - 1 {
            app.hits.push(Hit {
                rect: Rect::new(cols[0].x + 1, y, cols[0].width - 2, 1),
                action: ClickAction::Setting(i),
            });
        }
    }
    let mut lines = vec![
        Line::styled(
            "Runtime checks",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];
    if let Some(display) = &app.display {
        lines.push(Line::from(vec![
            Span::styled("▣ ", Style::default().fg(CYAN)),
            Span::styled(
                format!(
                    "{}  {}×{} ({})",
                    display.name, display.width, display.height, display.source
                ),
                Style::default().fg(TEXT),
            ),
        ]));
    }
    for (name, ok) in &app.dependencies {
        lines.push(Line::from(vec![
            Span::styled(
                if *ok { "✓ " } else { "✗ " },
                Style::default().fg(if *ok { GREEN } else { RED }),
            ),
            Span::styled(name, Style::default().fg(TEXT)),
        ]));
    }
    lines.extend([Line::raw(""), Line::styled("Performance", Style::default().fg(TEXT).add_modifier(Modifier::BOLD)), Line::styled("Native fragments are the reliable default. aria2 can be faster on some direct transfers, but is optional.", Style::default().fg(MUTED)), Line::raw(""), Line::styled("Privacy", Style::default().fg(TEXT).add_modifier(Modifier::BOLD)), Line::styled("Cookie and proxy values are passed directly to yt-dlp and never logged by Tide.", Style::default().fg(MUTED))]);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel(" System ")),
        cols[1],
    );
}

fn logs(frame: &mut Frame, app: &mut App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let end = app
        .logs
        .len()
        .saturating_sub(app.log_scroll.min(app.logs.len()));
    let start = end.saturating_sub(visible);
    let lines: Vec<Line> = app
        .logs
        .range(start..end)
        .map(|s| {
            Line::styled(
                s.as_str(),
                Style::default().fg(if s.contains("ERROR") {
                    RED
                } else if s.contains("WARNING") {
                    ORANGE
                } else {
                    TEXT
                }),
            )
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(
                " Live yt-dlp output  [↑↓/PgUp/PgDn] scroll  [c] clear ",
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
    if app.logs.len() > visible {
        let mut state = ScrollbarState::new(app.logs.len()).position(start);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut state,
        );
    }
}

fn help(frame: &mut Frame, app: &App, area: Rect) {
    let cols =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let key_text = Text::from(vec![
        heading("Navigation"),
        kv("1…7", "switch tabs"),
        kv("Ctrl ←/→", "previous / next tab"),
        kv("?", "this help"),
        kv("q / Ctrl-C", "quit safely"),
        Line::raw(""),
        heading("Download builder"),
        kv("/", "edit URL; paste works"),
        kv("j / k", "choose preset"),
        kv("↑ / ↓", "choose option"),
        kv("← / → / Space", "change option"),
        kv("e", "edit detailed value"),
        kv("i", "inspect formats"),
        kv("a / d", "queue / queue and view"),
        kv("c", "inspect exact command"),
        kv("o", "search every yt-dlp option"),
        Line::raw(""),
        heading("YouTube search"),
        kv("/ / e", "edit query and search"),
        kv("s / r", "search / retry"),
        kv("Enter", "use selected result"),
        kv("a / d", "queue / queue and view"),
        kv("i", "inspect selected result"),
        kv("v", "full-screen viu preview"),
        kv("[ / ]", "lower / raise video quality"),
        kv("{ / }", "lower / raise audio quality"),
        kv("t", "toggle available subtitles"),
        kv("p / Space", "play or pause audio preview"),
        kv("x", "stop audio preview"),
        Line::raw(""),
        heading("Queue & history"),
        kv("p", "pause / resume process"),
        kv("x", "cancel, keeping partial"),
        kv("r", "retry safely"),
        kv("R", "retry and overwrite"),
    ]);
    frame.render_widget(
        Paragraph::new(key_text)
            .block(panel(" Keyboard & mouse "))
            .wrap(Wrap { trim: true }),
        cols[0],
    );
    let notes = Text::from(vec![
        heading("Failsafe by design"),
        Line::styled(
            "• Jobs and settings are atomically saved.\n• Interrupted work returns to the queue.\n• .part files allow byte-level continuation.\n• Archive IDs prevent duplicate downloads.\n• Existing finished files are protected.\n• Arguments run directly—never through a shell.",
            Style::default().fg(TEXT),
        ),
        Line::raw(""),
        heading("Speed without folklore"),
        Line::styled(
            "Four fragment workers are a balanced default. Raising fragment or job concurrency can help, but may increase disk contention and HTTP 429 responses. aria2 is opt-in because its benefit depends on the protocol and host.",
            Style::default().fg(MUTED),
        ),
        Line::raw(""),
        heading("Power-user reach"),
        Line::styled(
            format!(
                "The installed yt-dlp help contains {} lines. Press o to search it, then place uncommon flags in Extra arguments. Values are parsed with shell quoting but no shell is executed.",
                app.help_text.lines().count()
            ),
            Style::default().fg(MUTED),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(notes)
            .block(panel(" How Tide behaves "))
            .wrap(Wrap { trim: true }),
        cols[1],
    );
}

fn footer(frame: &mut Frame, app: &App, area: Rect) {
    let style = if app.notice.to_ascii_lowercase().contains("could not")
        || app.notice.to_ascii_lowercase().contains("stopped")
    {
        Style::default().fg(RED)
    } else {
        Style::default().fg(MUTED)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("  ›  {}", app.notice), style),
            Span::styled("     ? help   q quit", Style::default().fg(MUTED)),
        ]))
        .style(Style::default().bg(PANEL)),
        area,
    );
}

fn modal(frame: &mut Frame, app: &mut App) {
    let Some(modal) = app.modal.clone() else {
        return;
    };
    // The overlay owns mouse input; controls underneath must not be clickable.
    app.hits.clear();
    let area = match modal {
        Modal::Explorer { .. } => centered(86, 82, frame.area()),
        Modal::Inspect => centered(84, 75, frame.area()),
        _ => centered(76, 42, frame.area()),
    };
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(PANEL)), area);
    match modal {
        Modal::Edit { title, value, cursor, secret, .. } => {
            let plain = if secret && !value.is_empty() { "•".repeat(value.chars().count()) } else { value };
            let split = plain.char_indices().nth(cursor).map(|(i, _)| i).unwrap_or(plain.len());
            let shown = format!("{}█{}", &plain[..split], &plain[split..]);
            frame.render_widget(Paragraph::new(vec![Line::styled("Type a value, then press Enter to apply.  Ctrl-U clears; Ctrl-W deletes a word.", Style::default().fg(MUTED)), Line::raw(""), Line::styled(format!("> {shown}"), Style::default().fg(TEXT).bg(PANEL_2))]).wrap(Wrap { trim: false }).block(dialog(&format!(" {title} "), " Enter apply  •  Esc cancel ")), area);
        }
        Modal::Explorer { query, scroll } => {
            let q = query.to_ascii_lowercase();
            let matches: Vec<&str> = app.help_text.lines().filter(|l| q.is_empty() || l.to_ascii_lowercase().contains(&q)).collect();
            let visible = area.height.saturating_sub(6) as usize;
            let lines: Vec<Line> = matches.iter().skip(scroll).take(visible).map(|l| Line::styled(*l, Style::default().fg(if l.trim_start().starts_with('-') { CYAN } else { TEXT }))).collect();
            let body = Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).split(area.inner(Margin { vertical: 1, horizontal: 1 }));
            frame.render_widget(Paragraph::new(format!("Search: {query}█   {} matches", matches.len())).style(Style::default().fg(TEXT).bg(PANEL_2)).block(Block::default().padding(Padding::horizontal(1))), body[0]);
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body[1]);
            frame.render_widget(dialog(" All installed yt-dlp options ", " Type to filter  •  ↑↓ scroll  •  Enter/Esc close "), area);
        }
        Modal::Inspect => {
            if let Some(info) = &app.metadata {
                let chunks = Layout::vertical([Constraint::Length(6), Constraint::Min(4)]).split(area.inner(Margin { vertical: 1, horizontal: 1 }));
                let summary = format!("{}\n{}  •  {}  •  {}\n{}", info.title, info.uploader, duration(info.duration), if info.live_status.is_empty() { "on demand" } else { &info.live_status }, if info.webpage_url.is_empty() { &info.id } else { &info.webpage_url });
                frame.render_widget(Paragraph::new(summary).style(Style::default().fg(TEXT)).wrap(Wrap { trim: true }), chunks[0]);
                let rows: Vec<Row> = info.formats.iter().rev().map(|f| Row::new(vec![Cell::from(f.format_id.clone()), Cell::from(if f.resolution.is_empty() { f.note.clone() } else { f.resolution.clone() }), Cell::from(f.ext.clone()), Cell::from(codec(&f.vcodec, &f.acodec)), Cell::from(bytes(f.filesize.or(f.filesize_approx).unwrap_or(0)))])).collect();
                let mut state = TableState::default().with_selected(app.format_index);
                frame.render_stateful_widget(Table::new(rows, [Constraint::Length(9), Constraint::Length(15), Constraint::Length(7), Constraint::Min(18), Constraint::Length(10)]).header(Row::new(["ID", "Resolution", "Ext", "Codecs", "Size≈"]).style(Style::default().fg(MUTED))).row_highlight_style(Style::default().fg(BG).bg(CYAN).add_modifier(Modifier::BOLD)).highlight_symbol("› ").block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(PANEL_2))), chunks[1], &mut state);
            }
            frame.render_widget(dialog(" Media inspection ", " ↑↓ select  •  f use exact format  •  Enter/Esc close "), area);
        }
        Modal::Command(command) => frame.render_widget(Paragraph::new(command).style(Style::default().fg(GREEN)).wrap(Wrap { trim: false }).block(dialog(" Exact command ", " Enter/Esc close ")), area),
        Modal::ConfirmCancel(_) => frame.render_widget(Paragraph::new("Cancel this download?\n\nThe process will stop, but .part files remain so a retry can continue from the downloaded bytes.").style(Style::default().fg(TEXT)).alignment(Alignment::Center).wrap(Wrap { trim: true }).block(dialog(" Confirm cancel ", " y/Enter cancel  •  n/Esc keep running ")), area),
        Modal::Error(error) => frame.render_widget(Paragraph::new(error).style(Style::default().fg(RED)).wrap(Wrap { trim: true }).block(dialog(" Something needs attention ", " Enter/Esc close ")), area),
    }
    let close = Rect::new(area.right().saturating_sub(4), area.y + 1, 3, 1);
    frame.render_widget(
        Paragraph::new(" × ").style(Style::default().fg(BG).bg(RED).add_modifier(Modifier::BOLD)),
        close,
    );
    app.hits.push(Hit {
        rect: close,
        action: ClickAction::CloseModal,
    });
}

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(43, 60, 78)))
        .style(Style::default().bg(PANEL))
        .padding(Padding::horizontal(1))
}

fn tab_caption(tab: Tab) -> &'static str {
    match tab {
        Tab::Download => "↓ Download",
        Tab::Search => "⌕ Search",
        Tab::Queue => "≡ Queue",
        Tab::History => "◷ History",
        Tab::Settings => "⚙ Settings",
        Tab::Logs => "≣ Logs",
        Tab::Help => "? Help",
    }
}
fn dialog<'a>(title: &'a str, footer: &'a str) -> Block<'a> {
    Block::default()
        .title(Line::styled(
            title,
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::styled(footer, Style::default().fg(MUTED)))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(CYAN))
        .style(Style::default().bg(PANEL))
        .padding(Padding::new(2, 2, 1, 1))
}
fn button(frame: &mut Frame, area: Rect, label: &str, key: &str, color: Color) {
    frame.render_widget(
        Paragraph::new(
            Line::from(vec![
                Span::styled(label, Style::default().fg(BG).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" [{key}]"),
                    Style::default().fg(Color::Rgb(45, 60, 70)),
                ),
            ])
            .alignment(Alignment::Center),
        )
        .style(Style::default().bg(color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(color)),
        ),
        area,
    );
}
fn centered(width_pct: u16, height_pct: u16, r: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - height_pct) / 2),
        Constraint::Percentage(height_pct),
        Constraint::Percentage((100 - height_pct) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .split(v[1])[1]
}
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.into()
    } else {
        format!(
            "{}…",
            s.chars().take(n.saturating_sub(1)).collect::<String>()
        )
    }
}
fn bytes(v: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut x = v as f64;
    let mut i = 0;
    while x >= 1024.0 && i < U.len() - 1 {
        x /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v} {}", U[i])
    } else {
        format!("{x:.1} {}", U[i])
    }
}
fn duration(v: Option<f64>) -> String {
    let s = v.unwrap_or(0.0) as u64;
    if s == 0 {
        "unknown duration".into()
    } else {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    }
}

fn count(value: Option<u64>) -> String {
    let Some(value) = value else {
        return "unknown".into();
    };
    match value {
        1_000_000_000.. => format!("{:.1}B", value as f64 / 1_000_000_000.0),
        1_000_000.. => format!("{:.1}M", value as f64 / 1_000_000.0),
        1_000.. => format!("{:.1}K", value as f64 / 1_000.0),
        _ => value.to_string(),
    }
}
fn codec(v: &str, a: &str) -> String {
    match (v == "none", a == "none") {
        (true, false) => format!("audio {a}"),
        (false, true) => format!("video {v}"),
        _ => format!("{v} + {a}"),
    }
}
fn heading(s: &str) -> Line<'_> {
    Line::styled(s, Style::default().fg(CYAN).add_modifier(Modifier::BOLD))
}
fn kv<'a>(key: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{key:12}"),
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(value, Style::default().fg(TEXT)),
    ])
}
