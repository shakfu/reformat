//! Plugin system for reformat transformers
//!
//! This crate provides the foundation for loading and managing
//! custom transformation plugins.

/// Plugin API placeholder for future implementation
pub struct PluginManager {
    // Will be implemented in future versions
}

impl PluginManager {
    /// Creates a new plugin manager
    pub fn new() -> Self {
        PluginManager {}
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
