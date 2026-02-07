pub use crate::fl;
use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "assets/locales"]
struct Asset;

// static instance of localizer for global use
pub static LOCALIZER: LazyLock<Localizer> = LazyLock::new(Localizer::new);

pub struct Localizer {
    pub loader: FluentLanguageLoader,
}

impl Localizer {
    pub fn new() -> Self {
        let loader: FluentLanguageLoader = fluent_language_loader!();

        let requested_languages = DesktopLanguageRequester::requested_languages();

        if let Err(e) = i18n_embed::select(&loader, &Asset, &requested_languages) {
            tracing::warn!(
                "Localized strings not found for system lang, falling back to English: {}",
                e
            );
        }

        Self { loader }
    }
}

pub fn localize_with_args(id: &str, args: &fluent_bundle::FluentArgs) -> String {
    let mut map = HashMap::new();
    for (k, v) in args.iter() {
        map.insert(k.to_string(), v.clone());
    }
    LOCALIZER.loader.get_args(id, map)
}

#[macro_export]
macro_rules! fl {
    // for static strings
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::localization::LOCALIZER.loader, $message_id)
    }};

    //for cases with single arg
    ($message_id:literal, $key:ident = $value:expr $(, $rest:tt)*) => {{
        i18n_embed_fl::fl!($crate::localization::LOCALIZER.loader, $message_id, $key = $value $(, $rest)*)
    }};

    //for complex edge cases when multiple args are needed
    ($message_id:literal, $args:expr) => {{
        $crate::localization::localize_with_args($message_id, &$args)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_translation() {
        // testing a simple static string
        let result = fl!("app-name");
        assert_eq!(result, "Lala Bar", "Should translate app-name correctly");
    }

    #[test]
    fn test_args_translation() {
        // testing a string with arguments
        let result = fl!("balance-left", percent = 50);

        // test was failing multiple times
        // realised it was due to BiDi markers being included in the output
        let clean_result = result.replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(
            clean_result, "Left: 50%",
            "Should translate balance-left correctly after stripping BiDi markers"
        );
    }

    #[test]
    fn test_language_consistency() {
        // this test uses en-US as a base reference
        // checks if all the keys are present in all the other langs
        use fluent_bundle::FluentResource;
        use fluent_syntax::ast::Entry;
        use std::collections::HashSet;

        // helper to extract keys from a file path
        let get_keys = |path: &str| -> HashSet<String> {
            let file = Asset::get(path).unwrap_or_else(|| panic!("File {} not found", path));
            let source = std::str::from_utf8(file.data.as_ref()).unwrap();
            let res = FluentResource::try_new(source.to_string())
                .unwrap_or_else(|_| panic!("Failed to parse {}", path));

            res.entries()
                .filter_map(|entry| {
                    if let Entry::Message(msg) = entry {
                        Some(msg.id.name.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        };

        //en-US as a base reference
        let base_path = "en-US/lala-bar.ftl";
        let expected_keys = get_keys(base_path);

        //iteration over all other langs
        let mut missing_stuff = false;

        Asset::iter()
            .filter(|path| path.ends_with(".ftl") && path.as_ref() != base_path)
            .for_each(|path| {
                let found_keys = get_keys(path.as_ref());
                let missing: Vec<_> = expected_keys.difference(&found_keys).collect();

                if !missing.is_empty() {
                    missing_stuff = true;
                    eprintln!("FAIL: {} is missing keys: {:?}", path, missing);
                }
            });

        assert!(
            !missing_stuff,
            "Localization consistency check failed. See stderr."
        );
    }
}
