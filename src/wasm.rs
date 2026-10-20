// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! WebAssembly exports: a hand-rolled C ABI over the compiler, no
//! bindgen dependency. The page allocates input buffers with
//! `magma_alloc`, writes UTF-8 into the module's memory, calls an
//! entry point, and gets back a packed `u64`
//! (`pointer << 32 | length`) addressing a JSON string owned by
//! the module (valid until the next call). Inputs are freed with
//! `magma_free`.
//!
//! Entry points:
//! - `magma_compile(bf_ptr, bf_len)` — the compile report JSON
//!   (see [`crate::compile_report`]).
//! - `magma_grid_to_bf(grid_ptr, grid_len)` — a Piet grid in text
//!   form to `{"ok":true,"bf":...}`.
//! - `magma_bf_to_grid(bf_ptr, bf_len, width)` — a program laid
//!   out on a field: `{"ok":true,"grid":...}` (text form).
//! - `magma_palette()` — the Piet palette as JSON, so the page
//!   renders colors from the same table the library defines.

use std::sync::Mutex;

use crate::piet::Grid;

static RESULT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn set_result(json: String) -> u64 {
    let mut result = RESULT.lock().unwrap();
    *result = json.into_bytes();
    ((result.as_ptr() as u64) << 32) | (result.len() as u64)
}

fn input(ptr: *const u8, len: usize) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8(bytes.to_vec()).ok()
}

fn bad_input() -> u64 {
    set_result("{\"ok\":false,\"error\":\"input is not valid UTF-8\"}".to_string())
}

#[no_mangle]
pub extern "C" fn magma_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

#[no_mangle]
pub extern "C" fn magma_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe {
            drop(Vec::from_raw_parts(ptr, len, len));
        }
    }
}

#[no_mangle]
pub extern "C" fn magma_compile(ptr: *const u8, len: usize) -> u64 {
    match input(ptr, len) {
        Some(source) => set_result(crate::compile_report(&source)),
        None => bad_input(),
    }
}

#[no_mangle]
pub extern "C" fn magma_grid_to_bf(ptr: *const u8, len: usize) -> u64 {
    match input(ptr, len) {
        Some(text) => {
            let grid = Grid::from_text(&text);
            set_result(format!(
                "{{\"ok\":true,\"bf\":\"{}\"}}",
                crate::json::escape(&grid.to_bf())
            ))
        }
        None => bad_input(),
    }
}

#[no_mangle]
pub extern "C" fn magma_bf_to_grid(ptr: *const u8, len: usize, width: usize) -> u64 {
    match input(ptr, len) {
        Some(source) => {
            let grid = Grid::from_bf(&source, width.max(1));
            set_result(format!(
                "{{\"ok\":true,\"grid\":\"{}\"}}",
                crate::json::escape(&grid.to_text())
            ))
        }
        None => bad_input(),
    }
}

#[no_mangle]
pub extern "C" fn magma_palette() -> u64 {
    set_result(format!("{{\"palette\":{}}}", crate::piet::palette_json()))
}
