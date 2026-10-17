mod bot;

use std::env;
use std::process::ExitCode;

use bot::Bot;
use tokio::signal::unix::{SignalKind, signal};
use tracing::{error, info};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let mut bot = Bot::new(&token).await;
    let shard_manager = bot.shard_manager();

    tokio::select! {
        result = bot.start() => {
            // The client only stops on its own after a fatal error (e.g. a rejected token or
            // intents). Exit non-zero so the supervisor restarts us instead of leaving a
            // process that runs but is offline.
            match result {
                Err(why) => error!("Client ended: {:?}", why),
                Ok(()) => error!("Client ended unexpectedly"),
            }
            return ExitCode::FAILURE;
        }
        () = shutdown_signal() => {}
    }

    info!("Received shutdown signal, shutting down.");
    shard_manager.shutdown_all().await;

    ExitCode::SUCCESS
}

/// Waits for SIGINT (Ctrl-C, Fly's default) or SIGTERM (`docker stop`).
async fn shutdown_signal() {
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to listen for SIGTERM");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
    }
}
