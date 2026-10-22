//! The `scix` binary. All logic lives in `scix_client::cli`.

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() {
    if let Err(e) = scix_client::cli::run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("This binary requires the 'cli' feature. Build with: cargo build --features cli");
    std::process::exit(1);
}
