//! Versioned browser assets and bundled resources, resolved once per publication.
use crate::{
    fingerprint::digest,
    locale::Locale,
    storage::{Publication, read_override, read_text_override},
};
use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::{borrow::Cow, collections::BTreeSet, path::Path};

include!("../../resources/font-subsets.rs");

struct Asset {
    path: &'static str,
    bytes: Cow<'static, [u8]>,
}

impl Asset {
    fn load(root: &Path, path: &'static str, fallback: &'static [u8]) -> Result<Self> {
        Ok(Self {
            path,
            bytes: read_override(root, path, fallback)?.into(),
        })
    }

    fn generated(path: &'static str, text: String) -> Self {
        Self {
            path,
            bytes: text.into_bytes().into(),
        }
    }

    fn version(&self) -> String {
        digest(&self.bytes)[..12].to_owned()
    }
}

#[derive(Serialize)]
pub(super) struct AssetVersions {
    site_css: String,
    site_js: String,
    references_js: String,
    typst_css: String,
    i18n_js: String,
    math_js: String,
    math_converter: String,
    fonts_css: String,
    pub playground: String,
}

pub(super) struct Assets {
    files: Vec<Asset>,
    pub versions: AssetVersions,
}

impl Assets {
    pub fn load<'a>(
        root: &Path,
        styles: impl Iterator<Item = &'a str>,
        locale: &Locale,
    ) -> Result<Self> {
        let i18n_js = Asset::load(
            root,
            "assets/i18n.js",
            include_bytes!("../../../../assets/i18n.js"),
        )?;
        let fonts_css = Asset::load(
            root,
            "assets/fonts/zhuque/subsets.css",
            include_bytes!("../../../../assets/fonts/zhuque/subsets.css"),
        )?;
        let site_css = Asset::load(
            root,
            "assets/site.css",
            include_bytes!("../../../../assets/site.css"),
        )?;
        let site_js = Asset::load(
            root,
            "assets/site.js",
            include_bytes!("../../../../assets/site.js"),
        )?;
        let references_js = Asset::load(
            root,
            "assets/references.js",
            include_bytes!("../../../../assets/references.js"),
        )?;
        let typst_css = Asset::generated(
            "assets/typst.css",
            styles
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let playground_js = Asset::load(
            root,
            "assets/vendor/playground.js",
            include_bytes!("../../../../assets/vendor/playground.js"),
        )?;
        let math_js = Asset::load(
            root,
            "assets/vendor/math.js",
            include_bytes!("../../../../assets/vendor/math.js"),
        )?;
        let math_converter = Asset::load(
            root,
            "assets/vendor/math-latex.js",
            include_bytes!("../../../../assets/vendor/math-latex.js"),
        )?;
        let template = read_text_override(
            root,
            "assets/playground.html",
            include_str!("../../../../assets/playground.html"),
        )?;
        let mut values = vec![
            ("__PLAYGROUND_VERSION__".to_owned(), playground_js.version()),
            ("__I18N_VERSION__".to_owned(), i18n_js.version()),
            ("__LANGUAGE__".to_owned(), locale.language().to_owned()),
            ("__LOCALE_JSON__".to_owned(), locale.script_data()?),
            (
                "__MONO_FONT_DATA__".to_owned(),
                STANDARD.encode(include_bytes!("../../../../assets/fonts/mono.woff2")),
            ),
        ];
        for key in [
            "playground_title",
            "run",
            "stop",
            "editor_loading",
            "output",
        ] {
            values.push((
                format!("__{}__", key.to_ascii_uppercase()),
                html_escape::encode_double_quoted_attribute(&locale.text(key)?).into_owned(),
            ));
        }
        let playground = Asset::generated("assets/playground.html", substitute(&template, &values));
        let versions = AssetVersions {
            site_css: site_css.version(),
            site_js: site_js.version(),
            references_js: references_js.version(),
            typst_css: typst_css.version(),
            i18n_js: i18n_js.version(),
            math_js: math_js.version(),
            math_converter: math_converter.version(),
            fonts_css: fonts_css.version(),
            playground: playground.version(),
        };
        let mut files = vec![
            site_css,
            site_js,
            references_js,
            typst_css,
            playground_js,
            playground,
            i18n_js,
            math_js,
            math_converter,
            fonts_css,
        ];
        files.extend(
            BUNDLED
                .iter()
                .chain(FONT_SUBSETS)
                .map(|&(path, bytes)| Asset {
                    path,
                    bytes: Cow::Borrowed(bytes),
                }),
        );
        Ok(Self { files, versions })
    }

    pub fn write(&self, output: &Publication) -> Result<()> {
        self.files
            .iter()
            .try_for_each(|asset| output.write(asset.path, &asset.bytes))
    }
}

// Replace template tokens only in the original source, never inside translations.
fn substitute(source: &str, values: &[(String, String)]) -> String {
    let mut remaining = source;
    let mut output = String::new();
    while let Some((offset, key, value)) = values
        .iter()
        .filter_map(|(key, value)| remaining.find(key).map(|offset| (offset, key, value)))
        .min_by_key(|(offset, _, _)| *offset)
    {
        output.push_str(&remaining[..offset]);
        output.push_str(value);
        remaining = &remaining[offset + key.len()..];
    }
    output.push_str(remaining);
    output
}

const BUNDLED: &[(&str, &[u8])] = &[
    (
        "assets/vendor/licenses.txt",
        include_bytes!("../../../../assets/vendor/licenses.txt"),
    ),
    (
        "assets/fonts/zhuque.woff2",
        include_bytes!("../../../../assets/fonts/zhuque.woff2"),
    ),
    (
        "assets/fonts/garamond.woff2",
        include_bytes!("../../../../assets/fonts/garamond.woff2"),
    ),
    (
        "assets/fonts/garamond-italic.woff2",
        include_bytes!("../../../../assets/fonts/garamond-italic.woff2"),
    ),
    (
        "assets/fonts/mono.woff2",
        include_bytes!("../../../../assets/fonts/mono.woff2"),
    ),
    (
        "assets/fonts/zhuque-OFL.txt",
        include_bytes!("../../../../assets/fonts/zhuque-OFL.txt"),
    ),
    (
        "assets/fonts/garamond-OFL.txt",
        include_bytes!("../../../../assets/fonts/garamond-OFL.txt"),
    ),
    (
        "assets/fonts/mono-OFL.txt",
        include_bytes!("../../../../assets/fonts/mono-OFL.txt"),
    ),
];
