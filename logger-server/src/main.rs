#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value)]

use std::{
    collections::HashMap,
    io::Read,
    mem::size_of,
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, RwLock},
    time::Duration,
};

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use nix::{
    sys::{
        epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags},
        pthread::Pthread,
    },
    unistd::Pid,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Row, Table},
    Frame, Terminal,
};

const DEFAULT_CAPACITY: usize = 1024;

/// Simple program to greet a person
#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "/tmp/mp-logger-socket")]
    socket: String,
}

struct Process {
    id: Pid,
    // Maps threads ids to their index in `self.threads`.
    thread_id_map: HashMap<Pthread, usize>,
    threads: Vec<Thread>,
}

struct Thread {
    id: Pthread,
    log: Vec<Log>,
}

struct Log {
    time: Duration,
    level: log::Level,
    message: String,
}

struct App {
    process_id_map: HashMap<Pid, usize>,
    processes: Vec<Process>,
    process: ListState,
    thread: ListState,
    log: usize,
}
impl App {
    fn new() -> Self {
        Self {
            process_id_map: HashMap::new(),
            processes: Vec::new(),
            process: ListState::default(),
            thread: ListState::default(),
            log: 0,
        }
    }

    pub fn next_process(&mut self) {
        if let Some(process) = self.process.selected() {
            let new_process = (process + 1) % self.processes.len();
            self.process.select(Some(new_process));

            if !self.processes[new_process].threads.is_empty() {
                self.thread.select(Some(0));

                if !self.processes[new_process].threads[0].log.is_empty() {
                    self.log = 0;
                }
            }
        }
    }

    pub fn previous_process(&mut self) {
        if let Some(process) = self.process.selected() {
            let new_process = if process > 0 {
                process - 1
            } else {
                self.processes.len() - 1
            };
            self.process.select(Some(new_process));

            if !self.processes[new_process].threads.is_empty() {
                self.thread.select(Some(0));

                if !self.processes[new_process].threads[0].log.is_empty() {
                    self.log = 0;
                }
            }
        }
    }

    pub fn next_thread(&mut self) {
        if let Some(thread) = self.thread.selected() {
            let new_thread = (thread + 1)
                % self.processes[self.process.selected().unwrap()]
                    .threads
                    .len();
            self.thread.select(Some(new_thread));

            if !self.processes[self.process.selected().unwrap()].threads[new_thread]
                .log
                .is_empty()
            {
                self.log = 0;
            }
        }
    }

    pub fn previous_thread(&mut self) {
        if let Some(thread) = self.thread.selected() {
            let new_thread = if thread > 0 {
                thread - 1
            } else {
                self.processes[self.process.selected().unwrap()]
                    .threads
                    .len()
                    - 1
            };
            self.thread.select(Some(new_thread));

            if !self.processes[self.process.selected().unwrap()].threads[new_thread]
                .log
                .is_empty()
            {
                self.log = 0;
            }
        }
    }

    pub fn next_log(&mut self) {
        if let (Some(process), Some(thread)) = (self.process.selected(), self.thread.selected()) {
            if self.log < self.processes[process].threads[thread].log.len() {
                self.log += 1;
            }
        }
    }

    pub fn previous_log(&mut self) {
        if self.log > 0 {
            self.log -= 1;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // setup terminal
    enable_raw_mode()?;
    let mut log = std::io::stdout();
    execute!(log, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(log);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app, args.socket);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: App,
    socket: String,
) -> std::io::Result<()> {
    let app = Arc::new(RwLock::new(app));

    let app_clone = app.clone();
    let socket_clone = socket.clone();
    let _ = std::thread::spawn(move || {
        let listener = UnixListener::bind(&socket_clone).unwrap();
        for (stream, id) in listener.incoming().zip(0..) {
            // println!("Received connection: {id:08x}");

            let stream = stream.unwrap();
            let app_clone_clone = app_clone.clone();
            std::thread::spawn(move || handle_stream(stream, id, app_clone_clone));
        }
    });

    loop {
        let app_clone = app.clone();
        terminal.draw(|f| ui(f, app_clone))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('w') => app.write().unwrap().previous_process(),
                KeyCode::Char('s') => app.write().unwrap().next_process(),
                KeyCode::Char('e') => app.write().unwrap().previous_thread(),
                KeyCode::Char('d') => app.write().unwrap().next_thread(),
                KeyCode::Char('r') => app.write().unwrap().previous_log(),
                KeyCode::Char('f') => app.write().unwrap().next_log(),
                _ => {}
            }
        }
    }

    std::fs::remove_file(&socket).unwrap();

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, app: Arc<RwLock<App>>) {
    let mut app = app.write().unwrap();

    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(9),
                Constraint::Length(14),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(size);

    let block = Block::default().style(Style::default());
    f.render_widget(block, size);

    // Process
    // ---------------------------------------------------------------------------------------------
    let process_numbers = app
        .processes
        .iter()
        .map(|t| ListItem::new(format!("{:x}", t.id.as_raw())))
        .collect::<Vec<_>>();

    let process_tabs = List::new(process_numbers)
        .block(Block::default().borders(Borders::ALL).title("Process"))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        );

    f.render_stateful_widget(process_tabs, chunks[0], &mut app.process);

    // Thread
    // ---------------------------------------------------------------------------------------------
    let thread_ids = if let Some(process) = app.process.selected() {
        app.processes[process]
            .threads
            .iter()
            .map(|thread| ListItem::new(format!("{:x}", thread.id)))
            .collect()
    } else {
        Vec::new()
    };

    let thread_tabs = List::new(thread_ids)
        .block(Block::default().borders(Borders::ALL).title("Thread"))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        );

    f.render_stateful_widget(thread_tabs, chunks[1], &mut app.thread);

    // log
    // ---------------------------------------------------------------------------------------------
    let log = match (app.process.selected(), app.thread.selected()) {
        (Some(process), Some(thread)) => {
            let x = app.processes[process].threads[thread]
                .log
                .iter()
                .skip(app.log)
                .map(
                    |Log {
                         time,
                         message,
                         level,
                     }| {
                        Row::new(vec![
                            time.as_micros().to_string(),
                            level.to_string(),
                            message.clone(),
                        ])
                    },
                )
                .collect::<Vec<_>>();
            Table::new(x)
        }
        _ => Table::new(Vec::new()),
    }
    .block(Block::default().title("Log").borders(Borders::ALL))
    .header(Row::new(vec!["Time (μs)", "Level", "Message"]))
    .widths(&[
        Constraint::Length(16),
        Constraint::Length(5),
        Constraint::Percentage(100),
    ]);

    f.render_widget(log, chunks[2]);
}

fn non_blocking(res: std::io::Result<usize>) -> std::io::Result<usize> {
    match res {
        Ok(n) => Ok(n),
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(err) => Err(err),
    }
}

#[repr(C)]
struct LogData {
    secs: u64,
    nanos: u32,
    pid: nix::unistd::Pid,
    pthread: nix::sys::pthread::Pthread,
    length: usize,
    level: log::Level,
}

fn handle_stream(mut stream: UnixStream, id: usize, app: Arc<RwLock<App>>) {
    stream.set_nonblocking(true).unwrap();

    let mut array = [0; size_of::<LogData>()];
    let mut data = Vec::with_capacity(DEFAULT_CAPACITY);

    let epoll = Epoll::new(EpollCreateFlags::empty()).unwrap();
    epoll
        .add(
            &stream,
            EpollEvent::new(EpollFlags::EPOLLIN | EpollFlags::EPOLLET, 0),
        )
        .unwrap();

    loop {
        // Fixed size data
        // -----------------------------------------------------------------------------------------
        let mut array_index = non_blocking(stream.read(&mut array)).unwrap();
        while array_index < array.len() {
            epoll.wait(&mut [EpollEvent::empty()], -1).unwrap();
            array_index += stream.read(&mut array[array_index..]).unwrap();
        }
        let log_data = unsafe { std::mem::transmute::<_, LogData>(array) };

        // Dynamic size data
        // -----------------------------------------------------------------------------------------
        data.resize(log_data.length, 0);

        let mut data_index = non_blocking(stream.read(&mut data)).unwrap();
        while data_index < data.len() {
            epoll.wait(&mut [EpollEvent::empty()], -1).unwrap();
            data_index += stream.read(&mut data[data_index..]).unwrap();
        }

        let message = String::from(std::str::from_utf8(&data).unwrap());

        // Add data
        // -----------------------------------------------------------------------------------------
        let mut app = app.write().unwrap();
        let time = Duration::new(log_data.secs, log_data.nanos);
        let log = Log {
            time,
            message,
            level: log_data.level,
        };

        if let Some(process) = app
            .process_id_map
            .get(&log_data.pid)
            .copied()
            .map(|i| &mut app.processes[i])
        {
            if let Some(thread) = process
                .thread_id_map
                .get(&log_data.pthread)
                .map(|i| &mut process.threads[*i])
            {
                thread.log.push(log);
            } else {
                process
                    .thread_id_map
                    .insert(log_data.pthread, process.threads.len());
                process.threads.push(Thread {
                    id: log_data.pthread,
                    log: vec![log],
                });
            }
        } else {
            let len = app.processes.len();
            app.process_id_map.insert(log_data.pid, len);
            app.processes.push(Process {
                id: log_data.pid,
                thread_id_map: std::iter::once((log_data.pthread, 0)).collect(),
                threads: vec![Thread {
                    id: log_data.pthread,
                    log: vec![log],
                }],
            });
        }
        if app.thread.selected().is_none() {
            app.process.select(Some(id));
            app.thread.select(Some(0));
        }
    }
}
