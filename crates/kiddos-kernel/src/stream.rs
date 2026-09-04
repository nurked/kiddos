//! stdin/stdout/stderr endpoints and the pipe that connects processes.

use parking_lot::{Condvar, Mutex};
use std::collections::VecDeque;
use std::sync::Arc;

const PIPE_CAPACITY: usize = 64 * 1024;

struct PipeState {
    buf: VecDeque<u8>,
    writers: usize,
    readers: usize,
}

/// A byte pipe with back-pressure. Dropping the last writer means EOF for the
/// reader; dropping the last reader makes writes fail (our SIGPIPE).
pub struct Pipe {
    state: Mutex<PipeState>,
    cv: Condvar,
}

impl Pipe {
    pub fn pair() -> (PipeReader, PipeWriter) {
        let p = Arc::new(Pipe {
            state: Mutex::new(PipeState {
                buf: VecDeque::new(),
                writers: 1,
                readers: 1,
            }),
            cv: Condvar::new(),
        });
        (PipeReader { pipe: p.clone() }, PipeWriter { pipe: p })
    }
}

pub struct PipeReader {
    pipe: Arc<Pipe>,
}

pub struct PipeWriter {
    pipe: Arc<Pipe>,
}

impl PipeReader {
    /// Read up to `max` bytes. Empty result means EOF.
    /// `should_stop` is polled while blocked so a killed process can leave.
    pub fn read(&self, max: usize, should_stop: &dyn Fn() -> bool) -> Vec<u8> {
        let mut st = self.pipe.state.lock();
        loop {
            if !st.buf.is_empty() {
                let n = max.min(st.buf.len());
                let out: Vec<u8> = st.buf.drain(..n).collect();
                drop(st);
                self.pipe.cv.notify_all();
                return out;
            }
            if st.writers == 0 || should_stop() {
                return Vec::new();
            }
            self.pipe.cv.wait_for(&mut st, std::time::Duration::from_millis(50));
        }
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        let mut st = self.pipe.state.lock();
        st.readers -= 1;
        drop(st);
        self.pipe.cv.notify_all();
    }
}

impl PipeWriter {
    /// Returns false if there is no reader any more.
    pub fn write(&self, mut data: &[u8], should_stop: &dyn Fn() -> bool) -> bool {
        let mut st = self.pipe.state.lock();
        while !data.is_empty() {
            if st.readers == 0 {
                return false;
            }
            if should_stop() {
                return false;
            }
            let room = PIPE_CAPACITY.saturating_sub(st.buf.len());
            if room == 0 {
                self.pipe.cv.wait_for(&mut st, std::time::Duration::from_millis(50));
                continue;
            }
            let n = room.min(data.len());
            st.buf.extend(&data[..n]);
            data = &data[n..];
            self.pipe.cv.notify_all();
        }
        true
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        let mut st = self.pipe.state.lock();
        st.writers -= 1;
        drop(st);
        self.pipe.cv.notify_all();
    }
}

/// Where a process's stdin comes from.
pub enum Input {
    /// The keyboard (canonical line mode).
    Tty,
    Null,
    Pipe(PipeReader),
    /// Whole contents of a file, already read.
    Bytes(Mutex<(Vec<u8>, usize)>),
}

impl Input {
    pub fn bytes(data: Vec<u8>) -> Input {
        Input::Bytes(Mutex::new((data, 0)))
    }
    pub fn is_tty(&self) -> bool {
        matches!(self, Input::Tty)
    }
}

/// Where a process's stdout/stderr go.
pub enum Output {
    /// The screen.
    Tty,
    Null,
    Pipe(PipeWriter),
    /// A VFS file, written through on every write. `append` chooses `>>`.
    File {
        path: String,
    },
    /// `/dev/speaker`: text is spoken at newlines and at close.
    Speaker(Mutex<String>),
}

impl Output {
    pub fn is_tty(&self) -> bool {
        matches!(self, Output::Tty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_roundtrip_and_eof() {
        let (r, w) = Pipe::pair();
        let t = std::thread::spawn(move || {
            assert!(w.write(b"hello ", &|| false));
            assert!(w.write(b"world", &|| false));
        });
        t.join().unwrap();
        let mut all = Vec::new();
        loop {
            let chunk = r.read(4, &|| false);
            if chunk.is_empty() {
                break;
            }
            all.extend(chunk);
        }
        assert_eq!(all, b"hello world");
    }

    #[test]
    fn broken_pipe() {
        let (r, w) = Pipe::pair();
        drop(r);
        assert!(!w.write(b"x", &|| false));
    }

    #[test]
    fn backpressure() {
        let (r, w) = Pipe::pair();
        let big = vec![b'a'; PIPE_CAPACITY * 3];
        let t = std::thread::spawn(move || w.write(&big, &|| false));
        let mut total = 0;
        loop {
            let c = r.read(8192, &|| false);
            if c.is_empty() {
                break;
            }
            total += c.len();
        }
        assert!(t.join().unwrap());
        assert_eq!(total, PIPE_CAPACITY * 3);
    }
}
