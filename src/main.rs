use clap::Parser;
use serde_dispatch::serde_dispatch;
use std::error::Error;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime};

const SOCKET_PATH_COMMANDS: &str = "/tmp/waybar_timer_commands.sock";
const SOCKET_PATH_UPDATES: &str = "/tmp/waybar_timer_updates.sock";
const INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

fn send_notification(summary: String) {
    let _ = notify_rust::Notification::new()
        .appname("Waybar Timer")
        .id(12345)
        .summary(&summary)
        .urgency(notify_rust::Urgency::Low)
        .show();
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
enum WorldError {
    NoTimerExisting,
    TimerAlreadyExisting,
}
impl std::fmt::Display for WorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldError::NoTimerExisting => write!(f, "no timer exists right now"),
            WorldError::TimerAlreadyExisting => write!(f, "there already exists a timer"),
        }
    }
}
impl Error for WorldError {}

#[serde_dispatch]
trait World {
    fn cancel(&mut self) -> Result<(), WorldError>;
    fn start(&mut self, minutes: u32, command: Option<String>) -> Result<(), WorldError>;
    fn increase(&mut self, seconds: i64) -> Result<(), WorldError>;
    fn togglepause(&mut self) -> Result<(), WorldError>;
}

#[derive(Debug)]
enum Timer {
    Idle,
    Running {
        expiry: OffsetDateTime,
        command: Option<String>,
    },
    Paused {
        time_left: Duration,
        command: Option<String>,
    },
}

impl Timer {
    /// updates timer, potentially executes action, and returns formatted string for waybar
    fn update(&mut self) -> String {
        let now = OffsetDateTime::now_local().unwrap();

        // check if timer expired
        if let Self::Running { expiry, command } = self {
            let time_left = *expiry - now;
            if time_left <= Duration::ZERO {
                // timer has expired, send notification and set timer to idle
                if let Some(command) = command {
                    let _ = std::process::Command::new("bash")
                        .arg("-c")
                        .arg(command)
                        .output();
                }
                *self = Timer::Idle;
            }
        }

        // print new output to stdout (for waybar)
        let (text, alt, tooltip) = match self {
            Self::Idle => (0, "standby", "No timer set".into()),
            Self::Running { expiry, .. } => {
                let time_left = *expiry - now;
                let minutes_left = time_left.whole_minutes() + 1;
                let tooltip = Self::tooltip(expiry);
                (minutes_left, "running", tooltip)
            }
            Self::Paused { time_left, .. } => {
                let minutes_left = time_left.whole_minutes() + 1;
                let tooltip = "Timer paused".into();
                (minutes_left, "paused", tooltip)
            }
        };
        format!("{{\"text\": \"{text}\", \"alt\": \"{alt}\", \"tooltip\": \"{tooltip}\", \"class\": \"timer\"}}")
    }

    fn tooltip(expiry: &OffsetDateTime) -> String {
        let format_desc = time::macros::format_description!("[hour]:[minute]");
        let expiry_str = expiry.format(&format_desc).unwrap();
        format!("Timer expires at {expiry_str}")
    }
}

impl World for Timer {
    fn cancel(&mut self) -> Result<(), WorldError> {
        match self {
            Self::Idle => {}
            _ => send_notification("Timer canceled".into()),
        };
        *self = Self::Idle;
        Ok(())
    }

    fn start(&mut self, minutes: u32, command: Option<String>) -> Result<(), WorldError> {
        match self {
            Self::Idle => {
                let expiry = OffsetDateTime::now_local().unwrap()
                    + Duration::minutes(minutes.into())
                    - Duration::MILLISECOND;
                send_notification(Self::tooltip(&expiry));
                *self = Self::Running { expiry, command };
                Ok(())
            }
            Self::Paused { .. } | Self::Running { .. } => Err(WorldError::TimerAlreadyExisting),
        }
    }

    fn increase(&mut self, seconds: i64) -> Result<(), WorldError> {
        match self {
            Self::Running { expiry, .. } => {
                *expiry += Duration::seconds(seconds);
                send_notification(Self::tooltip(expiry));
                Ok(())
            }
            Self::Paused {
                time_left,
                command: _,
            } => {
                *time_left += Duration::seconds(seconds);
                Ok(())
            }
            Self::Idle => Err(WorldError::NoTimerExisting),
        }
    }

    fn togglepause(&mut self) -> Result<(), WorldError> {
        match self {
            Self::Running { expiry, command } => {
                let time_left = *expiry - OffsetDateTime::now_local().unwrap();
                send_notification("Timer paused".into());
                *self = Self::Paused {
                    time_left,
                    command: command.take(),
                };
                Ok(())
            }
            Self::Paused { time_left, command } => {
                let expiry = OffsetDateTime::now_local().unwrap() + *time_left;
                send_notification(Self::tooltip(&expiry));
                *self = Self::Running {
                    expiry,
                    command: command.take(),
                };
                Ok(())
            }
            Self::Idle => Err(WorldError::NoTimerExisting),
        }
    }
}

/// Waybar Timer (see https://github.com/jbirnick/waybar-timer/)
#[derive(Parser)]
enum Args {
    /// Serve a timer API (should be called once at compositor startup)
    Serve,
    /// Keep reading the latest status of the timer (should be called by waybar)
    Hook,
    /// Start a new timer
    New {
        minutes: u32,
        command: Option<String>,
    },
    /// Increase the current timer
    Increase { seconds: u32 },
    /// Decrease the current timer
    Decrease { seconds: u32 },
    /// Pause or resume the current timer
    Togglepause,
    /// Cancel the current timer
    Cancel,
}

struct ServerState {
    timer: Timer,
    subs: Vec<UnixStream>,
}

impl ServerState {
    fn update(&mut self) {
        // update timer and get waybar string
        let message = self.timer.update();

        // broadcast it to subscribers
        let mut i: usize = 0;
        loop {
            if i == self.subs.len() {
                break;
            }
            match writeln!(self.subs[i], "{}", message) {
                Ok(()) => {
                    let _ = self.subs[i].flush();
                    i += 1;
                }
                Err(err) => {
                    println!("couldn't write to subscriber stream: {}", err);
                    println!("will drop the subscriber");
                    self.subs.swap_remove(i);
                }
            }
        }
    }
}

fn run_serve() {
    let state = Arc::new(Mutex::new(ServerState {
        timer: Timer::Idle,
        subs: Vec::new(),
    }));

    // spawn a thread which is responsible for calling update in a regular interval
    let state_thread_interval = state.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(INTERVAL);
        let mut state = state_thread_interval.lock().unwrap();
        state.update();
    });

    // spawn a thread which is responsible for accepting new subscribers
    let state_thread_subaccept = state.clone();
    std::thread::spawn(move || {
        // NOTE: binding is not possible if the file already exists, that's why we delete it first
        // this leads to undefined behavior when there is already a tail process running
        // maybe would be better to instead remove the file when program exits
        let _ = std::fs::remove_file(SOCKET_PATH_UPDATES);
        let listener = UnixListener::bind(SOCKET_PATH_UPDATES).unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    // put to list of subscribers and trigger update so that
                    // the new subscriber gets the current state
                    let mut state = state_thread_subaccept.lock().unwrap();
                    stream.shutdown(std::net::Shutdown::Read).unwrap();
                    state.subs.push(stream);
                    state.update();
                }
                Err(err) => {
                    panic!("{err}")
                }
            }
        }
    });

    // the main thread handles handle requests from the CLI
    // NOTE: binding is not possible if the file already exists, that's why we delete it first
    // this leads to undefined behavior when there is already a tail process running
    // maybe would be better to instead remove the file when program exits
    let _ = std::fs::remove_file(SOCKET_PATH_COMMANDS);
    let listener = UnixListener::bind(SOCKET_PATH_COMMANDS).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // handles a single remote procedure call
                let mut state = state.lock().unwrap();
                state.timer.handle_with(&stream, &stream).unwrap();
                stream.shutdown(std::net::Shutdown::Both).unwrap();
                state.update();
            }
            Err(err) => {
                panic!("{err}")
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    match args {
        Args::Serve => {
            run_serve();
            Ok(())
        }
        Args::Hook => {
            let mut stream = UnixStream::connect(SOCKET_PATH_UPDATES)?;
            stream.shutdown(std::net::Shutdown::Write)?;
            let mut stdout = std::io::stdout();
            std::io::copy(&mut stream, &mut stdout)?;
            Ok(())
        }
        Args::New { minutes, command } => {
            let stream = UnixStream::connect(SOCKET_PATH_COMMANDS)?;
            WorldRPCClient::call_with(&stream, &stream).start(&minutes, &command)??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Increase { seconds } => {
            let stream = UnixStream::connect(SOCKET_PATH_COMMANDS)?;
            WorldRPCClient::call_with(&stream, &stream).increase(&seconds.into())??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Decrease { seconds } => {
            let seconds: i64 = seconds.into();
            let stream = UnixStream::connect(SOCKET_PATH_COMMANDS)?;
            WorldRPCClient::call_with(&stream, &stream).increase(&-seconds)??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Togglepause => {
            let stream = UnixStream::connect(SOCKET_PATH_COMMANDS)?;
            WorldRPCClient::call_with(&stream, &stream).togglepause()??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Cancel => {
            let stream = UnixStream::connect(SOCKET_PATH_COMMANDS)?;
            WorldRPCClient::call_with(&stream, &stream).cancel()??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
    }
}
