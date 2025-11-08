//! Strategy selection logic for choosing optimal hash calculation strategy
//!
//! The selector analyzes file characteristics, available resources, and
//! algorithm requirements to choose the best strategy for each scenario.

use super::{
    HashingContext, HashingStrategy, HybridStrategy, MultipleStrategy, ParallelStrategy,
    SequentialStrategy,
};
use crate::HashAlgorithm;
use std::sync::Arc;

/// Hints that can be provided to influence strategy selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrategyHint {
    /// Prefer memory efficiency over speed
    PreferMemoryEfficiency,
    /// Prefer speed over memory usage
    PreferSpeed,
    /// Prefer simple sequential processing
    PreferSequential,
    /// Prefer parallel processing when possible
    PreferParallel,
    /// Let the selector decide without bias
    #[default]
    Automatic,
}

/// Selector for choosing the optimal hashing strategy
pub struct StrategySelector {
    strategies: Vec<Box<dyn HashingStrategy>>,
    hint: StrategyHint,
}

impl std::fmt::Debug for StrategySelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrategySelector")
            .field("strategies_count", &self.strategies.len())
            .field("hint", &self.hint)
            .finish()
    }
}

impl StrategySelector {
    /// Create a new strategy selector with default strategies
    pub fn new() -> Self {
        Self::with_hint(StrategyHint::Automatic)
    }

    /// Create a selector with a specific hint
    pub fn with_hint(hint: StrategyHint) -> Self {
        let strategies: Vec<Box<dyn HashingStrategy>> = vec![
            Box::new(SequentialStrategy::default()),
            Box::new(MultipleStrategy::with_defaults()),
            Box::new(ParallelStrategy::with_defaults()),
            Box::new(HybridStrategy::with_defaults()),
        ];

        Self { strategies, hint }
    }

    /// Create a selector with custom strategies
    pub fn with_strategies(strategies: Vec<Box<dyn HashingStrategy>>) -> Self {
        Self {
            strategies,
            hint: StrategyHint::Automatic,
        }
    }

    /// Select the best strategy for the given context
    pub fn select(&self, context: &HashingContext) -> Arc<dyn HashingStrategy> {
        // Apply hint-based filtering first
        let candidates: Vec<&Box<dyn HashingStrategy>> = match self.hint {
            StrategyHint::PreferMemoryEfficiency => {
                // Filter to memory-efficient strategies
                self.strategies
                    .iter()
                    .filter(|s| {
                        let mem_req = s.memory_requirements(context.file_size);
                        mem_req.optimal < 100 * 1024 * 1024 // < 100MB optimal
                    })
                    .collect()
            }
            StrategyHint::PreferSpeed => {
                // Prefer parallel strategies for speed
                self.strategies
                    .iter()
                    .filter(|s| matches!(s.name(), "parallel" | "hybrid"))
                    .collect()
            }
            StrategyHint::PreferSequential => {
                // Only sequential strategy
                self.strategies
                    .iter()
                    .filter(|s| s.name() == "sequential")
                    .collect()
            }
            StrategyHint::PreferParallel => {
                // Only parallel strategies
                self.strategies
                    .iter()
                    .filter(|s| matches!(s.name(), "parallel" | "hybrid"))
                    .collect()
            }
            StrategyHint::Automatic => {
                // All strategies are candidates
                self.strategies.iter().collect()
            }
        };

        // If hint filtering left no candidates, use all strategies
        let candidates = if candidates.is_empty() {
            self.strategies.iter().collect()
        } else {
            candidates
        };

        // Find suitable strategies and select the one with highest priority
        let mut best_strategy: Option<&dyn HashingStrategy> = None;
        let mut best_score = 0u32;

        for strategy in candidates {
            if strategy.is_suitable(context) {
                let score = self.calculate_adjusted_score(strategy.as_ref(), context);
                if score > best_score {
                    best_score = score;
                    best_strategy = Some(strategy.as_ref());
                }
            }
        }

        // If no suitable strategy found, use fallback logic
        let selected = best_strategy.unwrap_or_else(|| self.fallback_selection(context));

        // Clone the strategy into an Arc for sharing
        self.clone_strategy_to_arc(selected)
    }

    /// Calculate adjusted score based on context and hints
    fn calculate_adjusted_score(
        &self,
        strategy: &dyn HashingStrategy,
        context: &HashingContext,
    ) -> u32 {
        let mut score = strategy.priority_score(context);

        // Apply hint adjustments
        match self.hint {
            StrategyHint::PreferMemoryEfficiency => {
                let mem_req = strategy.memory_requirements(context.file_size);
                if mem_req.optimal < 50 * 1024 * 1024 {
                    score += 200; // Strong bonus for low memory usage
                }
                // Penalty for parallel strategy when memory efficiency is preferred
                if strategy.name() == "parallel" {
                    score = score.saturating_sub(150);
                }
            }
            StrategyHint::PreferSpeed => {
                if matches!(strategy.name(), "parallel" | "hybrid") {
                    score += 150; // Bonus for parallel strategies
                }
            }
            _ => {}
        }

        // Special case adjustments
        if Self::has_special_requirements(context) {
            score = self.adjust_for_special_cases(strategy, context, score);
        }

        score
    }

    /// Check if the context has special requirements
    fn has_special_requirements(context: &HashingContext) -> bool {
        // ED2K has special chunking requirements
        context.algorithms.contains(&HashAlgorithm::ED2K)
            || context.algorithms.contains(&HashAlgorithm::TTH)
    }

    /// Adjust score for special cases
    fn adjust_for_special_cases(
        &self,
        strategy: &dyn HashingStrategy,
        context: &HashingContext,
        mut score: u32,
    ) -> u32 {
        // ED2K special handling
        if context.algorithms.contains(&HashAlgorithm::ED2K) {
            if context.algorithms.len() > 1 {
                // ED2K with other algorithms - hybrid is best
                if strategy.name() == "hybrid" {
                    score += 200;
                }
            } else {
                // ED2K alone - sequential is fine
                if strategy.name() == "sequential" {
                    score += 100;
                }
            }
        }

        // TTH (Tiger Tree Hash) benefits from parallel processing
        if context.algorithms.contains(&HashAlgorithm::TTH)
            && matches!(strategy.name(), "parallel" | "hybrid")
        {
            score += 50;
        }

        score
    }

    /// Fallback selection when no strategy is suitable
    fn fallback_selection(&self, context: &HashingContext) -> &dyn HashingStrategy {
        // Fallback logic based on basic heuristics
        if context.algorithms.len() == 1 {
            // Single algorithm - use sequential
            self.strategies
                .iter()
                .find(|s| s.name() == "sequential")
                .unwrap_or(&self.strategies[0])
                .as_ref()
        } else if context.file_size < 100 * 1024 * 1024 {
            // Small file with multiple algorithms - use multiple
            self.strategies
                .iter()
                .find(|s| s.name() == "multiple")
                .unwrap_or(&self.strategies[0])
                .as_ref()
        } else {
            // Default to hybrid for large files with multiple algorithms
            self.strategies
                .iter()
                .find(|s| s.name() == "hybrid")
                .unwrap_or(&self.strategies[0])
                .as_ref()
        }
    }

    /// Clone a strategy into an Arc
    fn clone_strategy_to_arc(&self, strategy: &dyn HashingStrategy) -> Arc<dyn HashingStrategy> {
        // Create new instance based on strategy name
        // This is a workaround since we can't clone trait objects directly
        match strategy.name() {
            "sequential" => Arc::new(SequentialStrategy::default()),
            "multiple" => Arc::new(MultipleStrategy::with_defaults()),
            "parallel" => Arc::new(ParallelStrategy::with_defaults()),
            "hybrid" => Arc::new(HybridStrategy::with_defaults()),
            _ => Arc::new(SequentialStrategy::default()), // Fallback
        }
    }
}

impl Default for StrategySelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::Ed2kVariant;
    use crate::hashing::strategies::HashConfig;
    use std::path::PathBuf;

    #[test]
    fn test_selector_creation() {
        let selector = StrategySelector::new();
        assert_eq!(selector.hint, StrategyHint::Automatic);
        assert_eq!(selector.strategies.len(), 4);
    }

    #[test]
    fn test_selector_with_hint() {
        let selector = StrategySelector::with_hint(StrategyHint::PreferSpeed);
        assert_eq!(selector.hint, StrategyHint::PreferSpeed);
    }

    #[test]
    fn test_selection_single_algorithm_small_file() {
        let selector = StrategySelector::new();
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 10 * 1024 * 1024, // 10MB
            algorithms: vec![HashAlgorithm::MD5],
            config: Default::default(),
        };

        let strategy = selector.select(&context);
        assert_eq!(strategy.name(), "sequential");
    }

    #[test]
    fn test_selection_multiple_algorithms_medium_file() {
        let selector = StrategySelector::new();
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 200 * 1024 * 1024, // 200MB
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };

        let strategy = selector.select(&context);
        // Should select multiple or parallel
        assert!(matches!(strategy.name(), "multiple" | "parallel"));
    }

    #[test]
    fn test_selection_ed2k_combo() {
        let selector = StrategySelector::new();
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024, // 1GB
            algorithms: vec![HashAlgorithm::ED2K, HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: HashConfig {
                ed2k_variant: Ed2kVariant::Red,
                ..Default::default()
            },
        };

        let strategy = selector.select(&context);
        // Should strongly prefer hybrid for ED2K combinations
        assert_eq!(strategy.name(), "hybrid");
    }

    #[test]
    fn test_selection_with_memory_hint() {
        let selector = StrategySelector::with_hint(StrategyHint::PreferMemoryEfficiency);
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 500 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };

        let strategy = selector.select(&context);
        // Should NOT select parallel when memory efficiency is preferred
        assert_ne!(
            strategy.name(),
            "parallel",
            "Should not select parallel strategy when memory efficiency is preferred"
        );
    }

    #[test]
    fn test_selection_with_speed_hint() {
        let selector = StrategySelector::with_hint(StrategyHint::PreferSpeed);
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024,
            algorithms: vec![
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::CRC32,
            ],
            config: Default::default(),
        };

        let strategy = selector.select(&context);
        // Should prefer parallel or hybrid
        assert!(matches!(strategy.name(), "parallel" | "hybrid"));
    }

    #[test]
    fn test_fallback_selection() {
        let selector = StrategySelector::new();

        // Create a context that might not match any strategy perfectly
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 0, // Zero-sized file
            algorithms: vec![],
            config: Default::default(),
        };

        // Should still return a strategy (fallback)
        let strategy = selector.select(&context);
        assert!(!strategy.name().is_empty());
    }
}
