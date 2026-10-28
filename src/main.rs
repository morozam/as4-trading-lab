#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let config_path = match (args.next(), args.next()) {
        (Some(flag), Some(path)) if flag == "--config" => path,
        _ => {
            eprintln!("Usage: nomination-lab --config path/to/config.json");
            std::process::exit(2)
        }
    };
    let config = std::fs::read_to_string(&config_path)
        .map_err(|e| e.to_string())
        .and_then(|raw| {
            serde_json::from_str::<nomination_lab::app::Config>(&raw).map_err(|e| e.to_string())
        });
    match config {
        Ok(config) => {
            if let Err(e) = nomination_lab::web::serve(config).await {
                eprintln!("{e}");
                std::process::exit(1)
            }
        }
        Err(e) => {
            eprintln!("Cannot read {config_path}: {e}");
            std::process::exit(2)
        }
    }
}
