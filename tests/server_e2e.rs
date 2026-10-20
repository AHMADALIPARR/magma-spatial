// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! Live end-to-end test of the front-end server: HTTP static
//! serving, the WebSocket handshake (checked against the RFC 6455
//! accept value), hello/state flow, grid edits broadcast to a
//! second client, and compile-from-BF.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

fn fixture_web_root() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("magma_e2e_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("index.html"),
        "<!doctype html><title>MAGMA</title>",
    )
    .unwrap();
    fs::write(dir.join("magma.wasm"), b"\0asm\x01\0\0\0").unwrap();
    dir
}

fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let root = fixture_web_root();
    std::thread::spawn(move || {
        let _ = magma_spatial::server::serve(listener, root);
    });
    port
}

fn http_get(port: u16, path: &str) -> (u32, Vec<(String, String)>, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    let status: u32 = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    let headers = head
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    // Body may contain non-UTF8 in general; here fixtures are text.
    let body = body.as_bytes().to_vec();
    (status, headers, body)
}

/// A minimal WebSocket client: handshake + masked text frames out,
/// unmasked frames in.
struct WsClient {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl WsClient {
    fn connect(port: u16) -> (WsClient, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "GET /ws HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )
        .unwrap();
        // Read the 101 response headers.
        let mut raw = Vec::new();
        let mut byte = [0u8; 1];
        while !raw.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            raw.push(byte[0]);
        }
        let head = String::from_utf8(raw).unwrap();
        assert!(head.contains("101"), "handshake failed: {head}");
        assert!(
            head.contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
            "bad accept value: {head}"
        );
        (
            WsClient {
                stream,
                buf: Vec::new(),
            },
            head,
        )
    }

    fn send_text(&mut self, text: &str) {
        let mask = [0x12, 0x34, 0x56, 0x78];
        let payload = text.as_bytes();
        let mut frame = vec![0x81];
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        for (i, b) in payload.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
        self.stream.write_all(&frame).unwrap();
    }

    fn read_text(&mut self) -> String {
        loop {
            if let Some(text) = self.try_parse_frame() {
                return text;
            }
            let mut chunk = [0u8; 4096];
            let n = self.stream.read(&mut chunk).unwrap();
            assert!(n > 0, "connection closed while waiting for a frame");
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    fn try_parse_frame(&mut self) -> Option<String> {
        if self.buf.len() < 2 {
            return None;
        }
        let opcode = self.buf[0] & 0x0F;
        let mut len = (self.buf[1] & 0x7F) as usize;
        let mut at = 2;
        if len == 126 {
            if self.buf.len() < 4 {
                return None;
            }
            len = u16::from_be_bytes([self.buf[2], self.buf[3]]) as usize;
            at = 4;
        }
        if self.buf.len() < at + len {
            return None;
        }
        let payload = self.buf[at..at + len].to_vec();
        self.buf.drain(..at + len);
        assert_eq!(opcode, 1, "expected a text frame");
        Some(String::from_utf8(payload).unwrap())
    }
}

#[test]
fn http_static_and_404() {
    let port = start_server();
    let (status, _, body) = http_get(port, "/");
    assert_eq!(status, 200);
    assert!(String::from_utf8(body).unwrap().contains("MAGMA"));
    let (status, headers, _) = http_get(port, "/magma.wasm");
    assert_eq!(status, 200);
    assert!(headers
        .iter()
        .any(|(k, v)| k == "content-type" && v == "application/wasm"));
    let (status, _, _) = http_get(port, "/nope.txt");
    assert_eq!(status, 404);
    let (status, _, _) = http_get(port, "/../Cargo.toml");
    assert_eq!(status, 404);
}

#[test]
fn websocket_grid_flow_broadcasts() {
    let port = start_server();
    let (mut a, _) = WsClient::connect(port);
    let hello = a.read_text();
    assert!(hello.contains("\"type\":\"hello\""), "{hello}");
    assert!(hello.contains("#E53935"), "palette missing: {hello}");
    assert!(hello.contains("+++>+++++>--.<.<."), "initial bf: {hello}");

    // A second client joins and edits the grid.
    let (mut b, _) = WsClient::connect(port);
    let _hello_b = b.read_text();
    b.send_text("{\"type\":\"grid\",\"grid\":\"++>\\n   -\"}");

    // Both clients receive the new shared state. The snake reads
    // row 1 right-to-left, so the bf is "++>-".
    for client in [&mut a, &mut b] {
        let state = client.read_text();
        assert!(state.contains("\"type\":\"state\""), "{state}");
        assert!(state.contains("\"bf\":\"++>-\""), "{state}");
        assert!(state.contains("\"cells\":2"), "{state}");
    }

    // Compiling raw BF re-lays-out the grid and broadcasts again.
    a.send_text("{\"type\":\"compile\",\"bf\":\"+.\"}");
    for client in [&mut a, &mut b] {
        let state = client.read_text();
        assert!(state.contains("\"bf\":\"+.\""), "{state}");
        assert!(state.contains("\"cells\":1"), "{state}");
        assert!(state.contains("\"taps\":1"), "{state}");
    }
}
