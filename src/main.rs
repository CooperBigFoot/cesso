use anyhow::Result;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("cesso starting");
    Ok(())
}
