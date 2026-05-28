mod classification;
mod evaluation;
mod messages;
mod mode;
mod scoring;

pub use evaluation::evaluate;
pub use messages::finding_location;
pub use mode::CheckMode;
pub use scoring::{STRUCTURAL_MATCH_THRESHOLD, score_match};

#[cfg(test)]
mod tests;
