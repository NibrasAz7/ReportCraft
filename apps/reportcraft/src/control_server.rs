//! TCP JSON-lines control server on `127.0.0.1:PORT`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use rc_ui_egui::control::ControlRequest;
use serde_json::{json, Value};

const MAX_LINE: usize = 8 * 1024 * 1024;
const MAX_CONNECTIONS: usize = 8;
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);

pub fn spawn(port: u16, ctx: egui::Context) -> Receiver<ControlRequest> {
    let (tx, rx) = std::sync::mpsc::channel();
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("control server failed to bind 127.0.0.1:{port}: {e}");
            return rx;
        }
    };
    eprintln!("control server listening on 127.0.0.1:{port}");
    std::thread::spawn(move || accept_loop(listener, tx, ctx));
    rx
}

struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn accept_loop(listener: TcpListener, tx: Sender<ControlRequest>, ctx: egui::Context) {
    let open = Arc::new(AtomicUsize::new(0));
    for mut stream in listener.incoming().flatten() {
        if open.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            open.fetch_sub(1, Ordering::SeqCst);
            let _ = writeln!(stream, "{}", json!({"ok": false, "error": "too many control connections"}));
            continue;
        }
        let slot = Slot(Arc::clone(&open));
        let tx = tx.clone();
        let ctx = ctx.clone();
        let _ = std::thread::Builder::new().spawn(move || {
            let _slot = slot;
            serve(stream, tx, ctx);
        });
    }
}

fn read_bounded_line(reader: &mut impl BufRead, buf: &mut Vec<u8>, max: usize) -> Result<bool, String> {
    buf.clear();
    loop {
        let avail = reader.fill_buf().map_err(|e| e.to_string())?;
        if avail.is_empty() {
            return Ok(!buf.is_empty());
        }
        let take = match avail.iter().position(|&b| b == b'\n') {
            Some(pos) => pos + 1,
            None => avail.len(),
        };
        buf.extend_from_slice(avail.get(..take).unwrap_or(&[]));
        let done = buf.last() == Some(&b'\n');
        reader.consume(take);
        if buf.len() > max {
            return Err(format!("request line longer than {max} bytes; closing the connection"));
        }
        if done {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            return Ok(true);
        }
    }
}

fn serve(stream: std::net::TcpStream, tx: Sender<ControlRequest>, ctx: egui::Context) {
    let Ok(read) = stream.try_clone() else { return };
    let mut out = stream;
    let mut reader = BufReader::new(read);
    let mut buf = Vec::new();
    loop {
        match read_bounded_line(&mut reader, &mut buf, MAX_LINE) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => {
                let _ = writeln!(out, "{}", json!({"ok": false, "error": e}));
                break;
            }
        }
        let Ok(line) = std::str::from_utf8(&buf) else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(msg) => {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
                let params = msg.get("params").cloned().unwrap_or(json!({}));
                let (req, pending) = ControlRequest::new(method, params);
                if tx.send(req).is_err() {
                    break;
                }
                ctx.request_repaint();
                let mut r = pending.wait(REPLY_TIMEOUT);
                if let Some(o) = r.as_object_mut() {
                    o.insert("id".into(), id);
                }
                r
            }
            Err(e) => json!({"ok": false, "error": format!("bad JSON: {e}")}),
        };
        if writeln!(out, "{reply}").is_err() {
            break;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn bounded_line_strips_crlf_and_enforces_limit() {
        let mut r = Cursor::new(b"hello\r\n".to_vec());
        let mut buf = Vec::new();
        assert!(read_bounded_line(&mut r, &mut buf, 16).unwrap());
        assert_eq!(buf, b"hello");

        let mut r2 = Cursor::new(vec![b'a'; 32]);
        assert!(read_bounded_line(&mut r2, &mut buf, 8).is_err());
    }
}
