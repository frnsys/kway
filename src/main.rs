mod app;
mod autocorrect;
mod keyboard;
mod layout;
mod pointer;
mod session;
mod ui;

use std::path::PathBuf;

use app::App;
use bpaf::Bpaf;
use layout::Layout;
use tracing_subscriber::EnvFilter;

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
/// vkbd, a virtual keyboard.
struct Args {
    /// Path to layout file
    layout: Option<PathBuf>,
}

fn main() {
    let opts = args().run();
    let layout = opts.layout.map_or_else(Layout::default, Layout::from_path);

    let filter = "none,vkbd=debug";
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .without_time()
        .compact()
        .init();

    let app = App::new(layout);
    app.run();
}
