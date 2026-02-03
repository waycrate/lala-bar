use fluent_bundle::FluentResource;
use intl_memoizer::concurrent::IntlLangMemoizer;
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "assets/locales"]
struct Asset;

// static instance of localizer for global use
pub static LOCALIZER: LazyLock<Localizer> = LazyLock::new(Localizer::new);

pub struct Localizer {
    bundle: fluent_bundle::bundle::FluentBundle<FluentResource, IntlLangMemoizer>,
}

impl Localizer {
    pub fn new() -> Self {
        let mut bundle =
            fluent_bundle::bundle::FluentBundle::new_concurrent(vec!["en-US".parse().unwrap()]);

        let path = "en-US/main.ftl";
        let file = Asset::get(path).expect("Failed to load English locale");
        let source = std::str::from_utf8(file.data.as_ref()).unwrap().to_string();
        let resource = FluentResource::try_new(source).expect("Failed to parse ftl");

        bundle.add_resource(resource).unwrap();
        Self { bundle }
    }

    //for static strings without args
    //returns key as a string if no match found
    pub fn tr(&self, key: &str) -> String {
        // using the key to fetch the msg from bundle
        match self.bundle.get_message(key) {
            Some(m) => {
                if let Some(value) = m.value() {
                    self.bundle
                        .format_pattern(value, None, &mut vec![])
                        .to_string()
                } else {
                    key.to_string()
                }
            }
            None => key.to_string(),
        }
    }

    //for strings with args (For ex: battery percentage)
    pub fn tr_with_args(&self, key: &str, args: &fluent_bundle::FluentArgs) -> String {
        match self.bundle.get_message(key) {
            Some(m) => {
                if let Some(value) = m.value() {
                    let mut errors = vec![];
                    self.bundle
                        .format_pattern(value, Some(args), &mut errors)
                        .to_string()
                } else {
                    key.to_string()
                }
            }
            None => key.to_string(),
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
}
