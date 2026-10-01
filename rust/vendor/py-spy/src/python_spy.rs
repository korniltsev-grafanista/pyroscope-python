use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Error, Result};
use remoteprocess::{Pid, Process, ProcessMemory, Tid};

use crate::config::{Config, LockingStrategy};
use crate::python_bindings::{
    v2_7_15, v3_10_0, v3_11_0, v3_12_0, v3_13_0, v3_14_0, v3_3_7, v3_5_5, v3_6_6, v3_7_0, v3_8_0,
    v3_9_5,
};
use crate::python_data_access::format_variable;
use crate::python_interpreters::{InterpreterState, ThreadState};
use crate::python_process_info::{
    get_interpreter_address_with_debug_offsets, get_python_version, get_threadstate_address,
    PythonDebugOffsets, PythonProcessInfo,
};
use crate::python_threading::thread_name_lookup;
use crate::stack_trace::{
    get_gil_threadid, get_stack_trace, new_frame_cache, FrameCache, StackTrace,
};
use crate::version::Version;

/// Lets you retrieve stack traces of a running python program
pub struct PythonSpy {
    pub pid: Pid,
    pub process: Process,
    pub version: Version,
    pub interpreter_address: usize,
    pub threadstate_address: usize,
    pub config: Config,
    pub short_filenames: HashMap<String, Option<String>>,
    pub frame_cache: FrameCache,
    pub python_thread_ids: HashMap<u64, Tid>,
    pub python_thread_names: HashMap<u64, String>,
    pub(crate) debug_offsets: Option<PythonDebugOffsets>,
    #[cfg(target_os = "linux")]
    pub dockerized: bool,
}

impl PythonSpy {
    /// Constructs a new PythonSpy object.
    pub fn new(pid: Pid, config: &Config) -> Result<PythonSpy, Error> {
        let process = remoteprocess::Process::new(pid)
            .context("Failed to open process - check if it is running.")?;

        // get basic process information (memory maps/symbols etc)
        let python_info = PythonProcessInfo::new(&process)?;

        // lock the process when loading up on freebsd (rather than locking
        // on every memory read). Needs done after getting python process info
        // because procmaps also tries to attach w/ ptrace on freebsd
        #[cfg(target_os = "freebsd")]
        let _lock = process.lock();

        let version = get_python_version(&python_info, &process)?;
        info!("python version {} detected", version);

        let (interpreter_address, debug_offsets) =
            get_interpreter_address_with_debug_offsets(&python_info, &process, &version)?;
        info!("Found interpreter at 0x{:016x}", interpreter_address);

        // lets us figure out which thread has the GIL
        let threadstate_address = get_threadstate_address(
            interpreter_address,
            &python_info,
            &process,
            &version,
            config,
        )?;

        Ok(PythonSpy {
            pid,
            process,
            version,
            interpreter_address,
            threadstate_address,
            #[cfg(target_os = "linux")]
            dockerized: python_info.dockerized,
            config: config.clone(),
            short_filenames: HashMap::new(),
            python_thread_ids: HashMap::new(),
            python_thread_names: HashMap::new(),
            debug_offsets,
            frame_cache: new_frame_cache(),
        })
    }

    /// Creates a PythonSpy object, retrying up to max_retries times.
    /// Mainly useful for the case where the process is just started and
    /// symbols or the python interpreter might not be loaded yet.
    pub fn retry_new(pid: Pid, config: &Config, max_retries: u64) -> Result<PythonSpy, Error> {
        let mut retries = 0;
        loop {
            let err = match PythonSpy::new(pid, config) {
                Ok(mut process) => {
                    // verify that we can load a stack trace before returning success
                    match process.get_stack_traces() {
                        Ok(_) => return Ok(process),
                        Err(err) => err,
                    }
                }
                Err(err) => err,
            };

            // If we failed, retry a couple times before returning the last error
            retries += 1;
            if retries >= max_retries {
                return Err(err);
            }
            info!("Failed to connect to process, retrying. Error: {}", err);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Gets a StackTrace for each thread in the current process
    pub fn get_stack_traces(&mut self) -> Result<Vec<StackTrace>, Error> {
        match self.version {
            // ABI for 2.3/2.4/2.5/2.6/2.7 is compatible for our purpose
            Version {
                major: 2,
                minor: 3..=7,
                ..
            } => self._get_stack_traces::<v2_7_15::_is>(),
            Version {
                major: 3, minor: 3, ..
            } => self._get_stack_traces::<v3_3_7::_is>(),
            // ABI for 3.4 and 3.5 is the same for our purposes
            Version {
                major: 3, minor: 4, ..
            } => self._get_stack_traces::<v3_5_5::_is>(),
            Version {
                major: 3, minor: 5, ..
            } => self._get_stack_traces::<v3_5_5::_is>(),
            Version {
                major: 3, minor: 6, ..
            } => self._get_stack_traces::<v3_6_6::_is>(),
            Version {
                major: 3, minor: 7, ..
            } => self._get_stack_traces::<v3_7_0::_is>(),
            Version {
                major: 3, minor: 8, ..
            } => self._get_stack_traces::<v3_8_0::_is>(),
            Version {
                major: 3, minor: 9, ..
            } => self._get_stack_traces::<v3_9_5::_is>(),
            Version {
                major: 3,
                minor: 10,
                ..
            } => self._get_stack_traces::<v3_10_0::_is>(),
            Version {
                major: 3,
                minor: 11,
                ..
            } => self._get_stack_traces::<v3_11_0::_is>(),
            Version {
                major: 3,
                minor: 12,
                ..
            } => self._get_stack_traces::<v3_12_0::_is>(),
            Version {
                major: 3,
                minor: 13,
                ..
            } => self._get_stack_traces::<v3_13_0::_is>(),
            Version {
                major: 3,
                minor: 14,
                ..
            } => self._get_stack_traces::<v3_14_0::_is>(),
            _ => Err(format_err!(
                "Unsupported version of Python: {}",
                self.version
            )),
        }
    }

    // implementation of get_stack_traces, where we have a type for the InterpreterState
    fn _get_stack_traces<I: InterpreterState>(&mut self) -> Result<Vec<StackTrace>, Error> {
        // Query the OS to get if each thread in the process is running or not
        let mut thread_activity = HashMap::new();
        if self.config.gil_only {
            // Don't need to collect thread activity if we're only getting the
            // GIL thread: If we're holding the GIL we're by definition active.
        } else {
            for thread in self.process.threads()?.iter() {
                let threadid: Tid = thread.id()?;
                let Ok(active) = thread.active() else {
                    // Do not fail all sampling if a single thread died between entering the loop
                    // and reading its status.
                    continue;
                };
                thread_activity.insert(threadid, active);
            }
        }

        // Lock the process if appropriate. Note we have to lock AFTER getting the thread
        // activity status from the OS (otherwise each thread would report being inactive always).
        // This has the potential for race conditions (in that the thread activity could change
        // between getting the status and locking the thread, but seems unavoidable right now
        let _lock = if self.config.blocking == LockingStrategy::Lock {
            Some(self.process.lock().context("Failed to suspend process")?)
        } else {
            None
        };

        // Find PyThreadState, and loop over all the python threads
        let threadstate_ptr_ptr = I::threadstate_ptr_ptr(self.interpreter_address);
        let threads_head = self
            .process
            .copy_pointer(threadstate_ptr_ptr)
            .context("Failed to copy PyThreadState head pointer")?;

        // get the threadid of the gil if appropriate
        let gil_thread_id = get_gil_threadid::<I, Process>(self.threadstate_address, &self.process)
            .context("failed to get gil_thread_id")?;

        let mut traces = Vec::new();
        let mut threads = threads_head;
        while !threads.is_null() {
            // Get the stack trace of the python thread
            let thread = self
                .process
                .copy_pointer(threads)
                .context("Failed to copy PyThreadState")?;
            threads = thread.next();

            let python_thread_id = thread.thread_id();
            let owns_gil = python_thread_id == gil_thread_id;

            if self.config.gil_only && !owns_gil {
                continue;
            }

            let mut trace = get_stack_trace(
                &thread,
                &self.process,
                self.config.dump_locals > 0,
                self.config.lineno,
                Some(&mut self.frame_cache),
            )
            .with_context(|| {
                format!(
                    "Failed to call get_stack_trace for thread {}",
                    python_thread_id
                )
            })?;

            // Try getting the native thread id

            // python 3.11+ has the native thread id directly on the PyThreadState object,
            // for older versions of python, try using OS specific code to get the native
            // thread id (doesn't work on freebsd, or on arm/i686 processors on linux)
            if trace.os_thread_id.is_none() {
                let mut os_thread_id =
                    self._get_os_thread_id::<I>(python_thread_id, threads_head)?;

                // linux can see issues where pthread_ids get recycled for new OS threads,
                // which totally breaks the caching we were doing here. Detect this and retry
                if let Some(tid) = os_thread_id {
                    if !thread_activity.is_empty() && !thread_activity.contains_key(&tid) {
                        info!("clearing away thread id caches, thread {} has exited", tid);
                        self.python_thread_ids.clear();
                        self.python_thread_names.clear();
                        os_thread_id =
                            self._get_os_thread_id::<I>(python_thread_id, threads_head)?;
                    }
                }

                trace.os_thread_id = os_thread_id.map(|id| id as u64);
            }

            if self.config.include_thread_names {
                trace.thread_name = self._get_python_thread_name(python_thread_id);
            }
            trace.owns_gil = owns_gil;
            trace.pid = self.process.pid;

            // Figure out if the thread is sleeping from the OS if possible
            trace.active = true;
            if let Some(id) = trace.os_thread_id {
                let id = id as Tid;
                if let Some(active) = thread_activity.get(&id as _) {
                    trace.active = *active;
                }
            }

            // fallback to using a heuristic if we think the thread is still active
            // Note that on linux the OS thread activity can only be gotten on x86_64
            // processors and even then seems to be wrong occasionally in thinking 'select'
            // calls are active (which seems related to the thread locking code,
            // this problem doesn't seem to happen with the --nonblocking option)
            if trace.active {
                trace.active = !self._heuristic_is_thread_idle(&trace);
            }

            for frame in &mut trace.frames {
                frame.short_filename = self.shorten_filename(&frame.filename);
                if let Some(locals) = frame.locals.as_mut() {
                    let max_length = (128 * self.config.dump_locals) as isize;
                    for local in locals {
                        let repr = format_variable::<I, Process>(
                            &self.process,
                            &self.version,
                            local.addr,
                            max_length,
                        );
                        local.repr = Some(repr.unwrap_or_else(|_| "?".to_owned()));
                    }
                }
            }

            traces.push(trace);

            // This seems to happen occasionally when scanning BSS addresses for valid interpreters
            if traces.len() > 4096 {
                return Err(format_err!("Max thread recursion depth reached"));
            }

            if self.config.gil_only {
                // There's only one GIL thread and we've captured it, so we can
                // stop now
                break;
            }
        }
        Ok(traces)
    }

    // heuristic fallback for determining if a thread is active, used
    // when we don't have the ability to get the thread information from the OS
    fn _heuristic_is_thread_idle(&self, trace: &StackTrace) -> bool {
        let frames = &trace.frames;
        if frames.is_empty() {
            // we could have 0 python frames, but still be active running native
            // code.
            false
        } else {
            let frame = &frames[0];
            (frame.name == "wait" && frame.filename.ends_with("threading.py"))
                || (frame.name == "select" && frame.filename.ends_with("selectors.py"))
                || (frame.name == "poll"
                    && (frame.filename.ends_with("asyncore.py")
                        || frame.filename.contains("zmq")
                        || frame.filename.contains("gevent")
                        || frame.filename.contains("tornado")))
        }
    }

    #[cfg(windows)]
    fn _get_os_thread_id<I: InterpreterState>(
        &mut self,
        python_thread_id: u64,
        _interp_head: *const I::ThreadState,
    ) -> Result<Option<Tid>, Error> {
        Ok(Some(python_thread_id as Tid))
    }

    #[cfg(target_os = "macos")]
    fn _get_os_thread_id<I: InterpreterState>(
        &mut self,
        python_thread_id: u64,
        _interp_head: *const I::ThreadState,
    ) -> Result<Option<Tid>, Error> {
        // If we've already know this threadid, we're good
        if let Some(thread_id) = self.python_thread_ids.get(&python_thread_id) {
            return Ok(Some(*thread_id));
        }

        for thread in self.process.threads()?.iter() {
            // ok, this is crazy pants. is this 224 constant right?  Is this right for all versions of OSX? how is this determined?
            // is this correct for all versions of python? Why does this even work?
            let current_handle = thread.thread_handle()? - 224;
            self.python_thread_ids.insert(current_handle, thread.id()?);
        }

        if let Some(thread_id) = self.python_thread_ids.get(&python_thread_id) {
            return Ok(Some(*thread_id));
        }
        Ok(None)
    }

    #[cfg(target_os = "linux")]
    fn _get_os_thread_id<I: InterpreterState>(
        &mut self,
        _python_thread_id: u64,
        _interp_head: *const I::ThreadState,
    ) -> Result<Option<Tid>, Error> {
        Ok(None)
    }

    #[cfg(target_os = "freebsd")]
    fn _get_os_thread_id<I: InterpreterState>(
        &mut self,
        _python_thread_id: u64,
        _interp_head: *const I::ThreadState,
    ) -> Result<Option<Tid>, Error> {
        Ok(None)
    }

    fn _get_python_thread_name(&mut self, python_thread_id: u64) -> Option<String> {
        match self.python_thread_names.get(&python_thread_id) {
            Some(thread_name) => Some(thread_name.clone()),
            None => {
                self.python_thread_names = thread_name_lookup(self).unwrap_or_default();
                self.python_thread_names.get(&python_thread_id).cloned()
            }
        }
    }

    /// We want to display filenames without the boilerplate of the python installation
    /// directory etc. This function looks only includes paths inside a python
    /// package or subpackage, and not the path the package is installed at
    fn shorten_filename(&mut self, filename: &str) -> Option<String> {
        // if the user requested full filenames, skip shortening
        if self.config.full_filenames {
            return Some(filename.to_string());
        }

        // if we have figured out the short filename already, use it
        if let Some(short) = self.short_filenames.get(filename) {
            return short.clone();
        }

        // on linux the process could be running in docker, access the filename through procfs
        #[cfg(target_os = "linux")]
        let filename_storage;

        #[cfg(target_os = "linux")]
        let filename = if self.dockerized {
            filename_storage = format!("/proc/{}/root{}", self.pid, filename);
            if Path::new(&filename_storage).exists() {
                &filename_storage
            } else {
                filename
            }
        } else {
            filename
        };

        // only include paths that include an __init__.py
        let mut path = Path::new(filename);
        while let Some(parent) = path.parent() {
            path = parent;
            if !parent.join("__init__.py").exists() {
                break;
            }
        }

        // remove the parent prefix and convert to an optional string
        let shortened = Path::new(filename)
            .strip_prefix(path)
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        self.short_filenames
            .insert(filename.to_owned(), shortened.clone());
        shortened
    }
}
