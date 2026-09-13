use std::{fmt, io, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionLocation {
    Root,
    Item { index: usize, id: Option<String> },
}

#[derive(Debug)]
pub enum LoadError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json(serde_json::Error),
    Admission {
        location: AdmissionLocation,
        field: String,
        reason: &'static str,
    },
}

impl AdmissionLocation {
    pub(crate) fn error(&self, field: impl Into<String>, reason: &'static str) -> LoadError {
        LoadError::Admission {
            location: self.clone(),
            field: field.into(),
            reason,
        }
    }
}

impl fmt::Display for AdmissionLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => write!(f, "root"),
            Self::Item { index, id } => {
                write!(f, "item[{index}]")?;
                if let Some(id) = id {
                    write!(f, " ({id:?})")?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "read {}: {source}", path.display()),
            Self::Json(source) => write!(f, "JSON syntax: {source}"),
            Self::Admission {
                location,
                field,
                reason,
            } => {
                write!(f, "{location}, field {field}: {reason}")
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(source) => Some(source),
            Self::Admission { .. } => None,
        }
    }
}
