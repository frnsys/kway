mod app;
mod kbd;
mod ptr;
mod ui;

use app::App;

fn main() {
    let app = App::new();
    app.run();
}
