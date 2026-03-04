use clap::Parser;
use seqx::cmd::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Stats(args) => seqx::cmd::stats::run(args),
        Commands::Convert(args) => seqx::cmd::convert::run(args),
        Commands::Filter(args) => seqx::cmd::filter::run(args),
        Commands::Extract(args) => seqx::cmd::extract::run(args),
        Commands::Search(args) => seqx::cmd::search::run(args),
        Commands::Modify(args) => seqx::cmd::modify::run(args),
        Commands::Sample(args) => seqx::cmd::sample::run(args),
        Commands::Sort(args) => seqx::cmd::sort::run(args),
        Commands::Dedup(args) => seqx::cmd::dedup::run(args),
        Commands::Merge(args) => seqx::cmd::merge::run(args),
        Commands::Split(args) => seqx::cmd::split::run(args),
    }
}
