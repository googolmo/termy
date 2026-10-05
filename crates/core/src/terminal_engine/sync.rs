//! Synchronized output stages bytes, never copies the screen or scrollback.
//!
//! A second streaming parser recognizes mode markers without dispatching screen
//! mutations. This keeps fragmented CSI, UTF-8 and string termination semantics
//! identical to normal parsing while holding a strictly bounded byte buffer.

use std::time::{Duration, Instant};

use super::parser::{Handler, Param, Parser};

pub(super) const MAX_SYNC_BYTES: usize = 2 * 1024 * 1024;
pub(super) const SYNC_TIMEOUT: Duration = Duration::from_millis(150);

#[derive(Default)]
pub(super) struct SynchronizedUpdate {
    scanner: Parser,
    bytes: Vec<u8>,
    deadline: Option<Instant>,
}

pub(super) struct Buffered {
    pub(super) consumed: usize,
    pub(super) commit: bool,
}

impl SynchronizedUpdate {
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub(super) fn begin(&mut self, now: Instant) {
        self.bytes.clear();
        self.scanner.reset();
        self.deadline = Some(now + SYNC_TIMEOUT);
    }

    pub(super) fn push(&mut self, bytes: &[u8], now: Instant) -> Buffered {
        debug_assert!(self.deadline.is_some());
        let admitted = bytes.len().min(MAX_SYNC_BYTES - self.bytes.len());
        let mut marker = Marker::default();
        let consumed = self.scanner.advance(&mut marker, &bytes[..admitted]);
        let required = self.bytes.len() + consumed;
        if required > self.bytes.capacity() {
            let capacity = required.next_power_of_two().min(MAX_SYNC_BYTES);
            self.bytes.reserve_exact(capacity - self.bytes.len());
        }
        self.bytes.extend_from_slice(&bytes[..consumed]);
        if marker.refreshed {
            self.deadline = Some(now + SYNC_TIMEOUT);
        }
        Buffered {
            consumed,
            commit: marker.ended || self.bytes.len() == MAX_SYNC_BYTES,
        }
    }

    pub(super) fn take_buffer(&mut self) -> Option<Vec<u8>> {
        self.deadline.take()?;
        self.scanner.reset();
        Some(std::mem::take(&mut self.bytes))
    }

    pub(super) fn recycle_buffer(&mut self, mut bytes: Vec<u8>) {
        bytes.clear();
        self.bytes = bytes;
    }
}

#[derive(Default)]
struct Marker {
    refreshed: bool,
    ended: bool,
}

impl Handler for Marker {
    fn print(&mut self, _character: char) {}
    fn print_ascii(&mut self, _bytes: &[u8]) {}
    fn execute(&mut self, _byte: u8) {}
    fn escape(&mut self, intermediates: &[u8], final_byte: u8) {
        // A full terminal reset also resets synchronized-update mode.
        if intermediates.is_empty() && final_byte == b'c' {
            self.ended = true;
        }
    }
    fn osc(&mut self, _bytes: &[u8]) {}
    fn dcs(&mut self, _bytes: &[u8]) {}
    fn apc(&mut self, _bytes: &[u8]) {}

    fn csi(&mut self, params: &[Param], private: Option<u8>, intermediates: &[u8], final_byte: u8) {
        if private != Some(b'?') || !intermediates.is_empty() {
            return;
        }
        if !params
            .iter()
            .any(|param| param.value() == Some(2026) && param.subparams().is_empty())
        {
            return;
        }
        match final_byte {
            b'h' => self.refreshed = true,
            b'l' => self.ended = true,
            _ => {}
        }
    }

    fn pause_requested(&self) -> bool {
        self.ended
    }
}

#[cfg(test)]
#[path = "sync/tests.rs"]
mod tests;
