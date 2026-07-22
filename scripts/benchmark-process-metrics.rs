use std::env;
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(windows)]
use std::mem::{size_of, zeroed};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
#[allow(non_snake_case)]
#[repr(C)]
struct ProcessMemoryCounters {
    cb: u32,
    PageFaultCount: u32,
    PeakWorkingSetSize: usize,
    WorkingSetSize: usize,
    QuotaPeakPagedPoolUsage: usize,
    QuotaPagedPoolUsage: usize,
    QuotaPeakNonPagedPoolUsage: usize,
    QuotaNonPagedPoolUsage: usize,
    PagefileUsage: usize,
    PeakPagefileUsage: usize,
}

#[cfg(windows)]
#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(
        process: *mut c_void,
        counters: *mut ProcessMemoryCounters,
        size: u32,
    ) -> i32;
}

#[cfg(windows)]
fn peak_working_set(child: &std::process::Child) -> Option<u64> {
    let mut counters: ProcessMemoryCounters = unsafe { zeroed() };
    counters.cb = size_of::<ProcessMemoryCounters>() as u32;
    let succeeded = unsafe {
        GetProcessMemoryInfo(
            child.as_raw_handle(),
            &mut counters,
            size_of::<ProcessMemoryCounters>() as u32,
        )
    };
    (succeeded != 0).then_some(counters.PeakWorkingSetSize as u64)
}

#[cfg(target_os = "linux")]
fn peak_working_set(child: &std::process::Child) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{}/status", child.id())).ok()?;
    let kilobytes = status.lines().find_map(|line| {
        line.strip_prefix("VmHWM:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    })?;
    Some(kilobytes * 1024)
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct RusageInfoV2 {
    uuid: [u8; 16],
    user_time: u64,
    system_time: u64,
    pkg_idle_wkups: u64,
    interrupt_wkups: u64,
    pageins: u64,
    wired_size: u64,
    resident_size: u64,
    phys_footprint: u64,
    proc_start_abstime: u64,
    proc_exit_abstime: u64,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut c_void) -> i32;
}

#[cfg(target_os = "macos")]
fn peak_working_set(child: &std::process::Child) -> Option<u64> {
    const RUSAGE_INFO_V2: i32 = 2;
    let mut usage = RusageInfoV2 {
        uuid: [0; 16],
        user_time: 0,
        system_time: 0,
        pkg_idle_wkups: 0,
        interrupt_wkups: 0,
        pageins: 0,
        wired_size: 0,
        resident_size: 0,
        phys_footprint: 0,
        proc_start_abstime: 0,
        proc_exit_abstime: 0,
    };
    let succeeded = unsafe {
        proc_pid_rusage(
            child.id() as i32,
            RUSAGE_INFO_V2,
            (&mut usage as *mut RusageInfoV2).cast::<c_void>(),
        )
    };
    (succeeded == 0).then_some(usage.resident_size)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn peak_working_set(_child: &std::process::Child) -> Option<u64> {
    None
}

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        eprintln!("usage: benchmark-process-metrics <command> [arguments ...]");
        return ExitCode::from(2);
    };

    let started = Instant::now();
    let mut child = match Command::new(command)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("cannot start measured process: {error}");
            return ExitCode::from(2);
        }
    };

    let mut peak = 0_u64;
    loop {
        if let Some(observed) = peak_working_set(&child) {
            peak = peak.max(observed);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let elapsed = started.elapsed().as_nanos();
                let code = status.code().unwrap_or(-1);
                println!(
                    "{{\"schemaVersion\":1,\"elapsedNs\":{elapsed},\"peakWorkingSetBytes\":{peak},\"exitCode\":{code}}}"
                );
                return if status.success() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(1)),
            Err(error) => {
                eprintln!("cannot observe measured process: {error}");
                let _ = child.kill();
                return ExitCode::from(2);
            }
        }
    }
}
