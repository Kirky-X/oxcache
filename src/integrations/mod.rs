//! oxcache integration modules with external frameworks.
//!
//! Integrations are feature-gated so the core cache library stays
//! dependency-free when integrations are not needed.

#[cfg(feature = "kit")]
pub mod kit;
