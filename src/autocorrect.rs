// TODO
// Need to get the wayland input method protocol working.
// Then get suggestions here, and use n-grams in current text
// to choose the best suggestion.

// use skyspell_core::{Dictionary, SystemDictionary};

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
