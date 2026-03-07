use std::ffi::OsString;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn run_install(cmd: &str) -> i32 {
    let args: Vec<OsString> = std::iter::once(OsString::from("uv"))
        .chain(cmd.split_whitespace().map(OsString::from))
        .collect();
    unsafe { uv::main(args) };
    0
}

#[pyo3::pyfunction]
fn run(cmd: &str) -> pyo3::PyResult<(i32, String, String)> {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());

    #[cfg(unix)]
    {
        let mut fds = [0i32; 2];
        unsafe { libc::pipe(fds.as_mut_ptr()) };
        let (pipe_r, pipe_w) = (fds[0], fds[1]);
        let saved = unsafe { libc::dup(2) };
        unsafe { libc::dup2(pipe_w, 2); libc::close(pipe_w); }
        let rc = run_install(cmd);
        unsafe { libc::dup2(saved, 2); libc::close(saved); }
        unsafe { libc::fcntl(pipe_r, libc::F_SETFL, libc::O_NONBLOCK); }
        let mut out = Vec::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = unsafe { libc::read(pipe_r, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 { break; }
            out.extend_from_slice(&buf[..n as usize]);
        }
        unsafe { libc::close(pipe_r); }
        return Ok((rc, String::new(), String::from_utf8_lossy(&out).into_owned()));
    }

    #[cfg(windows)]
    {
        use std::sync::{Arc, Mutex as StdMutex};
        use std::io::Read;
        use std::os::windows::io::{IntoRawHandle, FromRawHandle};

        let (mut reader, writer) = os_pipe::pipe().map_err(|e|
            pyo3::exceptions::PyRuntimeError::new_err(format!("pipe failed: {}", e))
        )?;

        let stderr_buf = Arc::new(StdMutex::new(Vec::<u8>::new()));
        let stderr_buf2 = stderr_buf.clone();
        let reader_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = reader.read_to_end(&mut buf);
            *stderr_buf2.lock().unwrap() = buf;
        });

        let writer_fd = writer.into_raw_handle() as i32;
        let rc = unsafe {
            let saved = libc::dup(2);
            libc::dup2(writer_fd, 2);
            let rc = run_install(cmd);
            libc::dup2(saved, 2);
            libc::close(saved);
            drop(std::fs::File::from_raw_handle(writer_fd as _));
            rc
        };

        let _ = reader_thread.join();
        let stderr_str = {
            let buf = stderr_buf.lock().unwrap();
            String::from_utf8_lossy(&buf).into_owned()
        };
        return Ok((rc, String::new(), stderr_str));
    }

    #[allow(unreachable_code)]
    Ok((run_install(cmd), String::new(), String::new()))
}

#[pyo3::pymodule]
fn uv_ffi(_py: pyo3::Python, m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(run, m)?)?;
    Ok(())
}
