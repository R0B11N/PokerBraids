mod cli;
mod server;

use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    // Check for --server flag (debug slop)
    if args.iter().any(|arg| arg == "--server") {
        // Start the web server
        let reset_on_fold = args.iter().any(|arg| arg == "--reset-on-fold");
        server::start_server(reset_on_fold).await?;
    } else {
        // Run CLI mode
        cli::run_cli()?;
    }
    
    Ok(())
}
