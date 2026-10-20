// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! The front-end server: static files for the web UI plus a
//! WebSocket channel that carries the shared spatial program.
//!
//! State is one Piet grid, its serialized Brainfuck, and its
//! compile report. Any connected client may replace the grid
//! (`{"type":"grid","grid":<text>}`), compile raw Brainfuck
//! (`{"type":"compile","bf":<source>}` — the grid is re-laid-out
//! from it), or lay a program out on the field
//! (`{"type":"layout","bf":<source>,"width":N}`). Every change is
//! broadcast to all clients as
//! `{"type":"state","seq":N,"grid":...,"bf":...,"report":{...}}`,
//! so several browsers can watch and edit the same spatial
//! program live. On connect a client receives
//! `{"type":"hello","palette":[...],"state":{...}}`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::json;
use crate::piet::Grid;
use crate::ws;

/// Width used when laying a program out on the field.
pub const DEFAULT_WIDTH: usize = 24;

struct Shared {
    seq: u64,
    grid_text: String,
    bf: String,
    report: String,
    clients: Vec<(u64, Sender<String>)>,
}

impl Shared {
    fn state_json(&self) -> String {
        format!(
            "{{\"seq\":{},\"grid\":\"{}\",\"bf\":\"{}\",\"report\":{}}}",
            self.seq,
            json::escape(&self.grid_text),
            json::escape(&self.bf),
            self.report
        )
    }

    fn broadcast(&mut self) {
        let message = format!("{{\"type\":\"state\",\"state\":{}}}", self.state_json());
        self.clients
            .retain(|(_, tx)| tx.send(message.clone()).is_ok());
    }

    /// Replace the program from a grid: serialize, compile,
    /// advance the sequence, and broadcast.
    fn set_grid(&mut self, grid: Grid) {
        self.bf = grid.to_bf();
        self.grid_text = grid.to_text();
        self.report = crate::compile_report(&self.bf);
        self.seq += 1;
        self.broadcast();
    }

    /// Replace the program from raw Brainfuck: lay it out on the
    /// field at the current width and broadcast.
    fn set_bf(&mut self, source: &str, width: usize) {
        let grid = Grid::from_bf(source, width.max(1));
        self.set_grid(grid);
    }
}

/// Serve forever: HTTP static files from `web_root` and WebSocket
/// upgrades on the same port.
pub fn serve(listener: TcpListener, web_root: PathBuf) -> std::io::Result<()> {
    let initial = Grid::from_bf("+++>+++++>--.<.<.", DEFAULT_WIDTH);
    let shared = Arc::new(Mutex::new(Shared {
        seq: 0,
        bf: initial.to_bf(),
        grid_text: initial.to_text(),
        report: crate::compile_report(&initial.to_bf()),
        clients: Vec::new(),
    }));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let shared = Arc::clone(&shared);
        let web_root = web_root.clone();
        thread::spawn(move || {
            let _ = handle_connection(stream, shared, web_root);
        });
    }
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    shared: Arc<Mutex<Shared>>,
    web_root: PathBuf,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let (request_line, headers) = read_request(&mut reader)?;
    let is_upgrade = headers
        .get("upgrade")
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    if is_upgrade {
        let Some(key) = headers.get("sec-websocket-key") else {
            return respond(&mut stream, 400, "text/plain", b"missing Sec-WebSocket-Key");
        };
        let accept = ws::accept_key(key);
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        );
        stream.write_all(response.as_bytes())?;
        return ws_session(reader, stream, shared);
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    serve_static(&mut stream, &web_root, path)
}

fn read_request(
    reader: &mut BufReader<TcpStream>,
) -> std::io::Result<(String, HashMap<String, String>)> {
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    Ok((request_line.trim_end().to_string(), headers))
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn respond(stream: &mut TcpStream, status: u32, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn serve_static(stream: &mut TcpStream, web_root: &Path, path: &str) -> std::io::Result<()> {
    let rel = if path == "/" {
        "index.html"
    } else {
        &path[1..]
    };
    if rel.contains("..") {
        return respond(stream, 404, "text/plain", b"not found");
    }
    let file = web_root.join(rel);
    match std::fs::read(&file) {
        Ok(body) => respond(stream, 200, content_type(&file), &body),
        Err(_) => respond(stream, 404, "text/plain", b"not found"),
    }
}

fn ws_session(
    mut reader: BufReader<TcpStream>,
    stream: TcpStream,
    shared: Arc<Mutex<Shared>>,
) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let my_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = channel::<String>();
    {
        let mut shared = shared.lock().unwrap();
        let hello = format!(
            "{{\"type\":\"hello\",\"palette\":{},\"state\":{}}}",
            crate::piet::palette_json(),
            shared.state_json()
        );
        shared.clients.push((my_id, tx.clone()));
        let _ = tx.send(hello);
    }
    // Writer thread: channel -> socket.
    let mut write_half = stream.try_clone()?;
    let writer = thread::spawn(move || {
        while let Ok(message) = rx.recv() {
            if ws::write_frame(&mut write_half, ws::OP_TEXT, message.as_bytes()).is_err() {
                break;
            }
        }
    });
    // Reader loop on this thread.
    let mut fragments: Vec<u8> = Vec::new();
    while let Ok(frame) = ws::read_frame(&mut reader) {
        match frame.opcode {
            ws::OP_CLOSE => {
                let _ = tx_send_close(&stream);
                break;
            }
            ws::OP_PING => {
                if let Ok(mut s) = stream.try_clone() {
                    let _ = ws::write_frame(&mut s, ws::OP_PONG, &frame.payload);
                }
            }
            ws::OP_PONG => {}
            ws::OP_TEXT | ws::OP_BINARY | ws::OP_CONTINUATION => {
                fragments.extend_from_slice(&frame.payload);
                if frame.fin {
                    let text = String::from_utf8_lossy(&fragments).to_string();
                    fragments.clear();
                    handle_message(&text, &shared);
                }
            }
            _ => {}
        }
    }
    shared
        .lock()
        .unwrap()
        .clients
        .retain(|(id, _)| *id != my_id);
    drop(tx);
    let _ = writer.join();
    Ok(())
}

fn tx_send_close(stream: &TcpStream) -> std::io::Result<()> {
    let mut s = stream.try_clone()?;
    ws::write_frame(&mut s, ws::OP_CLOSE, &[])
}

fn handle_message(text: &str, shared: &Arc<Mutex<Shared>>) {
    let Some(kind) = json::get_string(text, "type") else {
        return;
    };
    let mut shared = shared.lock().unwrap();
    match kind.as_str() {
        "grid" => {
            if let Some(grid_text) = json::get_string(text, "grid") {
                shared.set_grid(Grid::from_text(&grid_text));
            }
        }
        "compile" => {
            if let Some(bf) = json::get_string(text, "bf") {
                let width = current_width(&shared);
                shared.set_bf(&bf, width);
            }
        }
        "layout" => {
            if let Some(bf) = json::get_string(text, "bf") {
                let width = json::get_number(text, "width")
                    .map(|w| w as usize)
                    .unwrap_or(DEFAULT_WIDTH);
                shared.set_bf(&bf, width);
            }
        }
        _ => {}
    }
}

fn current_width(shared: &Shared) -> usize {
    shared
        .grid_text
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(DEFAULT_WIDTH)
        .max(1)
}

/// Read exactly `n` bytes (used by tests and the frame layer).
pub fn read_n(reader: &mut impl Read, n: usize) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0u8; n];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}
