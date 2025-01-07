mod app;
mod autocorrect;
mod keyboard;
mod layout;
mod pointer;
mod session;
mod ui;

use app::App;
use tracing_subscriber::EnvFilter;

fn main() {
    let filter = "none,vkbd=debug";
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .without_time()
        .compact()
        .init();

    let app = App::new();
    app.run();
}
