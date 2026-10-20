// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

use std::fmt;

/// Ways compilation can fail. Each variant renders exactly the
/// diagnostic the v0.1 command-line tool printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// `[` or `]`: data-dependent loops need sequential feedback/state.
    DynamicLoop { character: usize },
    /// `,`: streaming input has no temporal semantics yet.
    StreamingInput { character: usize },
    /// The addressed span exceeds the resource guard.
    TooManyCells { count: usize },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::DynamicLoop { character } => write!(
                f,
                "dynamic loop at character {character}: feedback synthesis not implemented"
            ),
            CompileError::StreamingInput { character } => write!(
                f,
                "input at character {character}: streaming input synthesis not implemented"
            ),
            CompileError::TooManyCells { .. } => {
                write!(f, "more than 4096 addressed cells: resource guard")
            }
        }
    }
}

impl std::error::Error for CompileError {}
