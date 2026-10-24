//! hamesh-diag: bounded structured diagnostic recording.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// One diagnostic record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub seq: u64,
    pub kind: String,
    pub detail: String,
    pub app_revision: u64,
    pub frame_ref: Option<String>,
}

/// Bounded ring. Never grows without limit.
#[derive(Debug)]
pub struct Trace {
    capacity: usize,
    records: VecDeque<Record>,
    next_seq: u64,
}

impl Trace {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(16),
            records: VecDeque::new(),
            next_seq: 1,
        }
    }

    pub fn record(
        &mut self,
        kind: &str,
        detail: &str,
        app_revision: u64,
        frame_ref: Option<String>,
    ) {
        if self.records.len() >= self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(Record {
            seq: self.next_seq,
            kind: kind.to_string(),
            detail: detail.to_string(),
            app_revision,
            frame_ref,
        });
        self.next_seq += 1;
    }

    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.records.iter().collect::<Vec<_>>())
            .unwrap_or_else(|_| "[]".to_string())
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded() {
        let mut t = Trace::new(16);
        for i in 0..30 {
            t.record("test", &format!("r{i}"), i, None);
        }
        assert_eq!(t.len(), 16);
        assert!(t.export_json().contains("r29"));
    }
}
