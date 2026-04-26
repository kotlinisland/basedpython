/// Minimum Python runtime version the generated output must run on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PythonVersion {
    V310,
    V311,
    V312,
    V313,
    V314,
    V315,
}

impl PythonVersion {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "3.10" => Some(Self::V310),
            "3.11" => Some(Self::V311),
            "3.12" => Some(Self::V312),
            "3.13" => Some(Self::V313),
            "3.14" => Some(Self::V314),
            "3.15" => Some(Self::V315),
            _ => None,
        }
    }
}

impl std::fmt::Display for PythonVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::V310 => "3.10",
            Self::V311 => "3.11",
            Self::V312 => "3.12",
            Self::V313 => "3.13",
            Self::V314 => "3.14",
            Self::V315 => "3.15",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub min_version: PythonVersion,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_version: PythonVersion::V310,
        }
    }
}
