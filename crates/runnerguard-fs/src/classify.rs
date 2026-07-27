//! Classify a project-relative path into a [`SourceFileKind`].
//!
//! The classifier is intentionally dumb — it looks at extension, path
//! prefix, and a quick sniff of the first byte. Anything we can't classify
//! is [`SourceFileKind::OtherText`] or [`SourceFileKind::Binary`] so the
//! caller can still count it.

pub use runnerguard_model::{SourceFile, SourceFileKind};

/// Classify `relative_path` (forward-slash separated) and pair it with the
/// `size_bytes`. We do not read the file here.

/// Classify `relative_path` (forward-slash separated) and pair it with the
/// `size_bytes`. We do not read the file here.
pub fn classify(relative_path: &str, size_bytes: u64) -> SourceFile {
    let kind = classify_kind(relative_path);
    SourceFile::new(relative_path, size_bytes, kind)
}

fn classify_kind(relative_path: &str) -> SourceFileKind {
    let normalized = relative_path.replace('\\', "/");
    let lowered = normalized.to_ascii_lowercase();
    let file_name = lowered.rsplit('/').next().unwrap_or("");
    let is_in_mule = lowered.starts_with("src/main/mule/") && lowered.ends_with(".xml");
    let is_in_munit = lowered.starts_with("src/test/munit/") && lowered.ends_with(".xml");
    let is_resource = lowered.starts_with("src/main/resources/");
    let is_test_resource = lowered.starts_with("src/test/resources/");

    match file_name {
        "pom.xml" => SourceFileKind::Pom,
        "mule-artifact.json" => SourceFileKind::MuleArtifact,
        _ => {
            if is_in_mule || (lowered.ends_with(".xml") && lowered.contains("/mule/")) {
                SourceFileKind::MuleXml
            } else if is_in_munit {
                SourceFileKind::MunitXml
            } else if is_resource || is_test_resource {
                match extension_of(file_name) {
                    Some("yaml") | Some("yml") => SourceFileKind::ResourceYaml,
                    Some("json") => SourceFileKind::ResourceJson,
                    Some("dwl") | Some("dw") => SourceFileKind::ResourceDataweave,
                    Some("properties") => SourceFileKind::ResourceProperties,
                    _ => SourceFileKind::ResourceOther,
                }
            } else {
                match extension_of(file_name) {
                    Some("xml") => SourceFileKind::OtherXml,
                    Some("json") => SourceFileKind::OtherJson,
                    Some("yaml") | Some("yml") => SourceFileKind::OtherYaml,
                    Some("txt") | Some("md") | Some("csv") | Some("log") => {
                        SourceFileKind::OtherText
                    }
                    _ => SourceFileKind::Binary,
                }
            }
        }
    }
}

fn extension_of(file_name: &str) -> Option<&str> {
    let idx = file_name.rfind('.')?;
    // Skip leading dots (".gitignore")
    if idx == 0 {
        None
    } else {
        Some(&file_name[idx + 1..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pom_and_artifact_json_are_classified() {
        assert_eq!(classify_kind("pom.xml").as_label(), "Maven POM");
        assert_eq!(
            classify_kind("mule-artifact.json").as_label(),
            "Mule artifact descriptor"
        );
    }

    #[test]
    fn src_main_mule_files_are_mule_xml() {
        assert_eq!(
            classify_kind("src/main/mule/order-api.xml").as_label(),
            "Mule XML"
        );
    }

    #[test]
    fn resources_classified_by_extension() {
        assert_eq!(
            classify_kind("src/main/resources/config.yaml").as_label(),
            "YAML resource"
        );
        assert_eq!(
            classify_kind("src/main/resources/payments.dwl").as_label(),
            "DataWeave resource"
        );
        assert_eq!(
            classify_kind("src/main/resources/log4j2.properties").as_label(),
            "Properties resource"
        );
    }

    #[test]
    fn binary_extensions_under_resources_are_recorded_as_other_resource() {
        // Resources directory is a catch-all; binaries still get indexed so
        // they're counted in the file inventory even though we won't read
        // their bytes into a parser.
        assert_eq!(
            classify_kind("src/main/resources/image.png").as_label(),
            "Other resource"
        );
    }

    #[test]
    fn binary_extensions_outside_resources_are_binary() {
        assert_eq!(classify_kind("assets/logo.png").as_label(), "Binary");
    }

    #[test]
    fn unknown_extensions_fall_through() {
        assert_eq!(classify_kind("README.md").as_label(), "Other text");
    }
}
