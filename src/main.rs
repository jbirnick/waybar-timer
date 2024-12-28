use clap::Parser;
use serde_dispatch::serde_dispatch;
use std::error::Error;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime};

const SOCKET_PATH: &str = "/tmp/waybar_timer.sock";
//const SOCKET_PATH: &str = "mysocket";
const INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

fn send_notification(summary: String, critical: bool) {
    let _ = notify_rust::Notification::new()
        .appname("Waybar Timer")
        .id(12345)
        .summary(&summary)
        .urgency(match critical {
            true => notify_rust::Urgency::Critical,
            false => notify_rust::Urgency::Low,
        })
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
    fn start(&mut self, minutes: u32, name: Option<String>) -> Result<(), WorldError>;
    fn increase(&mut self, seconds: i64) -> Result<(), WorldError>;
    fn togglepause(&mut self) -> Result<(), WorldError>;
}

#[derive(Debug)]
enum Timer {
    Idle,
    Running {
        expiry: OffsetDateTime,
        name: Option<String>,
    },
    Paused {
        time_left: Duration,
        name: Option<String>,
    },
}

impl Timer {
    /// update routine which is called regularly and on every change of the timer
    fn update(&mut self) -> std::io::Result<()> {
        let now = OffsetDateTime::now_local().unwrap();

        // check if timer expired
        if let Self::Running { expiry, name } = self {
            let time_left = *expiry - now;
            if time_left <= Duration::ZERO {
                // timer has expired, send notification and set timer to idle
                let summary = match name {
                    Some(name) => format!("Timer '{name}' expired"),
                    None => "Timer expired".into(),
                };
                send_notification(summary, true);
                *self = Timer::Idle;
            }
        }

        // print new output to stdout (for waybar)
        let (text, alt, tooltip) = match self {
            Self::Idle => (0, "standby", "No timer set".into()),
            Self::Running { expiry, name } => {
                let time_left = *expiry - now;
                let minutes_left = time_left.whole_minutes() + 1;
                let tooltip = Self::tooltip(name, expiry);
                (minutes_left, "running", tooltip)
            }
            Self::Paused { time_left, name } => {
                let minutes_left = time_left.whole_minutes() + 1;
                let tooltip = match name {
                    Some(name) => format!("Timer '{name}' paused"),
                    None => "Timer paused".into(),
                };
                (minutes_left, "paused", tooltip)
            }
        };
        println!("{{\"text\": \"{text}\", \"alt\": \"{alt}\", \"tooltip\": \"{tooltip}\", \"class\": \"timer\"}}");
        std::io::stdout().flush()
    }

    fn tooltip(name: &Option<String>, expiry: &OffsetDateTime) -> String {
        let format_desc = time::macros::format_description!("[hour]:[minute]");
        let expiry_str = expiry.format(&format_desc).unwrap();

        match name {
            Some(name) => format!("Timer '{name}' expires at {expiry_str}"),
            None => format!("Timer expires at {expiry_str}"),
        }
    }
}

impl World for Timer {
    fn cancel(&mut self) -> Result<(), WorldError> {
        *self = Self::Idle;
        Ok(())
    }

    fn start(&mut self, minutes: u32, name: Option<String>) -> Result<(), WorldError> {
        match self {
            Self::Idle => {
                let expiry = OffsetDateTime::now_local().unwrap()
                    + Duration::minutes(minutes.into())
                    - Duration::MILLISECOND;
                send_notification(Self::tooltip(&name, &expiry), false);
                *self = Self::Running { expiry, name };
                Ok(())
            }
            Self::Paused { .. } | Self::Running { .. } => Err(WorldError::TimerAlreadyExisting),
        }
    }

    fn increase(&mut self, seconds: i64) -> Result<(), WorldError> {
        match self {
            Self::Running { expiry, name } => {
                *expiry += Duration::seconds(seconds);
                send_notification(Self::tooltip(name, expiry), false);
                Ok(())
            }
            Self::Paused { time_left, name: _ } => {
                *time_left += Duration::seconds(seconds);
                Ok(())
            }
            Self::Idle => Err(WorldError::NoTimerExisting),
        }
    }

    fn togglepause(&mut self) -> Result<(), WorldError> {
        match self {
            Self::Running { expiry, name } => {
                let time_left = *expiry - OffsetDateTime::now_local().unwrap();
                send_notification(Self::tooltip(name, expiry), false);
                *self = Self::Paused {
                    time_left,
                    name: name.take(),
                };
                Ok(())
            }
            Self::Paused { time_left, name } => {
                let expiry = OffsetDateTime::now_local().unwrap() + *time_left;
                send_notification(Self::tooltip(name, &expiry), false);
                *self = Self::Running {
                    expiry,
                    name: name.take(),
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
    /// Start a server process (should be from within waybar)
    Tail,
    /// Start a new timer
    New { minutes: u32, name: Option<String> },
    /// Increase the current timer
    Increase { seconds: u32 },
    /// Decrease the current timer
    Decrease { seconds: u32 },
    /// Pause or resume the current timer
    Togglepause,
    /// Cancel the current timer
    Cancel,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    match args {
        Args::Tail => {
            run_tail();
            Ok(())
        }
        Args::New { minutes, name } => {
            let stream = UnixStream::connect(SOCKET_PATH)?;
            WorldRPCClient::call_with(&stream, &stream).start(&minutes, &name)??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Increase { seconds } => {
            let stream = UnixStream::connect(SOCKET_PATH)?;
            WorldRPCClient::call_with(&stream, &stream).increase(&seconds.into())??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Decrease { seconds } => {
            let seconds: i64 = seconds.into();
            let stream = UnixStream::connect(SOCKET_PATH)?;
            WorldRPCClient::call_with(&stream, &stream).increase(&-seconds)??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Togglepause => {
            let stream = UnixStream::connect(SOCKET_PATH)?;
            WorldRPCClient::call_with(&stream, &stream).togglepause()??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
        Args::Cancel => {
            let stream = UnixStream::connect(SOCKET_PATH)?;
            WorldRPCClient::call_with(&stream, &stream).cancel()??;
            stream.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        }
    }
}

fn run_tail() {
    let timer = Arc::new(Mutex::new(Timer::Idle));
    {
        let mut timer = timer.lock().unwrap();
        timer.update().unwrap();
    }

    let timer_thread = timer.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(INTERVAL);
        let mut timer = timer_thread.lock().unwrap();
        timer.update().unwrap();
    });

    // handle requests from the CLI
    // NOTE: binding is not possible if the file already exists, that's why we delete it first
    // this leads to undefined behavior when there is already a tail process running
    // maybe would be better to instead remove the file when program exits
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // handles a single remote procedure call
                let mut timer = timer.lock().unwrap();
                timer.handle_with(&stream, &stream).unwrap();
                stream.shutdown(std::net::Shutdown::Both).unwrap();
                timer.update().unwrap();
            }
            Err(err) => {
                panic!("{err}")
            }
        }
    }
}
