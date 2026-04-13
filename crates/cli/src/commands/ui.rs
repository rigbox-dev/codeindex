use anyhow::Result;
use codeindex_core::config::Config;

pub fn run(port: u16, no_open: bool) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);

    if !index_dir.join("index.db").exists() {
        anyhow::bail!("No index found. Run `codeindex init && codeindex index` first.");
    }

    println!("Starting codeindex dashboard at http://127.0.0.1:{}", port);

    if !no_open {
        let url = format!("http://127.0.0.1:{}", port);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = webbrowser::open(&url);
        });
    }

    tokio::runtime::Runtime::new()?
        .block_on(codeindex_web::start_server(project_root, port))
}
