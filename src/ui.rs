use crate::poller::StatsReceiver;
use crate::stats::{ConnectionStats, StatsValue};
use chrono::{DateTime, Local};
use clap::Parser;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Parser)]
pub struct UiOpts {
    // TODO: rename
    #[clap(long, default_value_t = 600.0)]
    pub retention_period: f64,

    #[clap(long, short, default_value = ".*")]
    pub connection_filter: regex::Regex,

    #[clap(long, short = 'k', default_value = ".*")]
    pub stats_key_filter: regex::Regex,
}

impl UiOpts {
    // TODO: rename (apply_filter)
    fn filter_connections(&self, connections: Vec<ConnectionStats>) -> Vec<ConnectionStats> {
        connections
            .into_iter()
            .filter(|c| {
                c.stats
                    .iter()
                    .any(|(k, v)| self.connection_filter.is_match(&format!("{}:{}", k, v)))
            })
            .map(|mut c| {
                let stats = c
                    .stats
                    .into_iter()
                    .filter(|(k, _v)| self.stats_key_filter.is_match(k))
                    .collect();
                c.stats = stats;
                c
            })
            .collect()
    }
}

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;

type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

// TODO: rename
#[derive(Debug)]
pub struct Ui {
    opt: UiOpts,
    history: VecDeque<HistoryItem>,
    table_state: tui::widgets::TableState,
}

impl Ui {
    fn new(opt: UiOpts) -> Self {
        Self {
            opt,
            history: VecDeque::new(),
            table_state: Default::default(),
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        use tui::layout::{Constraint, Direction, Layout};

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(0)].as_ref())
            .split(f.size());

        self.render_header(f, chunks[0]);
        self.draw_stats(f, chunks[1]);
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

    fn render_status(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::Alignment;
        use tui::text::Spans;
        use tui::widgets::{Block, Borders, Paragraph};

        let item = self.history.back().expect("unreachable");
        let block = Block::default().borders(Borders::ALL).title("Status");
        let paragraph = Paragraph::new(vec![
            Spans::from(format!(
                "Update Time: {}",
                item.time
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
            )),
            Spans::from(format!("Connections: {}", item.connections.len())),
            Spans::from(format!(
                "Stats Keys:  {}",
                item.connections.get(0).map_or(0, |c| c.stats.len())
            )),
        ])
        .block(block)
        .alignment(Alignment::Left);
        f.render_widget(paragraph, area);
    }

    fn draw_stats(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::{Constraint, Direction, Layout};

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area);

        self.draw_aggregated_stats(f, chunks[0]);
        self.draw_detailed_stats(f, chunks[1]);
    }

    fn selected_item_chart(&self, selected: &StatsItem) -> Vec<(f64, f64)> {
        let mut items = Vec::new();
        let start = self.history[0].timestamp; // TODO
        for history_item in &self.history {
            let x = (history_item.timestamp - start).as_secs_f64();
            let mut y = 0.0;
            for conn in &self.history.back().expect("unreachable").connections {
                for (k, v) in &conn.stats {
                    if let StatsValue::Number(v) = v {
                        if *k == selected.key {
                            y += v.0;
                            break;
                        }
                    }
                }
            }
            items.push((x, y));
        }
        items // TODO: use delta instead of sum
    }

    fn selected_item_values(&self, selected: &StatsItem) -> Vec<(String, String)> {
        let mut items = Vec::new();
        for conn in &self.history.back().expect("unreachable").connections {
            for (k, v) in &conn.stats {
                if *k == selected.key {
                    let connection_id = conn.stats["connection_id"].to_string(); // TODO
                    items.push((connection_id, v.to_string()));
                    break;
                }
            }
        }
        // TODO: sort
        items
    }

    fn latest_stats(&self) -> Vec<StatsItem> {
        let mut items = BTreeMap::<_, StatsItem>::new();
        for conn in &self.history.back().expect("unreachable").connections {
            for (k, v) in &conn.stats {
                let entry = items.entry(k).or_default();
                entry.key = k.clone();
                entry.values.insert(v.clone());
            }
        }
        items.into_iter().map(|(_, v)| v).collect()
    }

    fn draw_aggregated_stats(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::Constraint;
        use tui::style::{Color, Modifier, Style};
        use tui::widgets::{Block, Borders, Cell, Row, Table};

        let selected_style = Style::default().add_modifier(Modifier::REVERSED);
        let normal_style = Style::default().bg(Color::Blue);

        let header_cells = ["Key", "Sum", "Uniq"]
            .into_iter()
            .map(|h| Cell::from(h).style(Style::default().fg(Color::Red)));
        let header = Row::new(header_cells)
            .style(normal_style)
            .height(1)
            .bottom_margin(1);

        let items = self.latest_stats();
        let rows = items.into_iter().map(|item| {
            let cells = match item.aggregated_value() {
                Ok(sum) => {
                    vec![
                        Cell::from(item.key),
                        Cell::from(sum.to_string()),
                        Cell::from(""),
                    ]
                }
                Err(uniq) => {
                    vec![
                        Cell::from(item.key),
                        Cell::from(""),
                        Cell::from(uniq.to_string()),
                    ]
                }
            };
            Row::new(cells)
        });

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Aggregated Stats");

        // TODO: align
        let t = Table::new(rows)
            .header(header)
            .block(block)
            .highlight_style(selected_style)
            .highlight_symbol(">> ")
            .widths(&[
                Constraint::Percentage(70),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
            ]);
        f.render_stateful_widget(t, area, &mut self.table_state);
    }

    fn draw_detailed_stats(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::{Constraint, Direction, Layout};
        use tui::widgets::{Block, Borders};

        if let Some(i) = self.table_state.selected() {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(area);

            let stats = self.latest_stats();
            let selected = &stats[i]; // TODO: range check
            self.draw_selected_stats(f, chunks[0], selected);
            if selected.is_number() {
                self.draw_chart(f, chunks[1], selected);
            } else {
            }
        } else {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Detailed Stats");
            f.render_widget(block, area);
        }
    }

    fn draw_chart(&mut self, f: &mut Frame, area: tui::layout::Rect, selected: &StatsItem) {
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

    fn draw_selected_stats(
        &mut self,
        f: &mut Frame,
        area: tui::layout::Rect,
        selected: &StatsItem,
    ) {
        use tui::layout::Constraint;
        use tui::style::{Color, Modifier, Style};
        use tui::widgets::{Block, Borders, Cell, Row, Table};

        let items = self.selected_item_values(selected);

        let selected_style = Style::default().add_modifier(Modifier::REVERSED);
        let normal_style = Style::default().bg(Color::Blue);

        let header_cells = ["Connection ID", "Value"]
            .into_iter()
            .map(|h| Cell::from(h).style(Style::default().fg(Color::Red)));
        let header = Row::new(header_cells)
            .style(normal_style)
            .height(1)
            .bottom_margin(1);

        let rows = items.into_iter().map(|(connection_id, value)| {
            Row::new(vec![Cell::from(connection_id), Cell::from(value)])
        });

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Values of {:?}", selected.key));

        // TODO: align
        let t = Table::new(rows)
            .header(header)
            .block(block)
            .highlight_style(selected_style)
            .highlight_symbol(">> ")
            .widths(&[
                Constraint::Percentage(50), // TODO: length
                Constraint::Percentage(50),
            ]);
        let mut state = Default::default(); // TODO
        f.render_stateful_widget(t, area, &mut state);
    }

    // TODO: rename
    fn draw_help(&mut self, f: &mut Frame, area: tui::layout::Rect) {
        use tui::layout::Alignment;
        use tui::text::Spans;
        use tui::widgets::{Block, Borders, Paragraph};

        let block = Block::default().borders(Borders::ALL).title("Help");
        let paragraph = Paragraph::new(vec![
            Spans::from("Quit: 'q' key"),
            Spans::from("Move: UP / DOWN / LEFT / RIGHT keys"),
        ])
        .block(block)
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
        Ok(Self {
            rx,
            ui: Ui::new(opt),
            terminal,
        })
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
                    // KeyCode::Right => {
                    //     let tab_index =
                    //         std::cmp::min(self.ui.tab_index + 1, self.ui.opt.tab.len() - 1);
                    //     if tab_index != self.ui.tab_index {
                    //         self.ui.tab_index = tab_index;
                    //         self.terminal.draw(|f| self.ui.draw(f))?;
                    //     }
                    // }
                    // KeyCode::Left => {
                    //     let tab_index = self.ui.tab_index.saturating_sub(1);
                    //     if tab_index != self.ui.tab_index {
                    //         self.ui.tab_index = tab_index;
                    //         self.terminal.draw(|f| self.ui.draw(f))?;
                    //     }
                    // }
                    KeyCode::Up => {
                        let i = if let Some(i) = self.ui.table_state.selected() {
                            i.saturating_sub(1)
                        } else {
                            0
                        };
                        self.ui.table_state.select(Some(i));
                        self.terminal.draw(|f| self.ui.draw(f))?;
                    }
                    KeyCode::Down => {
                        let i = if let Some(i) = self.ui.table_state.selected() {
                            // TODO: min
                            i + 1
                        } else {
                            0
                        };
                        self.ui.table_state.select(Some(i));
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
            Ok(connections) => {
                log::debug!("recv new stats");
                self.ui.history.push_back(HistoryItem {
                    timestamp: Instant::now(),
                    time: Local::now(),
                    connections: self.ui.opt.filter_connections(connections),
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
    timestamp: Instant,
    time: DateTime<Local>,
    connections: Vec<ConnectionStats>,
}

#[derive(Debug, Default)]
pub struct StatsItem {
    key: String,
    values: HashSet<StatsValue>,
}

impl StatsItem {
    pub fn aggregated_value(&self) -> Result<f64, usize> {
        let mut sum = 0.0;
        for v in &self.values {
            if let StatsValue::Number(v) = v {
                sum += v.0;
            } else {
                return Err(self.values.len());
            }
        }
        Ok(sum)
    }

    pub fn is_number(&self) -> bool {
        self.values
            .iter()
            .all(|x| matches!(x, StatsValue::Number(_)))
    }
}
