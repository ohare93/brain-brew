//! Pure domain model and behavior for Brain Brew.
//!
//! This crate intentionally contains no file formats, filesystem access, terminal UI,
//! or command-line concerns. It owns the CanonicalDeck domain model, validation,
//! composition, and semantic diffing as they are introduced through TDD.

mod compose;
mod messages;
mod model;
mod translation;
mod validate;

pub use model::*;
pub use translation::glob_matches;

#[cfg(test)]
mod tests;
