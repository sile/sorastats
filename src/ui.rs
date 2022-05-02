use crate::poller::StatsReceiver;
use crate::stats::Stats2;
use chrono::{DateTime, Local};
use clap::Parser;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Focus {
    AggregatedStats,
    ConnectionStats,
}

#[derive(Debug, Parser)]
pub struct UiOpts {
    // TODO: rename
    #[clap(long, default_value_t = 600.0)]
    pub retention_period: f64,
}

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;

type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

// TODO: rename
#[derive(Debug)]
pub struct Ui {
    opt: UiOpts,
    history: VecDeque<HistoryItem>,
    table_state: tui::widgets::TableState, // TODO: rename
    connection_table_state: tui::widgets::TableState, // TODO: rename
    focus: Focus,
}

impl Ui {
    fn latest_stats(&self) -> &Stats2 {
        &self.history.back().expect("unreachable").stats
    }

    fn new(opt: UiOpts) -> Self {
        let mut table_state = tui::widgets::TableState::default();
        table_state.select(Some(0));

        let mut connection_table_state = tui::widgets::TableState::default();
        connection_table_state.select(Some(0));

        Self {
            opt,
            history: VecDeque::new(),
            table_state,
            connection_table_state,
            focus: Focus::AggregatedStats,
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        use tui::layout::{Constraint, Direction, Layout};

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(0)].as_ref())
            .split(f.size());

        self.render_header(f, chunks[0]);
        self.render_body(f, chunks[1]);
    }

    fn render_header(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::{Constraint, Direction, Layout};

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area);

        self.render_status(f, chunks[0]);
        self.draw_help(f, chunks[1]);
    }

    fn make_block(&self, name: &str, block: Option<Focus>) -> tui::widgets::Block<'static> {
        use tui::style::{Modifier, Style};
        use tui::text::Span;
        use tui::widgets::{Block, Borders};

        if block == Some(self.focus) {
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    name.to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().add_modifier(Modifier::BOLD))
        } else {
            Block::default().borders(Borders::ALL).title(Span::styled(
                name.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ))
        }
    }

    fn render_status(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::Alignment;
        use tui::text::Spans;
        use tui::widgets::Paragraph;

        let item = self.history.back().expect("unreachable");
        let paragraph = Paragraph::new(vec![
            Spans::from(format!(
                "Update Time: {}",
                item.time
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
            )),
            Spans::from(format!(
                "Connections: {:5} (filter={})",
                item.stats.connection_count(),
                "TODO" // self.opt.connection_filter
            )),
            Spans::from(format!(
                "Stats  Keys: {:5} (filter={})",
                item.stats.item_count(),
                "TODO" //self.opt.stats_key_filter
            )),
        ])
        .block(self.make_block("Status", None))
        .alignment(Alignment::Left);
        f.render_widget(paragraph, area);
    }

    fn render_body(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::{Constraint, Direction, Layout};

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area);

        self.draw_aggregated_stats(f, chunks[0]);
        self.draw_detailed_stats(f, chunks[1]);
    }

    fn selected_item_chart(&self, _selected: &str) -> Vec<(f64, f64)> {
        // TODO
        // let mut items = Vec::new();
        // let start = self.history[0].timestamp; // TODO
        // for history_item in &self.history {
        //     let x = (history_item.timestamp - start).as_secs_f64();
        //     let mut y = 0.0;
        //     for conn in &self.history.back().expect("unreachable").connections {
        //         for (k, v) in &conn.stats {
        //             if let StatsValue::Number(v) = v {
        //                 if *k == selected.key {
        //                     y += v.0;
        //                     break;
        //                 }
        //             }
        //         }
        //     }
        //     items.push((x, y));
        // }
        // items // TODO: use delta instead of sum
        Vec::new()
    }

    // TODO: delete
    // fn selected_item_values(&self, selected: &str) -> Vec<(String, String)> {
    //     let mut items = Vec::new();
    //     for conn in self
    //         .history
    //         .back()
    //         .expect("unreachable")
    //         .stats
    //         .connections
    //         .values()
    //     {
    //         for (k, v) in &conn.stats {
    //             if k == selected {
    //                 items.push((conn.connection_id.clone(), v.value.to_string()));
    //                 break;
    //             }
    //         }
    //     }
    //     // TODO: sort
    //     items
    // }

    fn draw_aggregated_stats(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::Constraint;
        use tui::style::{Modifier, Style};
        use tui::widgets::{Cell, Row, Table};

        let selected_style = if self.focus == Focus::AggregatedStats {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let normal_style = Style::default();

        let header_cells = ["Key", "Sum", "Delta/s"]
            .into_iter()
            .map(|h| Cell::from(h).style(Style::default().add_modifier(Modifier::BOLD)));
        let header = Row::new(header_cells).style(normal_style).bottom_margin(1);

        let items = &self
            .history
            .back()
            .expect("unreachable")
            .stats
            .aggregated
            .stats;

        // TODO: optimize
        let sum_width = items
            .iter()
            .map(|(_, item)| item.format_value_sum().len())
            .max()
            .unwrap_or(0);
        let delta_width = items
            .iter()
            .map(|(_, item)| item.format_delta_per_sec().len())
            .max()
            .unwrap_or(0);

        let rows = items.iter().map(|(k, item)| {
            let cells = vec![
                Cell::from(k.clone()),
                Cell::from(format!("{:>sum_width$}", item.format_value_sum())),
                Cell::from(format!("{:>delta_width$}", item.format_delta_per_sec())),
            ];
            Row::new(cells)
        });

        let widths = [
            Constraint::Percentage(60),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ];

        let highlight_symbol = format!(
            "{:>width$}> ",
            self.table_state.selected().unwrap_or(0) + 1,
            width = (self.latest_stats().item_count()).to_string().len()
        );

        let t = Table::new(rows)
            .header(header)
            .block(self.make_block("Aggregated Stats", Some(Focus::AggregatedStats)))
            .highlight_style(selected_style)
            .highlight_symbol(&highlight_symbol)
            .widths(&widths);
        f.render_stateful_widget(t, area, &mut self.table_state);
    }

    // TODO: rename
    fn draw_detailed_stats(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::{Constraint, Direction, Layout};
        use tui::widgets::{Block, Borders};

        if let Some(i) = self.table_state.selected() {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(area);

            let selected = self
                .history
                .back()
                .expect("unreachable")
                .stats
                .aggregated
                .stats
                .keys()
                .nth(i)
                .expect("TODO: range check")
                .to_owned();
            self.draw_selected_stats(f, chunks[0], &selected);
            if false {
                self.draw_chart(f, chunks[1], &selected);
            }
        } else {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Detailed Stats");
            f.render_widget(block, area);
        }
    }

    fn draw_chart(&mut self, f: &mut Frame, area: tui::layout::Rect, selected: &str) {
        use tui::style::{Color, Modifier, Style};
        use tui::symbols::Marker;
        use tui::text::Span;
        use tui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType};

        let items = self.selected_item_chart(selected);

        // TODO: add average
        let datasets = vec![Dataset::default()
            // .name(ty)
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .data(&items)];

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title(Span::styled(
                        "Chart",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .title(format!(
                        "Time (duration: {} seconds)",
                        self.opt.retention_period
                    ))
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, self.opt.retention_period]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::default().fg(Color::Gray))
                    // TODO
                    // .labels(vec![
                    //     tui::text::Span::styled(
                    //         "0",
                    //         tui::style::Style::default().add_modifier(tui::style::Modifier::BOLD),
                    //     ),
                    //     tui::text::Span::styled(
                    //         "50",
                    //         tui::style::Style::default().add_modifier(tui::style::Modifier::BOLD),
                    //     ),
                    //     tui::text::Span::styled(
                    //         "100",
                    //         tui::style::Style::default().add_modifier(tui::style::Modifier::BOLD),
                    //     ),
                    // ])
                    .bounds([
                        // TODO
                        items
                            .iter()
                            .map(|(_, y)| *y)
                            .min_by(|y0, y1| y0.partial_cmp(&y1).unwrap())
                            .unwrap()
                            * 0.99,
                        items
                            .iter()
                            .map(|(_, y)| *y)
                            .max_by(|y0, y1| y0.partial_cmp(&y1).unwrap())
                            .unwrap()
                            * 1.01,
                    ]),
            );
        f.render_widget(chart, area);
    }

    fn draw_selected_stats(&mut self, f: &mut Frame, area: tui::layout::Rect, selected: &str) {
        use tui::layout::Constraint;
        use tui::style::{Modifier, Style};
        use tui::widgets::{Cell, Row, Table};

        let mut rows = Vec::new();
        let mut value_width = 0;
        let mut delta_width = 0;
        let mut is_value_num = true;
        for conn in self.latest_stats().connections.values() {
            if let Some((_, item)) = conn.stats.iter().find(|(k, _)| *k == selected) {
                let value = item.format_value();
                let delta = item.format_delta_per_sec();
                is_value_num &= item.value.as_f64().is_some();
                value_width = std::cmp::max(value_width, value.len());
                delta_width = std::cmp::max(delta_width, delta.len());
                rows.push((conn.connection_id.clone(), value, delta));
            }
        }

        let rows = rows.into_iter().map(|(connection_id, value, delta)| {
            if is_value_num {
                Row::new(vec![
                    Cell::from(connection_id),
                    Cell::from(format!("{:>value_width$}", value)),
                    Cell::from(format!("{:>delta_width$}", delta)),
                ])
            } else {
                Row::new(vec![Cell::from(connection_id), Cell::from(value)])
            }
        });

        let selected_style = if self.focus == Focus::ConnectionStats {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let normal_style = Style::default();

        let header_cells = if is_value_num {
            &["Connection ID", "Value", "Delta/s"][..]
        } else {
            &["Connection ID", "Value"][..]
        }
        .into_iter()
        .map(|&h| Cell::from(h).style(Style::default().add_modifier(Modifier::BOLD)));
        let header = Row::new(header_cells)
            .style(normal_style)
            .height(1)
            .bottom_margin(1);

        let widths = if is_value_num {
            vec![
                Constraint::Percentage(50),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ]
        } else {
            vec![Constraint::Percentage(50), Constraint::Percentage(50)]
        };

        let highlight_symbol = if self.focus == Focus::AggregatedStats {
            format!(
                "{:>width$}  ",
                "",
                width = (self.latest_stats().connection_count()).to_string().len()
            )
        } else {
            format!(
                "{:>width$}> ",
                self.connection_table_state.selected().unwrap_or(0) + 1,
                width = (self.latest_stats().connection_count()).to_string().len()
            )
        };

        let t = Table::new(rows)
            .header(header)
            .block(self.make_block(
                &format!("Values of {:?}", selected),
                Some(Focus::ConnectionStats),
            ))
            .highlight_style(selected_style)
            .highlight_symbol(&highlight_symbol)
            .widths(&widths);
        f.render_stateful_widget(t, area, &mut self.connection_table_state);
    }

    // TODO: rename
    fn draw_help(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::Alignment;
        use tui::text::Spans;
        use tui::widgets::Paragraph;

        let paragraph = Paragraph::new(vec![
            Spans::from("Quit: 'q' key"),
            Spans::from("Move: UP / DOWN / LEFT / RIGHT keys"),
        ])
        .block(self.make_block("Help", None))
        .alignment(Alignment::Left);
        f.render_widget(paragraph, area);
    }
}

pub struct App {
    rx: StatsReceiver,
    terminal: Terminal,
    ui: Ui,
}

impl App {
    pub fn new(rx: StatsReceiver, opt: UiOpts) -> anyhow::Result<Self> {
        let terminal = Self::setup_terminal()?;
        log::debug!("setup terminal");
        let ui = Ui::new(opt);
        Ok(Self { rx, ui, terminal })
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        loop {
            if self.handle_key_event()? {
                break;
            }
            self.handle_stats_poll()?;
        }
        Ok(())
    }

    fn handle_key_event(&mut self) -> anyhow::Result<bool> {
        if crossterm::event::poll(std::time::Duration::from_secs(0))? {
            // TODO: handle resize event
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('q') => {
                        return Ok(true);
                    }
                    KeyCode::Left => {
                        self.ui.focus = Focus::AggregatedStats;
                        self.terminal.draw(|f| self.ui.draw(f))?;
                    }
                    KeyCode::Right => {
                        self.ui.focus = Focus::ConnectionStats;
                        self.terminal.draw(|f| self.ui.draw(f))?;
                    }

                    KeyCode::Up => {
                        // TODO: zero items check
                        let table = if self.ui.focus == Focus::AggregatedStats {
                            &mut self.ui.table_state
                        } else {
                            &mut self.ui.connection_table_state
                        };
                        let i = if let Some(i) = table.selected() {
                            i.saturating_sub(1)
                        } else {
                            0
                        };
                        table.select(Some(i));
                        self.terminal.draw(|f| self.ui.draw(f))?;
                    }
                    KeyCode::Down => {
                        // TODO: zero items check
                        let (max, table) = if self.ui.focus == Focus::AggregatedStats {
                            (
                                self.ui.latest_stats().item_count(),
                                &mut self.ui.table_state,
                            )
                        } else {
                            (
                                self.ui.latest_stats().connection_count(),
                                &mut self.ui.connection_table_state,
                            )
                        };

                        let i = if let Some(i) = table.selected() {
                            if self.ui.focus == Focus::AggregatedStats {
                                std::cmp::min(i + 1, max - 1)
                            } else {
                                std::cmp::min(i + 1, max - 1)
                            }
                        } else {
                            0
                        };
                        table.select(Some(i));
                        self.terminal.draw(|f| self.ui.draw(f))?;
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    fn handle_stats_poll(&mut self) -> anyhow::Result<()> {
        match self.rx.recv_timeout(Duration::from_millis(10)) {
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("Sora stats polling thread terminated unexpectedly");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Ok(stats) => {
                log::debug!("recv new stats");
                self.ui.history.push_back(HistoryItem {
                    timestamp: Instant::now(),
                    time: Local::now(),
                    stats,
                });
                while let Some(item) = self.ui.history.pop_front() {
                    if item.timestamp.elapsed().as_secs_f64() < self.ui.opt.retention_period {
                        self.ui.history.push_front(item);
                        break;
                    }
                    log::debug!("remove old stats");
                }
                self.terminal.draw(|f| self.ui.draw(f))?;
            }
        }
        Ok(())
    }

    fn setup_terminal() -> anyhow::Result<Terminal> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        crossterm::execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        let backend = tui::backend::CrosstermBackend::new(stdout);
        let terminal = tui::Terminal::new(backend)?;
        Ok(terminal)
    }

    fn teardown_terminal(&mut self) -> anyhow::Result<()> {
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Err(e) = self.teardown_terminal() {
            log::warn!("failed to tear down terminal: {e}");
        } else {
            log::debug!("tear down terminal");
        }
    }
}

#[derive(Debug)]
pub struct HistoryItem {
    timestamp: Instant,    // TODO: delete(?)
    time: DateTime<Local>, // TODO: delete
    stats: Stats2,
}
