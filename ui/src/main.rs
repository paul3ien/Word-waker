//! Word Waker UI — macOS menu bar application.

mod app;
mod config;
mod delegate;
mod error;
mod socket_client;
mod status_item;

use config::UiConfig;
use icrate::Foundation::MainThreadMarker;
use socket_client::{IpcClient, UiEvent};
use tracing_subscriber::{fmt, EnvFilter};

fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| anyhow::anyhow!("L'UI doit être lancée depuis le main thread"))?;

    let config = UiConfig::from_env();
    tracing::info!(socket_path = %config.socket_path, "Démarrage de Word Waker UI");

    let (tx_event, rx_event) = crossbeam_channel::unbounded::<UiEvent>();

    let app = app::UiApp::new(config.clone(), mtm)?;
    let _delegate = delegate::AppDelegate::new()?;

    // Thread client socket
    let socket_config = config;
    std::thread::Builder::new()
        .name("ipc-client".into())
        .spawn(move || {
            let client = IpcClient::new(socket_config);
            client.run(tx_event);
        })?;

    tracing::info!("Thread socket démarré");
    tracing::info!("UI démarrée — applicationDidFinishLaunching");

    app.run(rx_event)?;

    tracing::info!("UI arrêtée — applicationWillTerminate");
    tracing::info!("Word Waker UI terminé");
    Ok(())
}
