//! `JsonCodec`: thin wrapper over `serde_json` that adds stable pretty
//! printing and uniform error reporting.

use crate::error::JsonError;
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

/// Round-trip a `serde::Serialize` value through bytes.
///
/// Implementations are intentionally simple — there is no caching, no
/// streaming, no schema awareness. Higher-level crates compose these calls.
pub trait JsonCodec: Send + Sync {
    fn from_slice<T>(&self, bytes: &[u8]) -> Result<T, JsonError>
    where
        T: DeserializeOwned;

    fn from_str<T>(&self, s: &str) -> Result<T, JsonError>
    where
        T: DeserializeOwned,
    {
        self.from_slice(s.as_bytes())
    }

    fn to_pretty_vec<T>(&self, value: &T) -> Result<Vec<u8>, JsonError>
    where
        T: Serialize + ?Sized;

    fn to_compact_vec<T>(&self, value: &T) -> Result<Vec<u8>, JsonError>
    where
        T: Serialize + ?Sized;

    fn read_file<T>(&self, path: &Path) -> Result<T, JsonError>
    where
        T: DeserializeOwned;
}

/// Default codec. Pretty output is two-space indented and ends with a
/// trailing newline so files can be safely concatenated.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultJsonCodec;

impl JsonCodec for DefaultJsonCodec {
    fn from_slice<T>(&self, bytes: &[u8]) -> Result<T, JsonError>
    where
        T: DeserializeOwned,
    {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|source| JsonError::Syntax {
                path: "<bytes>".into(),
                source,
            })?;
        serde_json::from_value(value).map_err(|source| JsonError::Deserialise {
            path: "<bytes>".into(),
            source,
        })
    }

    fn to_pretty_vec<T>(&self, value: &T) -> Result<Vec<u8>, JsonError>
    where
        T: Serialize + ?Sized,
    {
        let mut buf =
            serde_json::to_vec_pretty(value).map_err(|source| JsonError::Deserialise {
                path: "<value>".into(),
                source,
            })?;
        buf.push(b'\n');
        Ok(buf)
    }

    fn to_compact_vec<T>(&self, value: &T) -> Result<Vec<u8>, JsonError>
    where
        T: Serialize + ?Sized,
    {
        serde_json::to_vec(value).map_err(|source| JsonError::Deserialise {
            path: "<value>".into(),
            source,
        })
    }

    fn read_file<T>(&self, path: &Path) -> Result<T, JsonError>
    where
        T: DeserializeOwned,
    {
        let bytes = std::fs::read(path).map_err(|source| JsonError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let parsed: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|source| JsonError::Syntax {
                path: path.to_path_buf(),
                source,
            })?;
        serde_json::from_value(parsed).map_err(|source| JsonError::Deserialise {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        count: u32,
    }

    #[test]
    fn round_trips_through_pretty_and_compact() {
        let codec = DefaultJsonCodec;
        let sample = Sample {
            name: "demo".into(),
            count: 3,
        };
        let pretty = codec.to_pretty_vec(&sample).unwrap();
        let pretty_str = std::str::from_utf8(&pretty).unwrap();
        assert!(pretty_str.contains('\n'));
        let compact = codec.to_compact_vec(&sample).unwrap();
        let parsed: Sample = codec.from_slice(&compact).unwrap();
        assert_eq!(parsed, sample);
    }

    #[test]
    fn from_str_propagates_syntax_error() {
        let codec = DefaultJsonCodec;
        let err = codec.from_str::<Sample>("not-json").unwrap_err();
        matches!(err, JsonError::Syntax { .. });
    }

    #[test]
    fn read_file_errors_are_labeled_with_path() {
        let codec = DefaultJsonCodec;
        let err = codec
            .read_file::<Sample>(Path::new("definitely-not-here.json"))
            .unwrap_err();
        match err {
            JsonError::Io { path, .. } => {
                assert_eq!(path, Path::new("definitely-not-here.json"));
            }
            other => panic!("expected Io error, got {other:?}"),
        }
    }
}
