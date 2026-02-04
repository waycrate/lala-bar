use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "assets/locales"]
struct Asset;

// static instance of localizer for global use
pub static LOCALIZER: LazyLock<Localizer> = LazyLock::new(Localizer::new);

pub struct Localizer {
    loader: FluentLanguageLoader,
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

    //for static strings without args
    //returns key as a string if no match found
    pub fn tr(&self, key: &str) -> String {
        if self.loader.has(key) {
            self.loader.get(key)
        } else {
            key.to_string()
        }
    }

    //for strings with args (For ex: battery percentage)
    pub fn tr_with_args(&self, key: &str, args: &fluent_bundle::FluentArgs) -> String {
        if self.loader.has(key) {
            let mut map = std::collections::HashMap::new();
            for (k, v) in args.iter() {
                map.insert(k.to_string(), v.clone());
            }
            self.loader.get_args(key, map)
        } else {
            key.to_string()
        }
    }
}

pub fn fl(key: &str) -> String {
    LOCALIZER.tr(key)
}

pub fn fl_args(key: &str, args: fluent_bundle::FluentArgs) -> String {
    LOCALIZER.tr_with_args(key, &args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_bundle::FluentArgs;

    #[test]
    fn test_simple_translation() {
        // testing a simple static string
        let result = fl("app-name");
        assert_eq!(result, "Lala Bar", "Should translate app-name correctly");
    }

    #[test]
    fn test_args_translation() {
        // testing a string with arguments
        let mut args = FluentArgs::new();
        args.set("percent", 50);
        let result = fl_args("balance-left", args);

        // test was failing multiple times
        // realised it was due to BiDi markers being included in the output
        let clean_result = result.replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(
            clean_result, "Left: 50%",
            "Should translate balance-left correctly after stripping BiDi markers"
        );
    }

    #[test]
    fn test_missing_key() {
        // checking fallback
        let result = fl("non-existent-key");
        assert_eq!(result, "non-existent-key", "Should return key if missing");
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
