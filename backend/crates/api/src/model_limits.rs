use rapidmaker_geometry::{ConversionLimits, ThreeMfLimits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModelProfile {
    Standard,
    Large,
}

impl ModelProfile {
    pub fn parse(profile: Option<&str>, auth_mode: Option<&str>) -> Result<Self, String> {
        match profile {
            None | Some("standard") => Ok(Self::Standard),
            Some("large") if auth_mode == Some("localhost") => Ok(Self::Large),
            Some("large") => Err("MODEL_PROFILE=large requires AUTH_MODE=localhost".into()),
            _ => Err("MODEL_PROFILE must be standard or large".into()),
        }
    }

    pub fn from_env() -> Result<Self, String> {
        let profile = std::env::var("MODEL_PROFILE")
            .map(Some)
            .or_else(|error| match error {
                std::env::VarError::NotPresent => Ok(None),
                _ => Err("Invalid MODEL_PROFILE encoding".to_string()),
            })?;
        Self::parse(
            profile.as_deref(),
            std::env::var("AUTH_MODE").ok().as_deref(),
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Large => "large",
        }
    }

    pub fn input_bytes(self) -> usize {
        (match self {
            Self::Standard => 32,
            Self::Large => 96,
        }) * 1024
            * 1024
    }

    pub fn stl_bytes(self) -> usize {
        (match self {
            Self::Standard => 50,
            Self::Large => 200,
        }) * 1024
            * 1024
    }

    pub fn slicer_cpu_seconds(self) -> u64 {
        match self {
            Self::Standard => 60,
            Self::Large => 120,
        }
    }

    pub fn conversion(self) -> ConversionLimits {
        ConversionLimits {
            max_input_bytes: self.input_bytes(),
            max_output_stl_bytes: self.stl_bytes(),
            max_output_glb_bytes: 25 * 1024 * 1024,
            max_vertices: if self == Self::Large {
                4_000_000
            } else {
                1_000_000
            },
            max_triangles: (self.stl_bytes() - 84) / 50,
            max_parse_seconds: 25,
            ..Default::default()
        }
    }

    pub fn archive(self) -> ThreeMfLimits {
        let bytes = if self == Self::Large { 512 } else { 96 } * 1024 * 1024;
        ThreeMfLimits {
            max_entry_count: 512,
            max_uncompressed_bytes: bytes,
            max_entry_bytes: bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_explicit_localhost_only_and_budgets_remain_bounded() {
        assert_eq!(
            ModelProfile::parse(None, Some("production")).unwrap(),
            ModelProfile::Standard
        );
        assert_eq!(
            ModelProfile::parse(Some("standard"), None).unwrap(),
            ModelProfile::Standard
        );
        assert!(ModelProfile::parse(Some("large"), Some("production")).is_err());
        assert!(ModelProfile::parse(Some("large"), None).is_err());
        assert!(ModelProfile::parse(Some(""), Some("localhost")).is_err());
        assert!(ModelProfile::parse(Some("unknown"), Some("localhost")).is_err());
        assert_eq!(
            ModelProfile::parse(Some("large"), Some("localhost")).unwrap(),
            ModelProfile::Large
        );
        for profile in [ModelProfile::Standard, ModelProfile::Large] {
            let limits = profile.conversion();
            assert_eq!(limits.max_input_bytes, profile.input_bytes());
            assert_eq!(limits.max_output_glb_bytes, 25 * 1024 * 1024);
            assert_eq!(limits.max_parse_seconds, 25);
            assert!(84 + limits.max_triangles * 50 <= profile.stl_bytes());
            assert!(84 + (limits.max_triangles + 1) * 50 > profile.stl_bytes());
            assert_eq!(profile.archive().max_entry_count, 512);
        }
        assert_eq!(ModelProfile::Standard.slicer_cpu_seconds(), 60);
        assert_eq!(ModelProfile::Large.slicer_cpu_seconds(), 120);
        assert_eq!(ModelProfile::Standard.input_bytes(), 32 * 1024 * 1024);
        assert_eq!(ModelProfile::Standard.stl_bytes(), 50 * 1024 * 1024);
        assert_eq!(ModelProfile::Large.input_bytes(), 96 * 1024 * 1024);
        assert_eq!(ModelProfile::Large.stl_bytes(), 200 * 1024 * 1024);
        assert_eq!(
            ModelProfile::Large.archive().max_entry_bytes,
            512 * 1024 * 1024
        );
    }
}
