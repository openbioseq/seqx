use clap::{Parser, Subcommand};

pub mod convert;
pub mod dedup;
pub mod extract;
pub mod filter;
pub mod merge;
pub mod modify;
pub mod sample;
pub mod search;
pub mod sort;
pub mod split;
pub mod stats;

#[derive(Parser)]
#[command(name = "seqx")]
#[command(about = "Agent-friendly sequence processing tool")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Statistics of sequences
    Stats(stats::Args),
    /// Convert sequence format
    Convert(convert::Args),
    /// Filter sequences
    Filter(filter::Args),
    /// Extract sequences or regions
    Extract(extract::Args),
    /// Search patterns in sequences
    Search(search::Args),
    /// Modify sequences
    Modify(modify::Args),
    /// Sample random sequences
    Sample(sample::Args),
    /// Sort sequences
    Sort(sort::Args),
    /// Remove duplicate sequences
    Dedup(dedup::Args),
    /// Merge multiple files
    Merge(merge::Args),
    /// Split file into multiple files
    Split(split::Args),
}
