mod app;
mod kbd;
mod ptr;
mod ui;

use anyhow::Result;
use app::App;
use skyspell_core::{Dictionary, SystemDictionary};
use tracing_subscriber::EnvFilter;

// fn main() -> Result<()> {
//     SystemDictionary::init();
//     let lang = "en_US";
//     let word = "helo";
//     let spell_client = SystemDictionary::new(lang)?;
//     let ok = spell_client.check(word)?;
//     if ok {
//         println!("No error")
//     } else {
//         println!("Unknown word");
//         let suggestions = spell_client.suggest(word)?;
//         println!("Suggestions: {:?}", suggestions)
//     }
//     Ok(())
// }

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
