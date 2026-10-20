// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! The Turing machine: the temporal engine beside MAGMA's spatial
//! compiler. Where the compiler lowers a program to combinational
//! lanes (no program counter, no time), this machine *runs* a
//! program the classical way — a tape of 16-bit cells, a head, an
//! instruction pointer, one step at a time.
//!
//! It executes full Brainfuck, including the loops and input the
//! combinational stage rejects: the two engines are the two sides
//! of the line the README draws — finite combinational logic
//! cannot directly realize arbitrary unbounded execution, and this
//! is the execution it cannot realize. Cells are 16-bit wrapping,
//! matching MAGMA's logical cells.
//!
//! The machine is a pure value: [`Machine::step`] advances it by
//! one instruction. That makes it directly exposable over the
//! stateless WebAssembly ABI — the whole state crosses as JSON
//! per step, and the stepping logic lives here, once.

use crate::json;

/// A Turing machine executing a Brainfuck program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    /// The program's command stream (non-command characters are
    /// comments and were dropped at load).
    program: Vec<char>,
    /// Matching-bracket map: instruction index -> partner index.
    brackets: Vec<Option<usize>>,
    /// Tape cells, addressed from position 0 upward; the tape
    /// grows to the right as the head advances past its end.
    /// Moving left of position 0 is a machine error (the tape is
    /// left-bounded, as in the classical model variants used for
    /// teaching; MAGMA's spatial span is likewise finite).
    pub tape: Vec<u16>,
    /// Head position (a tape index).
    pub head: usize,
    /// Instruction pointer into `program`.
    pub ip: usize,
    /// Queued input values for `,`.
    pub input: Vec<u16>,
    /// Values emitted by `.`, in order.
    pub output: Vec<u16>,
    /// Total instructions executed.
    pub steps: u64,
    /// Why the machine stopped, if it has.
    pub halt: Option<Halt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Halt {
    /// The instruction pointer ran off the end of the program.
    End,
    /// The head moved left of the tape origin.
    TapeUnderflow,
    /// A step budget was exhausted (see [`Machine::run`]).
    StepLimit,
    /// Unbalanced brackets in the program.
    BadProgram,
}

impl Machine {
    /// Load a program. Unbalanced brackets halt the machine
    /// immediately with [`Halt::BadProgram`].
    pub fn new(source: &str) -> Machine {
        let program: Vec<char> = source
            .chars()
            .filter(|c| matches!(c, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']'))
            .collect();
        let mut brackets: Vec<Option<usize>> = vec![None; program.len()];
        let mut stack: Vec<usize> = Vec::new();
        let mut balanced = true;
        for (i, &c) in program.iter().enumerate() {
            match c {
                '[' => stack.push(i),
                ']' => match stack.pop() {
                    Some(open) => {
                        brackets[open] = Some(i);
                        brackets[i] = Some(open);
                    }
                    None => balanced = false,
                },
                _ => {}
            }
        }
        if !stack.is_empty() {
            balanced = false;
        }
        Machine {
            program,
            brackets,
            tape: vec![0],
            head: 0,
            ip: 0,
            input: Vec::new(),
            output: Vec::new(),
            steps: 0,
            halt: if balanced { None } else { Some(Halt::BadProgram) },
        }
    }

    /// Queue an input value for the next `,`.
    pub fn push_input(&mut self, value: u16) {
        self.input.push(value);
    }

    /// Rebuild a machine from a program and a saved state - the
    /// stateless stepping the WebAssembly ABI uses. A saved
    /// `step-limit` halt is a pause, not a stop: it resumes.
    pub fn resume(
        source: &str,
        tape: Vec<u16>,
        head: usize,
        ip: usize,
        input: Vec<u16>,
        output: Vec<u16>,
        steps: u64,
        halt: Option<Halt>,
    ) -> Machine {
        let mut m = Machine::new(source);
        m.tape = if tape.is_empty() { vec![0] } else { tape };
        m.head = head.min(m.tape.len() - 1);
        m.ip = ip;
        m.input = input;
        m.output = output;
        m.steps = steps;
        m.halt = match halt {
            Some(Halt::StepLimit) => None,
            other => other.or(m.halt),
        };
        m
    }

    pub fn halted(&self) -> bool {
        self.halt.is_some() || self.ip >= self.program.len()
    }

    /// The instruction under the pointer, if any.
    pub fn current(&self) -> Option<char> {
        self.program.get(self.ip).copied()
    }

    /// Execute one instruction. Returns false if the machine was
    /// already halted.
    pub fn step(&mut self) -> bool {
        if self.halt.is_some() {
            return false;
        }
        let Some(op) = self.current() else {
            self.halt = Some(Halt::End);
            return false;
        };
        match op {
            '>' => {
                self.head += 1;
                if self.head == self.tape.len() {
                    self.tape.push(0);
                }
                self.ip += 1;
            }
            '<' => {
                if self.head == 0 {
                    self.halt = Some(Halt::TapeUnderflow);
                    return true;
                }
                self.head -= 1;
                self.ip += 1;
            }
            '+' => {
                self.tape[self.head] = self.tape[self.head].wrapping_add(1);
                self.ip += 1;
            }
            '-' => {
                self.tape[self.head] = self.tape[self.head].wrapping_sub(1);
                self.ip += 1;
            }
            '.' => {
                self.output.push(self.tape[self.head]);
                self.ip += 1;
            }
            ',' => {
                if !self.input.is_empty() {
                    self.tape[self.head] = self.input.remove(0);
                }
                self.ip += 1;
            }
            '[' => {
                if self.tape[self.head] == 0 {
                    self.ip = self.brackets[self.ip].unwrap() + 1;
                } else {
                    self.ip += 1;
                }
            }
            ']' => {
                if self.tape[self.head] != 0 {
                    self.ip = self.brackets[self.ip].unwrap() + 1;
                } else {
                    self.ip += 1;
                }
            }
            _ => unreachable!(),
        }
        self.steps += 1;
        if self.ip >= self.program.len() {
            self.halt = Some(Halt::End);
        }
        true
    }

    /// Run up to `max_steps` more instructions (or to a halt).
    pub fn run(&mut self, max_steps: u64) {
        let deadline = self.steps + max_steps;
        while self.halt.is_none() && self.steps < deadline {
            self.step();
        }
        if self.halt.is_none() && self.ip < self.program.len() {
            self.halt = Some(Halt::StepLimit);
        }
    }

    /// Serialize the machine state as the flat JSON object the
    /// front end renders: tape, head, ip, next op, output, steps,
    /// and halt reason.
    pub fn to_json(&self) -> String {
        let tape: Vec<String> = self.tape.iter().map(|v| v.to_string()).collect();
        let output: Vec<String> = self.output.iter().map(|v| v.to_string()).collect();
        let input: Vec<String> = self.input.iter().map(|v| v.to_string()).collect();
        let halt = match self.halt {
            None => "null".to_string(),
            Some(Halt::End) => "\"end\"".to_string(),
            Some(Halt::TapeUnderflow) => "\"tape-underflow\"".to_string(),
            Some(Halt::StepLimit) => "\"step-limit\"".to_string(),
            Some(Halt::BadProgram) => "\"bad-program\"".to_string(),
        };
        let current = match self.current() {
            Some(c) => format!("\"{}\"", json::escape(&c.to_string())),
            None => "null".to_string(),
        };
        format!(
            "{{\"tape\":[{}],\"head\":{},\"ip\":{},\"current\":{},\"input\":[{}],\"output\":[{}],\"steps\":{},\"halt\":{}}}",
            tape.join(","),
            self.head,
            self.ip,
            current,
            input.join(","),
            output.join(","),
            self.steps,
            halt
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Machine {
        let mut m = Machine::new(source);
        m.run(1_000_000);
        m
    }

    #[test]
    fn straight_line_program() {
        let m = run("+++>+++++>--.");
        assert_eq!(m.tape, vec![3, 5, 65534]);
        assert_eq!(m.output, vec![65534]);
        assert_eq!(m.halt, Some(Halt::End));
    }

    #[test]
    fn loops_execute() {
        // Move cell0 (2) into cell1 — the program MAGMA rejects.
        let m = run("++[>+<-]");
        assert_eq!(m.tape, vec![0, 2]);
        assert_eq!(m.halt, Some(Halt::End));
    }

    #[test]
    fn nested_loops_multiply() {
        // 3 * 4 = 12 into cell2.
        let m = run("+++[>++++[>+<-]<-]");
        assert_eq!(m.tape[2], 12);
    }

    #[test]
    fn input_is_consumed_in_order() {
        let mut m = Machine::new(",>,.");
        m.push_input(40);
        m.push_input(2);
        m.run(100);
        assert_eq!(m.output, vec![2]);
        assert_eq!(m.tape, vec![40, 2]);
    }

    #[test]
    fn cells_wrap_at_16_bits() {
        let m = run("-");
        assert_eq!(m.tape[0], 65535);
    }

    #[test]
    fn tape_underflow_halts() {
        let mut m = Machine::new("<");
        assert!(m.step());
        assert_eq!(m.halt, Some(Halt::TapeUnderflow));
    }

    #[test]
    fn unbalanced_brackets_are_a_bad_program() {
        let m = Machine::new("++[>+");
        assert_eq!(m.halt, Some(Halt::BadProgram));
        let m = Machine::new("]");
        assert_eq!(m.halt, Some(Halt::BadProgram));
    }

    #[test]
    fn step_limit_stops_runaway_loops() {
        let mut m = Machine::new("+[]");
        m.run(1000);
        assert_eq!(m.halt, Some(Halt::StepLimit));
        assert_eq!(m.steps, 1000);
    }

    #[test]
    fn stepping_is_deterministic_and_ip_visible() {
        let mut m = Machine::new("+>+");
        assert_eq!(m.current(), Some('+'));
        m.step();
        assert_eq!(m.current(), Some('>'));
        assert_eq!(m.tape, vec![1]);
        m.step();
        assert_eq!(m.head, 1);
        assert_eq!(m.tape, vec![1, 0]);
    }

    #[test]
    fn json_state_shape() {
        let mut m = Machine::new("+.");
        m.run(10);
        let j = m.to_json();
        assert!(j.contains("\"tape\":[1]"), "{j}");
        assert!(j.contains("\"output\":[1]"), "{j}");
        assert!(j.contains("\"halt\":\"end\""), "{j}");
        assert!(j.contains("\"steps\":2"), "{j}");
    }
}
