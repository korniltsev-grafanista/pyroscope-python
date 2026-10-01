use remoteprocess::Pid;

/// Options on how to collect samples from a python process
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub blocking: LockingStrategy,
    pub native: bool,
    pub include_thread_names: bool,
    #[doc(hidden)]
    pub command: String,
    #[doc(hidden)]
    pub pid: Option<Pid>,
    #[doc(hidden)]
    pub python_program: Option<Vec<String>>,
    #[doc(hidden)]
    pub sampling_rate: u64,
    #[doc(hidden)]
    pub filename: Option<String>,
    #[doc(hidden)]
    pub format: Option<FileFormat>,
    #[doc(hidden)]
    pub show_line_numbers: bool,
    #[doc(hidden)]
    pub duration: RecordDuration,
    #[doc(hidden)]
    pub include_idle: bool,
    #[doc(hidden)]
    pub include_thread_ids: bool,
    #[doc(hidden)]
    pub subprocesses: bool,
    #[doc(hidden)]
    pub gil_only: bool,
    #[doc(hidden)]
    pub hide_progress: bool,
    #[doc(hidden)]
    pub capture_output: bool,
    #[doc(hidden)]
    pub dump_json: bool,
    #[doc(hidden)]
    pub dump_locals: u64,
    #[doc(hidden)]
    pub full_filenames: bool,
    #[doc(hidden)]
    pub lineno: LineNo,
    #[doc(hidden)]
    pub refresh_seconds: f64,
    #[doc(hidden)]
    pub core_filename: Option<String>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FileFormat {
    flamegraph,
    raw,
    speedscope,
    chrometrace,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LockingStrategy {
    NonBlocking,
    #[allow(dead_code)]
    AlreadyLocked,
    Lock,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RecordDuration {
    Unlimited,
    Seconds(u64),
}

#[derive(Debug, Clone, Eq, PartialEq, Copy)]
pub enum LineNo {
    NoLine,
    First,
    LastInstruction,
}

impl Default for Config {
    #[allow(dead_code)]
    fn default() -> Config {
        Config {
            pid: None,
            python_program: None,
            filename: None,
            format: None,
            command: String::from("top"),
            blocking: LockingStrategy::Lock,
            show_line_numbers: false,
            sampling_rate: 100,
            duration: RecordDuration::Unlimited,
            native: false,
            include_thread_names: true,
            gil_only: false,
            include_idle: false,
            include_thread_ids: false,
            hide_progress: false,
            capture_output: true,
            dump_json: false,
            dump_locals: 0,
            subprocesses: false,
            full_filenames: false,
            lineno: LineNo::LastInstruction,
            refresh_seconds: 1.0,
            core_filename: None,
        }
    }
}
