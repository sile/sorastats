use crate::poller::StatsReceiver;
use crate::stats::ConnectionStats;
use clap::Parser;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Parser)]
pub struct UiOpts {
    #[clap(long, default_value_t = 600.0)]
    pub retention_period: f64,

    #[clap(long, default_value = "total")]
    pub tab: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tab {
    Total,
    Channel(String),
    Client(String),
    Bundle(String),
    Connection(String),
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Total => write!(f, "total"),
            Self::Channel(v) => write!(f, "channel:{v}"),
            Self::Client(v) => write!(f, "client:{v}"),
            Self::Bundle(v) => write!(f, "bundle:{v}"),
            Self::Connection(v) => write!(f, "connection:{v}"),
        }
    }
}

impl std::str::FromStr for Tab {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "total" {
            Ok(Self::Total)
        } else if s.starts_with("channel:") {
            Ok(Self::Channel(s["channel:".len()..].to_owned()))
        } else if s.starts_with("client:") {
            Ok(Self::Client(s["client:".len()..].to_owned()))
        } else if s.starts_with("bundle:") {
            Ok(Self::Bundle(s["bundle:".len()..].to_owned()))
        } else if s.starts_with("connection:") {
            Ok(Self::Connection(s["connection:".len()..].to_owned()))
        } else {
            anyhow::bail!("invalid tab name {s:?}");
        }
    }
}

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;

type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

// TODO: rename
#[derive(Debug)]
pub struct Ui {
    opt: UiOpts,
    history: VecDeque<HistoryItem>,
}

impl Ui {
    fn draw(&mut self, f: &mut Frame) {}
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
            ui: Ui {
                opt,
                history: VecDeque::new(),
            },
            terminal,
        })
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        loop {
            if self.handle_key_event()? {
                break;
            }
            if self.handle_stats_poll()? {
                self.terminal.draw(|f| self.ui.draw(f))?;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self) -> anyhow::Result<bool> {
        if crossterm::event::poll(std::time::Duration::from_secs(0))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('q') => {
                        return Ok(true);
                    }
                    // KeyCode::Up => {
                    //     if let Some(i) = app.top_state.selected() {
                    //         app.top_state.select(Some(i.saturating_sub(1)));
                    //     } else {
                    //         app.top_state.select(Some(0));
                    //     }
                    // }
                    // KeyCode::Down => {
                    //     if let Some(i) = app.top_state.selected() {
                    //         app.top_state.select(Some(i + 1)); // TODO:
                    //     } else {
                    //         app.top_state.select(Some(0));
                    //     }
                    // }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    fn handle_stats_poll(&mut self) -> anyhow::Result<bool> {
        match self.rx.recv_timeout(Duration::from_millis(10)) {
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("Sora stats polling thread terminated unexpectedly");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Ok(connections) => {
                log::debug!("recv new stats");
                self.ui.history.push_back(HistoryItem {
                    timestamp: Instant::now(),
                    connections,
                });
                while let Some(item) = self.ui.history.pop_front() {
                    if item.timestamp.elapsed().as_secs_f64() < self.ui.opt.retention_period {
                        self.ui.history.push_front(item);
                        break;
                    }
                    log::debug!("remove old stats");
                }
                return Ok(true);
            }
        }
        Ok(false)
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
    connections: Vec<ConnectionStats>,
}
