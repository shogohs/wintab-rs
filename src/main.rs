#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod input;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod ui;

fn parse_seconds(raw: &str) -> Result<f64, String> {
    let seconds: f64 = raw
        .parse()
        .map_err(|_| "duration must be a number between 0 and 10 seconds")?;
    if seconds.is_finite() && (0.0..=10.0).contains(&seconds) && seconds > 0.0 {
        Ok(seconds)
    } else {
        Err("duration must be greater than 0 and at most 10 seconds".into())
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        return Err("wintab-rs currently supports macOS only".into());
    }
    #[cfg(target_os = "macos")]
    {
        enum Command {
            Resident,
            Status,
            Pid(i32),
            Tap(f64),
            Capture(f64),
            ListWindows(i32),
            RaiseWindow(i32, usize),
            SwitchPid(i32, f64),
            ListCandidates,
            Switch(f64),
        }
        let command = match args.as_slice() {
            [] => Command::Resident,
            [flag] if flag == "--status" => Command::Status,
            [flag, pid] if flag == "--pid" => {
                let pid: i32 = pid.parse().map_err(|_| "PID must be a positive integer")?;
                if pid <= 0 {
                    return Err("PID must be a positive integer".into());
                }
                Command::Pid(pid)
            }
            [flag, duration] if flag == "--tap-seconds" => Command::Tap(parse_seconds(duration)?),
            [flag, duration] if flag == "--capture-seconds" => Command::Capture(parse_seconds(duration)?),
            [flag, pid] if flag == "--list-windows" => {
                let pid: i32 = pid.parse().map_err(|_| "PID must be a positive integer")?;
                if pid <= 0 { return Err("PID must be a positive integer".into()); }
                Command::ListWindows(pid)
            }
            [flag, pid, index] if flag == "--raise-window" => {
                let pid: i32 = pid.parse().map_err(|_| "PID must be a positive integer")?;
                if pid <= 0 { return Err("PID must be a positive integer".into()); }
                let index: usize = index.parse().map_err(|_| "window index must be a non-negative integer")?;
                Command::RaiseWindow(pid, index)
            }
            [flag, pid, seconds_flag, duration] if flag == "--switch-pid" && seconds_flag == "--seconds" => {
                let pid: i32 = pid.parse().map_err(|_| "PID must be a positive integer")?;
                if pid <= 0 { return Err("PID must be a positive integer".into()); }
                Command::SwitchPid(pid, parse_seconds(duration)?)
            }
            [flag] if flag == "--list-candidates" => Command::ListCandidates,
            [flag, seconds_flag, duration] if flag == "--switch" && seconds_flag == "--seconds" => {
                Command::Switch(parse_seconds(duration)?)
            }
            _ => return Err("Usage: wintab-rs [--pid PID | --list-windows PID | --raise-window PID INDEX | --tap-seconds SECONDS | --capture-seconds SECONDS | --switch-pid PID --seconds N | --list-candidates | --switch --seconds N]".into()),
        };
        if matches!(&command, Command::Resident) {
            return macos::resident();
        }
        let (ax, input_monitoring) = macos::startup_permissions()?;
        match command {
            Command::Resident => unreachable!(),
            Command::Status => {
                println!("wintab-rs phase 1 diagnostic PoC");
                println!(
                    "Use --list-windows PID / --raise-window PID INDEX for AX diagnostics, --list-candidates / --switch --seconds N for discovered windows, or --tap-seconds N / --capture-seconds N for input diagnostics."
                );
                Ok(())
            }
            Command::Pid(pid) => {
                if !ax {
                    return Ok(());
                }
                let count = macos::windows_count(pid)?;
                println!("AX windows for PID {pid}: {count}");
                Ok(())
            }
            Command::Tap(seconds) => {
                if !input_monitoring {
                    return Ok(());
                }
                println!(
                    "Listening for key-down events for {seconds:.1}s; all events pass through unchanged."
                );
                macos::listen(seconds)
            }
            Command::Capture(seconds) => {
                if !ax || !input_monitoring {
                    return Ok(());
                }
                eprintln!("Capture suppresses Command+Tab for at most {seconds:.1}s and never switches a window. Other keys cancel; capture stops at the deadline.");
                macos::capture(seconds)
            }
            Command::ListWindows(pid) => {
                if !ax {
                    return Ok(());
                }
                macos::list_windows(pid)
            }
            Command::RaiseWindow(pid, index) => {
                if !ax {
                    return Ok(());
                }
                macos::raise_window(pid, index)
            }
            Command::SwitchPid(pid, seconds) => {
                if !ax || !input_monitoring {
                    return Ok(());
                }
                eprintln!("One-session window switch for at most {seconds:.1}s. Other keys cancel; timeout restores normal input. The AX window list is frozen for this session.");
                macos::switch_pid(pid, seconds)
            }
            Command::ListCandidates => {
                if !ax {
                    return Ok(());
                }
                macos::list_candidates()
            }
            Command::Switch(seconds) => {
                if !ax || !input_monitoring {
                    return Ok(());
                }
                eprintln!("Candidate discovery uses Core Graphics owner metadata and Accessibility standard windows; it may omit apps or Spaces and does not provide MRU ordering.");
                macos::switch_global(seconds)
            }
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("wintab-rs: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_seconds;

    #[test]
    fn duration_is_bounded_to_ten_seconds() {
        assert_eq!(parse_seconds("0.5").unwrap(), 0.5);
        assert!(parse_seconds("0").is_err());
        assert!(parse_seconds("10.1").is_err());
        assert!(parse_seconds("NaN").is_err());
    }
}
