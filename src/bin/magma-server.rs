// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

use std::net::TcpListener;
use std::path::PathBuf;

// MAGMA front-end server: serves the web UI (Piet 2D spatial
// editor + WebAssembly compiler) and the WebSocket channel that
// keeps every connected browser on the same spatial program.
// Usage: magma-server [addr] [web-root]   (defaults 127.0.0.1:8765, web)
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let addr = args.get(1).map(String::as_str).unwrap_or("127.0.0.1:8765");
    let web_root = PathBuf::from(args.get(2).map(String::as_str).unwrap_or("web"));
    let listener = TcpListener::bind(addr).unwrap_or_else(|e| {
        eprintln!("cannot bind {addr}: {e}");
        std::process::exit(1);
    });
    println!(
        "magma-server listening on http://{} (web root: {})",
        listener.local_addr().unwrap(),
        web_root.display()
    );
    magma_spatial::server::serve(listener, web_root).unwrap_or_else(|e| {
        eprintln!("server error: {e}");
        std::process::exit(1);
    });
}
