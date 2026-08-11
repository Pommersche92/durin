//! UI composition root.
//!
//! This module exposes the high-level split between full-page renderers
//! (`pages`) and shared, reusable widgets (`controls`).
//!
//! The separation keeps each page focused on user workflows while controls
//! remain stateless helpers that can be reused in multiple contexts.

pub mod controls;
pub mod pages;
