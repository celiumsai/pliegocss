use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::{Command, exit};
use std::thread;
use std::time::Duration;

fn main() {
    let arguments = env::args_os().skip(1).collect::<Vec<OsString>>();
    let should_block = arguments
        .windows(2)
        .any(|pair| pair[0] == "--style" && pair[1] == "cancel-me");
    if should_block {
        let marker = env::var_os("PLIEGOCSS_CANCELLATION_MARKER")
            .expect("PLIEGOCSS_CANCELLATION_MARKER is required");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker)
            .expect("cannot create cancellation marker");
        writeln!(file, "{}", std::process::id()).expect("cannot write cancellation marker");
        file.flush().expect("cannot flush cancellation marker");
        thread::sleep(Duration::from_secs(30));
        exit(86);
    }
    let should_emit_invalid_json = arguments
        .windows(2)
        .any(|pair| pair[0] == "--style" && pair[1] == "proxy-invalid-json");
    if should_emit_invalid_json {
        eprintln!("not diagnostic JSON");
        exit(87);
    }

    let compiler = env::var_os("PLIEGOCSS_REAL_COMPILER")
        .expect("PLIEGOCSS_REAL_COMPILER is required");
    let status = Command::new(compiler)
        .args(arguments)
        .status()
        .expect("cannot forward to pliego-cssc");
    exit(status.code().unwrap_or(1));
}
