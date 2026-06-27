use conflux_daemon::{load_config, run_daemon, DaemonError};

#[tokio::main]
async fn main() {
    let loaded = match load_config() {
        Ok(loaded) => loaded,
        Err(err) => {
            eprintln!("failed to load config: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = run_daemon(loaded).await {
        match err {
            DaemonError::Ipc(protocol_err) => {
                eprintln!("IPC server failed: {protocol_err}");
            }
            other => {
                eprintln!("daemon failed: {other}");
            }
        }
        std::process::exit(1);
    }
}
