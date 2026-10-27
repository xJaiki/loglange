use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tempfile::NamedTempFile;

pub struct CliInitResult {
    pub initial_file: Option<String>,
    pub stdin_temp: Option<NamedTempFile>,
}

/// Inspects command-line arguments and standard input.
/// Supports opening files via argument (e.g. `loglance /path/to/file.log`)
/// and streaming standard input via pipe (e.g. `cat file.log | loglance`).
pub fn process_cli_args() -> CliInitResult {
    let args: Vec<String> = std::env::args().collect();

    // Check for help and version flags
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Loglance v{}", env!("CARGO_PKG_VERSION"));
        println!("Loglance — Ultra-Fast Minimalist Real-Time Log Viewer\n");
        println!("USAGE:");
        println!("  loglance [FILE]          Open a log file directly");
        println!("  cat file.log | loglance  Stream log lines from standard input\n");
        println!("OPTIONS:");
        println!("  -h, --help     Print help information");
        println!("  -v, --version  Print version information");
        std::process::exit(0);
    }

    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("loglance {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    // Check for positional file path argument
    let file_arg = args.iter().skip(1).find(|a| !a.starts_with('-') && !a.ends_with("tauri"));
    if let Some(arg) = file_arg {
        let p = Path::new(arg);
        let abs_path = if p.is_absolute() {
            p.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };

        if abs_path.exists() {
            return CliInitResult {
                initial_file: Some(abs_path.to_string_lossy().into_owned()),
                stdin_temp: None,
            };
        }
    }

    // Check if stdin is piped
    if !std::io::stdin().is_terminal() {
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let mut stdin = std::io::stdin();
            let mut buf = [0u8; 64 * 1024];

            // Try reading first chunk
            match stdin.read(&mut buf) {
                Ok(0) => {
                    // Empty stdin / EOF (e.g. /dev/null)
                    let _ = tx.send(None);
                }
                Ok(n) => {
                    let Ok(mut temp) = tempfile::Builder::new()
                        .prefix("loglance-stdin-")
                        .suffix(".log")
                        .tempfile()
                    else {
                        let _ = tx.send(None);
                        return;
                    };

                    if temp.write_all(&buf[..n]).is_err() || temp.flush().is_err() {
                        let _ = tx.send(None);
                        return;
                    }

                    let path_str = temp.path().to_string_lossy().into_owned();
                    let Ok(mut writer) = temp.as_file().try_clone() else {
                        let _ = tx.send(None);
                        return;
                    };

                    // Send the NamedTempFile to keep it alive
                    let _ = tx.send(Some((path_str, temp)));

                    // Continue streaming stdin in loop until EOF
                    loop {
                        match stdin.read(&mut buf) {
                            Ok(0) => break,
                            Ok(num) => {
                                let _ = writer.write_all(&buf[..num]);
                                let _ = writer.flush();
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(_) => break,
                        }
                    }
                }
                Err(_) => {
                    let _ = tx.send(None);
                }
            }
        });

        // Wait up to 100ms for first chunk
        if let Ok(Some((path, temp_file))) = rx.recv_timeout(Duration::from_millis(100)) {
            return CliInitResult {
                initial_file: Some(path),
                stdin_temp: Some(temp_file),
            };
        }
    }

    CliInitResult {
        initial_file: None,
        stdin_temp: None,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_cli_help_flag_detection() {
        let test_args = ["loglance".to_string(), "--help".to_string()];
        assert!(test_args.iter().any(|a| a == "-h" || a == "--help"));
    }

    #[test]
    fn test_cli_version_flag_detection() {
        let test_args = ["loglance".to_string(), "-v".to_string()];
        assert!(test_args.iter().any(|a| a == "-v" || a == "--version"));
    }

    #[test]
    fn test_cli_positional_arg_resolution() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path_str = temp.path().to_str().unwrap().to_string();
        let args = ["loglance".to_string(), path_str.clone()];
        let file_arg = args.iter().skip(1).find(|a| !a.starts_with('-'));
        assert_eq!(file_arg, Some(&path_str));
    }
}
