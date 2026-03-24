use anyhow::Result;
use clap::Parser;
use squid::cli::Cli;

fn main() -> Result<()> {
    squid::runtime::run(Cli::parse().path)
}
