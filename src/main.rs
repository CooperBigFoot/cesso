use anyhow::Result;
use tracing::info;

use cesso_uci::UciEngine;

fn main() -> Result<()> {
    // UCI protocol uses stdout; tracing defaults to stderr
    tracing_subscriber::fmt::init();
    info!("cesso starting");

    let engine = UciEngine::new();
    engine.run()?;

    Ok(())
}
