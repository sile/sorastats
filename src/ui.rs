use crate::poll::StatsReceiver;
use crate::stats::Stats;
use crate::Options;
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Focus {
    AggregatedStats,
    ConnectionStats,
}

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;

type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

// TODO: rename
#[derive(Debug)]
pub struct Ui {
    options: Options,
    history: VecDeque<Stats>,
    table_state: tui::widgets::TableState, // TODO: rename
    connection_table_state: tui::widgets::TableState, // TODO: rename
    focus: Focus,
}

impl Ui {
    fn latest_stats(&self) -> &Stats {
        &self.history.back().expect("unreachable")
    }

    fn new(options: Options) -> Self {
        let mut table_state = tui::widgets::TableState::default();
        table_state.select(Some(0));

        let mut connection_table_state = tui::widgets::TableState::default();
        connection_table_state.select(Some(0));

        Self {
            options,
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

        let stats = self.latest_stats();
        let paragraph = Paragraph::new(vec![
            Spans::from(format!(
                "Update Time: {}",
                chrono::DateTime::<chrono::Local>::from(stats.time)
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
            )),
            Spans::from(format!(
                "Connections: {:5} (filter={})",
                stats.connection_count(),
                self.options.connection_filter
            )),
            Spans::from(format!(
                "Stats  Keys: {:5} (filter={})",
                stats.item_count(),
                self.options.stats_key_filter
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

    fn selected_item_chart(&self, selected: &str) -> Vec<(f64, f64)> {
        let mut items = Vec::new();
        let start = self.history[0].timestamp;
        for stats in &self.history {
            let x = (stats.timestamp - start).as_secs_f64();
            if let Some(y) = stats
                .aggregated
                .items
                .get(selected)
                .and_then(|x| x.delta_per_sec)
            {
                items.push((x, y));
            }
        }
        items
    }

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

        let items = &self.latest_stats().aggregated.items;

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
                .latest_stats()
                .aggregated
                .items
                .keys()
                .nth(i)
                .expect("TODO: range check")
                .to_owned();
            self.draw_selected_stats(f, chunks[0], &selected);
            self.draw_chart(f, chunks[1], &selected);
        } else {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Detailed Stats");
            f.render_widget(block, area);
        }
    }

    fn draw_chart(&mut self, f: &mut Frame, area: tui::layout::Rect, selected: &str) {
        use tui::symbols::Marker;
        use tui::text::Span;
        use tui::widgets::{Axis, Chart, Dataset, GraphType};
        // TODO: Support individual chart

        let items = self.selected_item_chart(selected);
        if items.is_empty() {
            f.render_widget(self.make_block("Delta/s Chart", None), area);
            return;
        }

        let datasets = vec![Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .data(&items)];

        // TODO;
        let mut lower_bound = 0.0; // TODO: min
        let mut upper_bound = items
            .iter()
            .map(|(_, y)| *y)
            .max_by(|y0, y1| y0.partial_cmp(&y1).unwrap())
            .unwrap();
        //* 1.1;
        upper_bound = upper_bound.ceil();
        if lower_bound == 0.0 && upper_bound == 0.0 {
            lower_bound = 0.0;
            upper_bound = 1.0;
        }

        let chart = Chart::new(datasets)
            .block(self.make_block("Delta/s Chart", None))
            .x_axis(
                Axis::default()
                    .labels(vec![
                        Span::from("0s"),
                        Span::from(format!("{}s", self.options.chart_time_period.get())),
                    ])
                    .bounds([0.0, self.options.chart_time_period.get() as f64]),
            )
            .y_axis(
                Axis::default()
                    // TODO: format_u64
                    .labels(vec![Span::from("0"), Span::from(upper_bound.to_string())])
                    .bounds([lower_bound, upper_bound]),
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
            if let Some((_, item)) = conn.items.iter().find(|(k, _)| *k == selected) {
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
    pub fn new(rx: StatsReceiver, options: Options) -> anyhow::Result<Self> {
        let terminal = Self::setup_terminal()?;
        log::debug!("setup terminal");
        let ui = Ui::new(options);
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
                let timestamp = stats.timestamp;
                self.ui.history.push_back(stats);
                while let Some(item) = self.ui.history.pop_front() {
                    if (timestamp - item.timestamp).as_secs()
                        <= self.ui.options.chart_time_period.get() as u64
                    {
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
