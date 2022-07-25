use anyhow::Result;
use catp::{catp, CatpArgs};
use clap::Parser;

fn main() -> Result<()> {
    let args = CatpArgs::parse();

    catp(args, &mut std::io::stdout(), &mut std::io::stderr())
}
