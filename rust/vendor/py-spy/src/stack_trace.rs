use std::num::NonZeroUsize;
use std::sync::Arc;

use anyhow::{Context, Error, Result};
use lru::LruCache;

use remoteprocess::{Pid, ProcessMemory};
use serde_derive::Serialize;

use crate::config::{Config, LineNo};
use crate::python_data_access::{copy_bytes, copy_string};
use crate::python_interpreters::{
    CodeObject, FrameObject, InterpreterState, ThreadState, TupleObject,
};

const FRAME_CACHE_CAPACITY: usize = 4096;

/// Code-object-derived fields cached per code-object identity.
pub struct CachedFrame {
    pub name: String,
    pub filename: String,
    pub module: Option<String>,
    pub line: i32,
}

/// LRU cache keyed on (code_ptr, lasti, first_lineno, name_ptr, filename_ptr, line_table_ptr).
/// A new code object at the same address carries different string/table pointers, so
/// stale entries from a freed code object cannot be returned.
pub type FrameCache = LruCache<(usize, i32, i32, usize, usize, usize), CachedFrame>;

pub fn new_frame_cache() -> FrameCache {
    LruCache::new(NonZeroUsize::new(FRAME_CACHE_CAPACITY).unwrap())
}

/// Call stack for a single python thread
#[derive(Debug, Clone, Serialize)]
pub struct StackTrace {
    /// The process id than generated this stack trace
    pub pid: Pid,
    /// The python thread id for this stack trace
    pub thread_id: u64,
    // The python thread name for this stack trace
    pub thread_name: Option<String>,
    /// The OS thread id for this stack tracee
    pub os_thread_id: Option<u64>,
    /// Whether or not the thread was active
    pub active: bool,
    /// Whether or not the thread held the GIL
    pub owns_gil: bool,
    /// The frames
    pub frames: Vec<Frame>,
    /// process commandline / parent process info
    pub process_info: Option<Arc<ProcessInfo>>,
}

/// Information about a single function call in a stack trace
#[derive(Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Serialize)]
pub struct Frame {
    /// The function name
    pub name: String,
    /// The full filename of the file
    pub filename: String,
    /// The module/shared library the
    pub module: Option<String>,
    /// A short, more readable, representation of the filename
    pub short_filename: Option<String>,
    /// The line number inside the file (or 0 for native frames without line information)
    pub line: i32,
    /// Local Variables associated with the frame
    pub locals: Option<Vec<LocalVariable>>,
    /// If this is an entry frame. Each entry frame corresponds to one native frame (Python 3.11)
    pub is_entry: bool,
    /// If the last frame was a shim. This is used in Python 3.12+ to detect entry frames.
    pub is_shim_entry: bool,
}

#[derive(Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Serialize)]
pub struct LocalVariable {
    pub name: String,
    pub addr: usize,
    pub arg: bool,
    pub repr: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: Pid,
    pub command_line: String,
    pub parent: Option<Box<ProcessInfo>>,
}

/// Given an InterpreterState, this function returns a vector of stack traces for each thread
pub fn get_stack_traces<I, P>(
    interpreter_address: usize,
    process: &P,
    threadstate_address: usize,
    config: Option<&Config>,
) -> Result<Vec<StackTrace>, Error>
where
    I: InterpreterState,
    P: ProcessMemory,
{
    let gil_thread_id = get_gil_threadid::<I, P>(threadstate_address, process)?;

    let threadstate_ptr_ptr = I::threadstate_ptr_ptr(interpreter_address);
    let mut threads: *const I::ThreadState = process
        .copy_struct(threadstate_ptr_ptr as usize)
        .context("Failed to copy PyThreadState head pointer")?;

    let mut ret = Vec::new();

    let lineno = config.map(|c| c.lineno).unwrap_or(LineNo::NoLine);
    let dump_locals = config.map(|c| c.dump_locals).unwrap_or(0);

    while !threads.is_null() {
        let thread = process
            .copy_pointer(threads)
            .context("Failed to copy PyThreadState")?;

        let mut trace = get_stack_trace(&thread, process, dump_locals > 0, lineno, None)?;
        trace.owns_gil = trace.thread_id == gil_thread_id;

        ret.push(trace);
        // This seems to happen occasionally when scanning BSS addresses for valid interpreters
        if ret.len() > 4096 {
            return Err(format_err!("Max thread recursion depth reached"));
        }
        threads = thread.next();
    }
    Ok(ret)
}

/// Gets a stack trace for an individual thread.
/// Pass `Some(cache)` from a `PythonSpy` to skip redundant string reads on repeated samples.
/// The free-function variant `get_stack_traces` passes `None`.
pub fn get_stack_trace<T, P>(
    thread: &T,
    process: &P,
    copy_locals: bool,
    lineno: LineNo,
    cache: Option<&mut FrameCache>,
) -> Result<StackTrace, Error>
where
    T: ThreadState,
    P: ProcessMemory,
{
    let mut frames = Vec::new();

    // python 3.11+ has an extra level of indirection to get the Frame from the threadstate
    let mut frame_address = thread.frame_address();
    if let Some(addr) = frame_address {
        frame_address = Some(process.copy_struct(addr)?);
    }

    let mut frame_ptr = thread.frame(frame_address);

    // We are iterating in reverse, i.e. from last call to first call.
    // Since Python 3.12, there are shim frames inserted before a block
    // of Python frames. When we encounter one, update the last frame.
    let set_last_frame_as_shim_entry = &mut |frames: &mut Vec<Frame>| {
        if let Some(frame) = frames.last_mut() {
            frame.is_shim_entry = true;
        }
    };

    // Reborrow as &mut Option<&mut FrameCache> for use inside the loop.
    let mut cache = cache;

    while !frame_ptr.is_null() {
        let frame = process
            .copy_pointer(frame_ptr)
            .context("Failed to copy PyFrameObject")?;

        if !frame.is_python_frame() {
            frame_ptr = frame.back();
            set_last_frame_as_shim_entry(&mut frames);
            continue;
        }

        let code_ptr = frame.code() as usize;
        let code = process
            .copy_pointer(frame.code())
            .context("Failed to copy PyCodeObject")?;
        let lasti = frame.lasti();
        let first_lineno = code.first_lineno();
        let name_ptr = code.qualname().unwrap_or_else(|| code.name()) as usize;
        let filename_ptr = code.filename() as usize;
        let line_table_ptr = code.line_table() as usize;
        let cache_key = (
            code_ptr,
            lasti,
            first_lineno,
            name_ptr,
            filename_ptr,
            line_table_ptr,
        );

        if let Some(ref mut c) = cache {
            if let Some(cached) = c.get(&cache_key) {
                let locals = if copy_locals {
                    Some(
                        get_locals(&code, frame_ptr, &frame, process)
                            .context("Failed to get local variables")?,
                    )
                } else {
                    None
                };
                let is_entry = frame.is_entry();
                frames.push(Frame {
                    name: cached.name.clone(),
                    filename: cached.filename.clone(),
                    line: cached.line,
                    short_filename: None,
                    module: cached.module.clone(),
                    locals,
                    is_entry,
                    is_shim_entry: false,
                });
                if frames.len() > 4096 {
                    return Err(format_err!("Max frame recursion depth reached"));
                }
                frame_ptr = frame.back();
                continue;
            }
        }

        let filename = copy_string(code.filename(), process).context("Failed to copy filename");

        // Try to get qualname first (available in Python 3.11+), fall back to name
        let name = match code.qualname() {
            Some(qualname_ptr) => {
                copy_string(qualname_ptr, process).or_else(|_| copy_string(code.name(), process))
            }
            None => copy_string(code.name(), process),
        }
        .context("Failed to copy function name");

        // Guards against torn reads while sampling without pausing the process.
        if filename.is_err() || name.is_err() {
            frame_ptr = frame.back();
            set_last_frame_as_shim_entry(&mut frames);
            continue;
        }
        let filename = filename?;
        let name = name?;

        // skip <shim> entries in python 3.13+
        // Unset file/function name in py3.13 means this is a shim.
        if filename.is_empty() || filename == "<shim>" {
            frame_ptr = frame.back();
            set_last_frame_as_shim_entry(&mut frames);
            continue;
        }

        let line = match lineno {
            LineNo::NoLine => 0,
            LineNo::First => code.first_lineno(),
            LineNo::LastInstruction => match get_line_number(&code, frame.lasti(), process) {
                Ok(line) => line,
                Err(e) => {
                    // Failling to get the line number really shouldn't be fatal here, but
                    // can happen in extreme cases (https://github.com/benfred/py-spy/issues/164)
                    // Rather than fail set the linenumber to 0. This is used by the native extensions
                    // to indicate that we can't load a line number and it should be handled gracefully
                    warn!(
                        "Failed to get line number from {}.{}: {}",
                        filename, name, e
                    );
                    0
                }
            },
        };

        let locals = if copy_locals {
            Some(
                get_locals(&code, frame_ptr, &frame, process)
                    .context("Failed to get local variables")?,
            )
        } else {
            None
        };

        if let Some(ref mut c) = cache {
            c.put(
                cache_key,
                CachedFrame {
                    name: name.clone(),
                    filename: filename.clone(),
                    module: None,
                    line,
                },
            );
        }

        let is_entry = frame.is_entry();

        frames.push(Frame {
            name,
            filename,
            line,
            short_filename: None,
            module: None,
            locals,
            is_entry,
            is_shim_entry: false,
        });
        if frames.len() > 4096 {
            return Err(format_err!("Max frame recursion depth reached"));
        }

        frame_ptr = frame.back();
    }

    // First frame is always a shim
    set_last_frame_as_shim_entry(&mut frames);

    Ok(StackTrace {
        pid: 0,
        frames,
        thread_id: thread.thread_id(),
        thread_name: None,
        owns_gil: false,
        active: true,
        os_thread_id: thread.native_thread_id(),
        process_info: None,
    })
}

impl StackTrace {
    pub fn status_str(&self) -> &str {
        match (self.owns_gil, self.active) {
            (_, false) => "idle",
            (true, true) => "active+gil",
            (false, true) => "active",
        }
    }

    pub fn format_threadid(&self) -> String {
        // native threadids in osx are kinda useless, use the pthread id instead
        #[cfg(target_os = "macos")]
        return format!("{:#X}", self.thread_id);

        // otherwise use the native threadid if given
        #[cfg(not(target_os = "macos"))]
        match self.os_thread_id {
            Some(tid) => format!("{}", tid),
            None => format!("{:#X}", self.thread_id),
        }
    }
}

/// Returns the line number from a PyCodeObject (given the lasti index from a PyFrameObject)
fn get_line_number<C: CodeObject, P: ProcessMemory>(
    code: &C,
    lasti: i32,
    process: &P,
) -> Result<i32, Error> {
    let table =
        copy_bytes(code.line_table(), process).context("Failed to copy line number table")?;
    Ok(code.get_line_number(lasti, &table))
}

fn get_locals<C: CodeObject, F: FrameObject, P: ProcessMemory>(
    code: &C,
    frameptr: *const F,
    frame: &F,
    process: &P,
) -> Result<Vec<LocalVariable>, Error> {
    let local_count = code.nlocals() as usize;
    let argcount = code.argcount() as usize;
    let varnames = process
        .copy_pointer(code.varnames())
        .context("Failed to get varnames from PyCodeObject")?;

    let ptr_size = std::mem::size_of::<*const i32>();
    let locals_addr = frameptr as usize + std::mem::size_of_val(frame) - ptr_size;

    let mut ret = Vec::new();

    for i in 0..local_count {
        let nameptr: *const C::StringObject =
            process.copy_struct(varnames.address(code.varnames() as usize, i))?;

        let name = copy_string(nameptr, process).context("Failed to copy local variable name")?;
        let addr: usize = process.copy_struct(locals_addr + i * ptr_size)?;

        // hack: handle things like None, True, False, small integer constants etc on Python 3.14
        let addr = if addr & 1 == 1 { addr - 1 } else { addr };

        if addr == 0 {
            continue;
        }
        ret.push(LocalVariable {
            name,
            addr,
            arg: i < argcount,
            repr: None,
        });
    }
    Ok(ret)
}

pub fn get_gil_threadstate_addr<I: InterpreterState, P: ProcessMemory>(
    threadstate_address: usize,
    process: &P,
) -> Result<usize, Error> {
    if threadstate_address == 0 {
        return Ok(0);
    }
    if I::HAS_GIL_RUNTIME_STATE {
        let gil_state: crate::python_bindings::v3_13_0::_gil_runtime_state =
            process.copy_struct(threadstate_address)?;
        Ok(if gil_state.locked != 0 {
            gil_state.last_holder as usize
        } else {
            0
        })
    } else {
        Ok(process.copy_struct::<usize>(threadstate_address)?)
    }
}

pub fn get_gil_threadid<I: InterpreterState, P: ProcessMemory>(
    threadstate_address: usize,
    process: &P,
) -> Result<u64, Error> {
    let addr = get_gil_threadstate_addr::<I, P>(threadstate_address, process)?;
    let threadid = if addr != 0 {
        let threadstate: I::ThreadState = process.copy_struct(addr)?;
        threadstate.thread_id()
    } else {
        0
    };
    Ok(threadid)
}

impl ProcessInfo {
    pub fn to_frame(&self) -> Frame {
        Frame {
            name: format!("process {}:\"{}\"", self.pid, self.command_line),
            filename: String::from(""),
            module: None,
            short_filename: None,
            line: 0,
            locals: None,
            is_entry: true,
            is_shim_entry: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::python_bindings::v3_7_0::PyCodeObject;
    use crate::python_data_access::tests::to_byteobject;
    use remoteprocess::LocalProcess;

    #[test]
    fn test_get_line_number() {
        let mut lnotab = to_byteobject(&[0u8, 1, 10, 1, 8, 1, 4, 1]);
        let code = PyCodeObject {
            co_firstlineno: 3,
            co_lnotab: &mut lnotab.base.ob_base.ob_base,
            ..Default::default()
        };
        let lineno = get_line_number(&code, 30, &LocalProcess).unwrap();
        assert_eq!(lineno, 7);
    }

    #[test]
    fn test_frame_cache_key_discrimination() {
        let mut cache = new_frame_cache();
        let entry = CachedFrame {
            name: "foo".to_string(),
            filename: "a.py".to_string(),
            module: None,
            line: 10,
        };
        cache.put((1000usize, 5i32, 1i32, 100usize, 200usize, 300usize), entry);

        assert!(
            cache.get(&(1000, 5, 1, 100, 200, 300)).is_some(),
            "same key should hit"
        );
        assert!(
            cache.get(&(1000, 6, 1, 100, 200, 300)).is_none(),
            "different lasti should miss"
        );
        assert!(
            cache.get(&(1000, 5, 2, 100, 200, 300)).is_none(),
            "different first_lineno should miss"
        );
        assert!(
            cache.get(&(2000, 5, 1, 100, 200, 300)).is_none(),
            "different code_ptr should miss"
        );
        assert!(
            cache.get(&(1000, 5, 1, 999, 200, 300)).is_none(),
            "different name_ptr should miss"
        );
        assert!(
            cache.get(&(1000, 5, 1, 100, 999, 300)).is_none(),
            "different filename_ptr should miss"
        );
        assert!(
            cache.get(&(1000, 5, 1, 100, 200, 999)).is_none(),
            "different line_table_ptr should miss"
        );
    }

    #[cfg(feature = "counters")]
    #[test]
    fn test_frame_cache_reduces_reads() {
        use crate::config::LineNo;
        use crate::python_bindings::v3_7_0::{self as py, _frame, _ts};
        use crate::python_data_access::tests::to_asciiobject;

        struct CountingProcess;
        impl remoteprocess::ProcessMemory for CountingProcess {
            fn read(&self, addr: usize, buf: &mut [u8]) -> Result<(), remoteprocess::Error> {
                unsafe {
                    std::ptr::copy_nonoverlapping(addr as *mut u8, buf.as_mut_ptr(), buf.len());
                }
                remoteprocess::counters::add(buf.len());
                Ok(())
            }
        }

        let mut filename_obj = to_asciiobject("bench.py");
        let mut name_obj = to_asciiobject("bench_fn");
        let mut lnotab_obj = to_byteobject(&[]);

        let mut code = PyCodeObject {
            co_firstlineno: 1,
            co_filename: &mut filename_obj.base as *mut py::PyASCIIObject as *mut py::PyObject,
            co_name: &mut name_obj.base as *mut py::PyASCIIObject as *mut py::PyObject,
            co_lnotab: &mut lnotab_obj.base as *mut py::PyBytesObject as *mut py::PyObject,
            ..Default::default()
        };

        let mut frame = _frame {
            f_code: &mut code as *mut PyCodeObject,
            f_back: std::ptr::null_mut(),
            f_lasti: 0,
            ..Default::default()
        };

        let thread = _ts {
            frame: &mut frame as *mut _frame,
            next: std::ptr::null_mut(),
            ..Default::default()
        };

        let mut cache = new_frame_cache();

        // Reset counters before measuring.
        remoteprocess::counters::take();

        get_stack_trace(
            &thread,
            &CountingProcess,
            false,
            LineNo::LastInstruction,
            Some(&mut cache),
        )
        .unwrap();
        let (cold_reads, _) = remoteprocess::counters::take();

        get_stack_trace(
            &thread,
            &CountingProcess,
            false,
            LineNo::LastInstruction,
            Some(&mut cache),
        )
        .unwrap();
        let (warm_reads, _) = remoteprocess::counters::take();

        assert!(
            warm_reads * 3 < cold_reads,
            "expected warm reads ({warm_reads}) to be < 1/3 of cold reads ({cold_reads})"
        );
    }
}
