use crate::poll::StatsReceiver;
use crate::stats::{format_u64, Stats};
use crate::Options;
use crossterm::event::{KeyCode, KeyEvent};
use orfail::OrFail;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table, TableState,
};
use ratatui::Frame;
use std::collections::VecDeque;
use std::time::Duration;

type Terminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>;

pub struct App {
    rx: StatsReceiver,
    terminal: Terminal,
    ui: UiState,
}

impl App {
    pub fn new(rx: StatsReceiver, options: Options) -> orfail::Result<Self> {
        let terminal = Self::setup_terminal()?;
        log::debug!("setup terminal");
        let ui = UiState::new(options);
        Ok(Self { rx, ui, terminal })
    }

    pub fn run(mut self) -> orfail::Result<()> {
        if !self.ui.realtime {
            self.handle_replay_stats_poll()?;
        }

        loop {
            if self.handle_event()? {
                break;
            }
            if self.ui.realtime {
                if self.ui.pause {
                    std::thread::sleep(self.recv_timeout());
                } else {
                    self.handle_realtime_stats_poll()?;
                }
            }
        }
        Ok(())
    }

    fn recv_timeout(&self) -> Duration {
        Duration::from_millis(10)
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> orfail::Result<bool> {
        match key.code {
            KeyCode::Char('q') => {
                return Ok(true);
            }
            KeyCode::Char('p') => {
                if self.ui.realtime {
                    self.ui.pause = !self.ui.pause;
                }
            }
            KeyCode::Char('l') => {
                if !self.ui.realtime {
                    self.handle_replay_stats_poll()?;
                }
            }
            KeyCode::Char('h') => {
                self.ui.end_pos = std::cmp::max(1, self.ui.end_pos.saturating_sub(1));
            }
            KeyCode::Left => {
                self.ui.focus = Focus::AggregatedStats;
            }
            KeyCode::Right => {
                self.ui.focus = Focus::IndividualStats;
            }
            KeyCode::Up => {
                let table = if self.ui.focus == Focus::AggregatedStats {
                    &mut self.ui.aggregated_table_state
                } else {
                    &mut self.ui.individual_table_state
                };

                let i = table.selected().unwrap_or(0).saturating_sub(1);
                table.select(Some(i));
                self.ui.ensure_table_indices_are_in_ranges();
            }
            KeyCode::Down => {
                let table = if self.ui.focus == Focus::AggregatedStats {
                    &mut self.ui.aggregated_table_state
                } else {
                    &mut self.ui.individual_table_state
                };

                let i = table.selected().unwrap_or(0) + 1;
                table.select(Some(i));
                self.ui.ensure_table_indices_are_in_ranges();
            }
            _ => {
                return Ok(false);
            }
        }
        self.terminal.draw(|f| self.ui.render(f)).or_fail()?;
        Ok(false)
    }

    fn handle_event(&mut self) -> orfail::Result<bool> {
        while crossterm::event::poll(std::time::Duration::from_secs(0)).or_fail()? {
            match crossterm::event::read().or_fail()? {
                crossterm::event::Event::Key(key) => {
                    if self.handle_key_event(key)? {
                        return Ok(true);
                    }
                }
                crossterm::event::Event::Resize(_, _) => {
                    self.terminal.draw(|f| self.ui.render(f)).or_fail()?;
                }
                _ => {}
            }
        }
        Ok(false)
    }

    fn handle_replay_stats_poll(&mut self) -> orfail::Result<()> {
        if self.ui.end_pos < self.ui.history.len() {
            self.ui.end_pos += 1;
        } else if let Ok(stats) = self.rx.recv() {
            log::debug!("recv new stats");
            self.ui.history.push_back(stats);
            self.ui.end_pos += 1;
        } else {
            self.ui.eof = true;
        }

        self.ui.ensure_table_indices_are_in_ranges();
        self.terminal.draw(|f| self.ui.render(f)).or_fail()?;

        Ok(())
    }

    fn handle_realtime_stats_poll(&mut self) -> orfail::Result<()> {
        match self.rx.recv_timeout(self.recv_timeout()) {
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(orfail::Failure::new(
                    "Sora stats polling thread terminated unexpectedly",
                ));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Ok(stats) => {
                log::debug!("recv new stats");
                let timestamp = stats.timestamp;
                self.ui.history.push_back(stats);
                while let Some(item) = self.ui.history.pop_front() {
                    let duration = (timestamp - item.timestamp).as_secs();
                    if duration <= self.ui.options.chart_time_period.get() as u64 {
                        self.ui.history.push_front(item);
                        break;
                    }
                    log::debug!("remove old stats");
                }
                self.ui.ensure_table_indices_are_in_ranges();
                self.terminal.draw(|f| self.ui.render(f)).or_fail()?;
            }
        }
        Ok(())
    }

    fn setup_terminal() -> orfail::Result<Terminal> {
        crossterm::terminal::enable_raw_mode().or_fail()?;
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen).or_fail()?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let terminal = ratatui::Terminal::new(backend).or_fail()?;
        Ok(terminal)
    }

    fn teardown_terminal(&mut self) -> orfail::Result<()> {
        crossterm::terminal::disable_raw_mode().or_fail()?;
        crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
        )
        .or_fail()?;
        self.terminal.show_cursor().or_fail()?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    AggregatedStats,
    IndividualStats,
}

#[derive(Debug)]
struct UiState {
    options: Options,
    history: VecDeque<Stats>,
    aggregated_table_state: TableState,
    individual_table_state: TableState,
    focus: Focus,
    pause: bool,
    realtime: bool,

    // For replay mode
    eof: bool,
    end_pos: usize,
}

impl UiState {
    fn new(options: Options) -> Self {
        let realtime = options.is_realtime_mode();
        Self {
            options,
            history: VecDeque::new(),
            aggregated_table_state: TableState::default(),
            individual_table_state: TableState::default(),
            focus: Focus::AggregatedStats,
            pause: false,
            realtime,
            eof: false,
            end_pos: 0,
        }
    }

    fn latest_stats(&self) -> &Stats {
        if self.realtime {
            self.history.back().expect("unreachable")
        } else {
            &self.history[self.end_pos - 1]
        }
    }

    #[allow(clippy::iter_skip_zero)]
    fn history_window(&self) -> (Duration, impl Iterator<Item = &Stats>) {
        if self.realtime {
            let start = self.history[0].timestamp;
            (start, self.history.iter().take(self.history.len()).skip(0))
        } else {
            let mut start_pos = self.end_pos - 1;
            let timestamp = self.latest_stats().timestamp;
            while start_pos > 0 {
                let duration = (timestamp - self.history[start_pos].timestamp).as_secs_f64();
                if duration > self.options.chart_time_period.get() as f64 {
                    start_pos += 1;
                    break;
                }
                start_pos -= 1;
            }
            let start = self.history[start_pos].timestamp;
            (
                start,
                self.history.iter().take(self.end_pos).skip(start_pos),
            )
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(5),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(f.size());

        self.render_header(f, chunks[0]);
        self.render_body(f, chunks[1]);
        self.render_footer(f, chunks[2]);
    }

    fn render_header(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area);

        self.render_status(f, chunks[0]);
        self.render_help(f, chunks[1]);
    }

    fn render_footer(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let mut text = vec![];
        if let Some(key) = self.selected_item_key() {
            text.push(Line::from(format!("[KEY] {}", key)));
        }

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Left);
        f.render_widget(paragraph, area);
    }

    fn render_status(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let block = if self.pause {
            self.make_block("Status (PAUSED)", None)
        } else if !self.realtime {
            if self.eof && self.end_pos == self.history.len() {
                self.make_block("Status (REPLAY, EOF)", None)
            } else {
                self.make_block("Status (REPLAY)", None)
            }
        } else {
            self.make_block("Status", None)
        };

        let stats = self.latest_stats();
        let paragraph = Paragraph::new(vec![
            Line::from(format!(
                "Update Time: {}",
                chrono::DateTime::<chrono::Local>::from(stats.time)
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
            )),
            Line::from(format!(
                "Connections: {:5} (filter={})",
                stats.connection_count(),
                self.options.connection_filter
            )),
            Line::from(format!(
                "Stats  Keys: {:5} (filter={})",
                stats.item_count(),
                self.options.stats_key_filter
            )),
        ])
        .block(block)
        .alignment(Alignment::Left);
        f.render_widget(paragraph, area);
    }

    fn render_help(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let paragraph = Paragraph::new(vec![
            Line::from("Quit:           'q' key"),
            if self.realtime {
                Line::from("Pause / Resume: 'p' key")
            } else {
                Line::from("Prev / Next:    'h' / 'l' keys")
            },
            Line::from("Move:           UP / DOWN / LEFT / RIGHT keys"),
        ])
        .block(self.make_block("Help", None))
        .alignment(Alignment::Left);
        f.render_widget(paragraph, area);
    }

    fn render_body(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area);

        self.render_aggregated_stats(f, chunks[0]);
        self.render_details(f, chunks[1]);
    }

    fn render_aggregated_stats(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let header_cells = ["Key", "Sum", "Delta/s"]
            .into_iter()
            .map(|h| Cell::from(h).style(Style::default().add_modifier(Modifier::BOLD)));
        let header = Row::new(header_cells).bottom_margin(1);

        let mut sum_width = 0;
        let mut delta_width = 0;
        let mut row_items = Vec::with_capacity(self.latest_stats().aggregated.items.len());
        for (k, item) in &self.latest_stats().aggregated.items {
            let sum = item.format_value_sum();
            let delta = item.format_delta_per_sec();
            sum_width = std::cmp::max(sum_width, sum.len());
            delta_width = std::cmp::max(delta_width, delta.len());
            row_items.push((k.clone(), sum, delta));
        }

        let rows = row_items.into_iter().map(|(k, sum, delta)| {
            Row::new(vec![
                Cell::from(k),
                Cell::from(format!("{:>sum_width$}", sum)),
                Cell::from(format!("{:>delta_width$}", delta)),
            ])
        });

        let widths = [
            Constraint::Percentage(60),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ];

        let highlight_style = if self.focus == Focus::AggregatedStats {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let highlight_symbol = format!(
            "{:>width$}> ",
            self.aggregated_table_state.selected().unwrap_or(0) + 1,
            width = (self.latest_stats().item_count()).to_string().len()
        );

        let table = Table::new(rows, widths)
            .header(header)
            .block(self.make_block("Aggregated Stats", Some(Focus::AggregatedStats)))
            .highlight_style(highlight_style)
            .highlight_symbol(highlight_symbol);
        f.render_stateful_widget(table, area, &mut self.aggregated_table_state);
    }

    fn render_details(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area);

        self.render_individual_stats(f, chunks[0]);
        self.render_chart(f, chunks[1]);
    }

    fn render_individual_stats(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let selected_key = self.selected_item_key();

        let mut row_items = Vec::with_capacity(self.latest_stats().connection_count());
        let mut value_width = 0;
        let mut delta_width = 0;
        let mut is_value_num = true;
        for connection in self.latest_stats().connections.values() {
            if let Some(item) = selected_key.and_then(|k| connection.items.get(k)) {
                let value = item.format_value();
                let delta = item.format_delta_per_sec();
                is_value_num &= item.value.as_f64().is_some();
                value_width = std::cmp::max(value_width, value.len());
                delta_width = std::cmp::max(delta_width, delta.len());
                row_items.push((connection.connection_id.clone(), value, delta));
            }
        }

        let rows = row_items.into_iter().map(|(connection_id, value, delta)| {
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

        let header_cells = if is_value_num {
            &["Connection ID", "Value", "Delta/s"][..]
        } else {
            &["Connection ID", "Value"][..]
        }
        .iter()
        .map(|&h| Cell::from(h).style(Style::default().add_modifier(Modifier::BOLD)));
        let header = Row::new(header_cells).bottom_margin(1);

        let widths = if is_value_num {
            vec![
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ]
        } else {
            vec![Constraint::Percentage(40), Constraint::Percentage(60)]
        };

        let highlight_style = if self.focus == Focus::IndividualStats {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        let cursor_width = (self.latest_stats().connection_count()).to_string().len();
        let highlight_symbol = if self.focus == Focus::IndividualStats {
            format!(
                "{:>width$}> ",
                self.individual_table_state.selected().unwrap_or(0) + 1,
                width = cursor_width
            )
        } else {
            format!("{:>width$}  ", "", width = cursor_width)
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(self.make_block(
                &format!("Values of {:?}", selected_key.unwrap_or("")),
                Some(Focus::IndividualStats),
            ))
            .highlight_style(highlight_style)
            .highlight_symbol(highlight_symbol);
        f.render_stateful_widget(table, area, &mut self.individual_table_state);
    }

    fn render_chart(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let block = match (self.selected_item_key(), self.selected_connection_id()) {
            (Some(key), Some(id)) => {
                self.make_block(&format!("Delta/s Chart of {:?} ({})", key, id), None)
            }
            (Some(key), _) => self.make_block(&format!("Delta/s Chart of {:?}", key), None),
            _ => self.make_block("Delta/s Chart", None),
        };

        let data = self.chart_data();
        if data.is_empty() {
            f.render_widget(block, area);
            return;
        }

        let datasets = vec![Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .data(&data)];

        let lower_bound = data
            .iter()
            .map(|(_, y)| *y)
            .min_by(|a, b| a.total_cmp(b))
            .expect("unreachable")
            .floor();
        let mut upper_bound = data
            .iter()
            .map(|(_, y)| *y)
            .max_by(|a, b| a.total_cmp(b))
            .expect("unreachable")
            .ceil();
        let is_constant = lower_bound == upper_bound;
        if is_constant {
            upper_bound = lower_bound + 1.0;
        }

        let x_max = self.options.chart_time_period.get();
        let y_labels = if is_constant {
            vec![Span::from(format_u64(lower_bound as u64)), Span::from("")]
        } else {
            vec![
                Span::from(format_u64(lower_bound as u64)),
                Span::from(format_u64(upper_bound as u64)),
            ]
        };

        let chart = Chart::new(datasets)
            .block(block)
            .x_axis(
                Axis::default()
                    .labels(vec![Span::from("0s"), Span::from(format!("{}s", x_max))])
                    .bounds([0.0, x_max as f64]),
            )
            .y_axis(
                Axis::default()
                    .labels(y_labels)
                    .bounds([lower_bound, upper_bound]),
            );
        f.render_widget(chart, area);
    }

    fn chart_data(&self) -> Vec<(f64, f64)> {
        match self.focus {
            Focus::AggregatedStats => self.aggregated_chart_data(),
            Focus::IndividualStats => self.individual_chart_data(),
        }
    }

    fn individual_chart_data(&self) -> Vec<(f64, f64)> {
        let (key, id) = if let (Some(key), Some(id)) =
            (self.selected_item_key(), self.selected_connection_id())
        {
            (key, id)
        } else {
            return Vec::new();
        };

        let (start, items) = self.history_window();
        items
            .filter_map(|stats| {
                let x = (stats.timestamp - start).as_secs_f64();
                stats
                    .connections
                    .get(id)
                    .and_then(|c| c.items.get(key))
                    .and_then(|y| y.delta_per_sec)
                    .map(|y| (x, y))
            })
            .collect()
    }

    fn aggregated_chart_data(&self) -> Vec<(f64, f64)> {
        let key = if let Some(key) = self.selected_item_key() {
            key
        } else {
            return Vec::new();
        };

        let (start, items) = self.history_window();
        items
            .filter_map(|stats| {
                let x = (stats.timestamp - start).as_secs_f64();
                stats
                    .aggregated
                    .items
                    .get(key)
                    .and_then(|y| y.delta_per_sec)
                    .map(|y| (x, y))
            })
            .collect()
    }

    fn selected_item_key(&self) -> Option<&str> {
        self.aggregated_table_state.selected().and_then(|i| {
            self.latest_stats()
                .aggregated
                .items
                .iter()
                .nth(i)
                .map(|(k, _)| k.as_str())
        })
    }

    fn selected_connection_id(&self) -> Option<&str> {
        if self.focus == Focus::AggregatedStats {
            return None;
        }

        self.individual_table_state.selected().and_then(|i| {
            self.latest_stats()
                .connections
                .iter()
                .nth(i)
                .map(|(k, _)| k.as_str())
        })
    }

    fn make_block(&self, name: &str, block: Option<Focus>) -> ratatui::widgets::Block<'static> {
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

    fn ensure_table_indices_are_in_ranges(&mut self) {
        if self.latest_stats().item_count() == 0 {
            self.aggregated_table_state.select(None);
        } else {
            let n = self.latest_stats().item_count();
            let i = std::cmp::min(self.aggregated_table_state.selected().unwrap_or(0), n - 1);
            self.aggregated_table_state.select(Some(i));
        }

        if self.latest_stats().connection_count() == 0 {
            self.individual_table_state.select(None);
        } else {
            let n = self.latest_stats().connection_count();
            let i = std::cmp::min(self.individual_table_state.selected().unwrap_or(0), n - 1);
            self.individual_table_state.select(Some(i));
        }
    }
}
