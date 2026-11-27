//! Studio-only drum pattern tails. The native first sixteen steps and RBS bytes stay intact.

use serde::{Deserialize, Serialize};

pub const PATTERNS: usize = 32;
pub const EXTRA_STEPS: usize = 48;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pattern {
    /// Zero follows the native pattern length; only 32 and 64 extend it.
    pub length: u8,
    /// Rows for steps 17–64, in the native twelve-column instrument order.
    pub steps: Vec<[u8; 12]>,
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            length: 0,
            steps: vec![[0; 12]; EXTRA_STEPS],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bank {
    pub patterns: Vec<Pattern>,
}

impl Default for Bank {
    fn default() -> Self {
        Self {
            patterns: vec![Pattern::default(); PATTERNS],
        }
    }
}

impl Bank {
    /// Normalize untrusted project data on the loading/UI thread, never in the callback.
    pub fn sanitize(&mut self) {
        self.patterns.resize_with(PATTERNS, Pattern::default);
        self.patterns.truncate(PATTERNS);
        for pattern in &mut self.patterns {
            if !matches!(pattern.length, 32 | 64) {
                pattern.length = 0;
            }
            pattern.steps.resize(EXTRA_STEPS, [0; 12]);
            pattern.steps.truncate(EXTRA_STEPS);
            for row in &mut pattern.steps {
                for value in row {
                    *value = (*value).min(4);
                }
            }
        }
    }

    pub fn length(&self, pattern: u8, native: u8) -> u8 {
        self.patterns
            .get(usize::from(pattern))
            .map_or(native, |p| match p.length {
                32 | 64 => p.length,
                _ => native,
            })
    }

    pub fn row(&self, pattern: u8, step: usize) -> [u8; 12] {
        step.checked_sub(16)
            .filter(|&i| i < EXTRA_STEPS)
            .and_then(|i| self.patterns.get(usize::from(pattern))?.steps.get(i))
            .copied()
            .unwrap_or([0; 12])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_length_and_hidden_tail_survive_length_changes() {
        let mut bank = Bank::default();
        bank.patterns[0].steps[47][1] = 4;
        assert_eq!(bank.length(0, 7), 7);
        bank.patterns[0].length = 64;
        assert_eq!(bank.length(0, 7), 64);
        assert_eq!(bank.row(0, 63)[1], 4);
        bank.patterns[0].length = 32;
        bank.sanitize();
        assert_eq!(bank.row(0, 63)[1], 4);
        bank.patterns[0].length = 0;
        assert_eq!(bank.length(0, 16), 16);
        assert_eq!(bank.row(0, 15), [0; 12]);
        assert_eq!(bank.row(255, 63), [0; 12]);
    }

    #[test]
    fn malformed_banks_are_safe_before_and_after_ui_sanitizing() {
        let mut bank = Bank { patterns: vec![] };
        assert_eq!(bank.length(0, 16), 16);
        assert_eq!(bank.row(0, 16), [0; 12]);
        bank.sanitize();
        assert_eq!(bank.patterns.len(), PATTERNS);
        assert!(bank.patterns.iter().all(|p| p.steps.len() == EXTRA_STEPS));
    }
}
