//! Typed stable and release-candidate identity for release control surfaces.
//!
//! The contract intentionally supports the channels cargo-allow is prepared to
//! release: stable SemVer and numbered `-rc.N` candidates. Other prerelease
//! spellings and build metadata fail closed until their semantics are designed.

use std::fmt;

/// Supported release channels for cargo-allow release control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseChannelV1 {
    Stable,
    ReleaseCandidate { ordinal: u32 },
}

impl ReleaseChannelV1 {
    pub const fn github_prerelease(self) -> bool {
        matches!(self, Self::ReleaseCandidate { .. })
    }
}

/// Parsed, canonical version identity used by release surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseVersionV1 {
    canonical: String,
    core: String,
    channel: ReleaseChannelV1,
}

impl ReleaseVersionV1 {
    pub fn parse(value: &str) -> Result<Self, ReleaseIdentityErrorV1> {
        if value.is_empty() || value.trim() != value {
            return malformed(
                value,
                "version must be non-empty and contain no surrounding whitespace",
            );
        }
        if value.contains('+') {
            return malformed(
                value,
                "build metadata is not supported by the release identity contract",
            );
        }

        let (core, channel) = match value.split_once('-') {
            Some((core, prerelease)) => (
                core,
                ReleaseChannelV1::ReleaseCandidate {
                    ordinal: parse_rc_ordinal(prerelease, value)?,
                },
            ),
            None => (value, ReleaseChannelV1::Stable),
        };
        validate_core(core, value)?;

        Ok(Self {
            canonical: value.to_string(),
            core: core.to_string(),
            channel,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    /// The validated `major.minor.patch` core without any prerelease suffix.
    pub fn core(&self) -> &str {
        &self.core
    }

    pub const fn channel(&self) -> ReleaseChannelV1 {
        self.channel
    }

    pub fn tag(&self) -> String {
        format!("v{}", self.canonical)
    }
}

/// One validated release identity across version, tag, and GitHub release state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseIdentityV1 {
    version: ReleaseVersionV1,
    tag: String,
    github_prerelease: bool,
}

impl ReleaseIdentityV1 {
    pub fn parse(
        version: &str,
        tag: &str,
        github_prerelease: bool,
    ) -> Result<Self, ReleaseIdentityErrorV1> {
        let version = ReleaseVersionV1::parse(version)?;
        let expected_tag = version.tag();
        if tag != expected_tag {
            return Err(ReleaseIdentityErrorV1::TagMismatch {
                expected: expected_tag,
                actual: tag.to_string(),
            });
        }

        let expected_prerelease = version.channel().github_prerelease();
        if github_prerelease != expected_prerelease {
            return Err(ReleaseIdentityErrorV1::GithubPrereleaseMismatch {
                expected: expected_prerelease,
                actual: github_prerelease,
            });
        }

        Ok(Self {
            version,
            tag: tag.to_string(),
            github_prerelease,
        })
    }

    pub const fn version(&self) -> &ReleaseVersionV1 {
        &self.version
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub const fn github_prerelease(&self) -> bool {
        self.github_prerelease
    }

    /// Validate a package on the candidate line against the exact release bytes.
    pub fn validate_candidate_package_version(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<(), ReleaseIdentityErrorV1> {
        let actual = ReleaseVersionV1::parse(package_version)?;
        if actual != self.version {
            return Err(ReleaseIdentityErrorV1::CandidatePackageVersionMismatch {
                package_name: package_name.to_string(),
                expected: self.version.as_str().to_string(),
                actual: package_version.to_string(),
            });
        }
        Ok(())
    }

    /// Validate an independently versioned prerequisite against an exact stable line.
    pub fn validate_independent_stable_package_version(
        package_name: &str,
        expected_version: &str,
        package_version: &str,
    ) -> Result<(), ReleaseIdentityErrorV1> {
        let expected = ReleaseVersionV1::parse(expected_version)?;
        if expected.channel() != ReleaseChannelV1::Stable {
            return Err(ReleaseIdentityErrorV1::IndependentPackageLineNotStable {
                package_name: package_name.to_string(),
                version: expected_version.to_string(),
            });
        }

        let actual = ReleaseVersionV1::parse(package_version)?;
        if actual != expected {
            return Err(ReleaseIdentityErrorV1::IndependentPackageVersionMismatch {
                package_name: package_name.to_string(),
                expected: expected_version.to_string(),
                actual: package_version.to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseIdentityErrorV1 {
    MalformedVersion {
        value: String,
        reason: &'static str,
    },
    TagMismatch {
        expected: String,
        actual: String,
    },
    GithubPrereleaseMismatch {
        expected: bool,
        actual: bool,
    },
    CandidatePackageVersionMismatch {
        package_name: String,
        expected: String,
        actual: String,
    },
    IndependentPackageLineNotStable {
        package_name: String,
        version: String,
    },
    IndependentPackageVersionMismatch {
        package_name: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ReleaseIdentityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedVersion { value, reason } => {
                write!(formatter, "malformed release version `{value}`: {reason}")
            }
            Self::TagMismatch { expected, actual } => write!(
                formatter,
                "release tag `{actual}` does not match expected tag `{expected}`"
            ),
            Self::GithubPrereleaseMismatch { expected, actual } => write!(
                formatter,
                "GitHub prerelease state `{actual}` does not match expected state `{expected}`"
            ),
            Self::CandidatePackageVersionMismatch {
                package_name,
                expected,
                actual,
            } => write!(
                formatter,
                "candidate package `{package_name}` uses `{actual}` instead of `{expected}`"
            ),
            Self::IndependentPackageLineNotStable {
                package_name,
                version,
            } => write!(
                formatter,
                "independent package `{package_name}` expected line `{version}` is not stable"
            ),
            Self::IndependentPackageVersionMismatch {
                package_name,
                expected,
                actual,
            } => write!(
                formatter,
                "independent package `{package_name}` uses `{actual}` instead of `{expected}`"
            ),
        }
    }
}

impl std::error::Error for ReleaseIdentityErrorV1 {}

fn malformed<T>(value: &str, reason: &'static str) -> Result<T, ReleaseIdentityErrorV1> {
    Err(ReleaseIdentityErrorV1::MalformedVersion {
        value: value.to_string(),
        reason,
    })
}

fn validate_core(core: &str, full_version: &str) -> Result<(), ReleaseIdentityErrorV1> {
    let mut identifiers = core.split('.');
    for _ in 0..3 {
        let Some(identifier) = identifiers.next() else {
            return malformed(
                full_version,
                "version core must contain exactly major.minor.patch",
            );
        };
        parse_numeric_identifier(identifier, full_version)?;
    }
    if identifiers.next().is_some() {
        return malformed(
            full_version,
            "version core must contain exactly major.minor.patch",
        );
    }
    Ok(())
}

fn parse_rc_ordinal(prerelease: &str, full_version: &str) -> Result<u32, ReleaseIdentityErrorV1> {
    let Some(identifier) = prerelease.strip_prefix("rc.") else {
        return malformed(full_version, "the only supported prerelease form is rc.N");
    };
    let ordinal = parse_numeric_identifier(identifier, full_version)?;
    let ordinal = u32::try_from(ordinal).map_err(|_| ReleaseIdentityErrorV1::MalformedVersion {
        value: full_version.to_string(),
        reason: "rc ordinal exceeds the supported range",
    })?;
    if ordinal == 0 {
        return malformed(full_version, "rc ordinal must be greater than zero");
    }
    Ok(ordinal)
}

fn parse_numeric_identifier(
    identifier: &str,
    full_version: &str,
) -> Result<u64, ReleaseIdentityErrorV1> {
    if identifier.is_empty()
        || (identifier.len() > 1 && identifier.starts_with('0'))
        || !identifier
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return malformed(
            full_version,
            "numeric identifiers must be canonical unsigned integers",
        );
    }
    identifier
        .parse::<u64>()
        .map_err(|_| ReleaseIdentityErrorV1::MalformedVersion {
            value: full_version.to_string(),
            reason: "numeric identifier exceeds the supported range",
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_accessor_exposes_the_validated_release_line() -> Result<(), String> {
        let rc = ReleaseVersionV1::parse("0.2.0-rc.1").map_err(|error| error.to_string())?;
        if rc.core() != "0.2.0" || rc.as_str() != "0.2.0-rc.1" {
            return Err(format!("core accessor lost identity: {rc:?}"));
        }
        let stable = ReleaseVersionV1::parse("0.1.11").map_err(|error| error.to_string())?;
        if stable.core() != "0.1.11" {
            return Err(format!("stable core accessor lost identity: {stable:?}"));
        }
        Ok(())
    }

    #[test]
    fn stable_identity_binds_version_tag_and_release_state() -> Result<(), String> {
        let identity = ReleaseIdentityV1::parse("0.2.0", "v0.2.0", false)
            .map_err(|error| error.to_string())?;
        if identity.version().channel() != ReleaseChannelV1::Stable
            || identity.tag() != "v0.2.0"
            || identity.github_prerelease()
        {
            return Err(format!("stable identity lost information: {identity:?}"));
        }
        Ok(())
    }

    #[test]
    fn rc_identity_binds_version_tag_and_prerelease_state() -> Result<(), String> {
        let identity = ReleaseIdentityV1::parse("0.2.0-rc.1", "v0.2.0-rc.1", true)
            .map_err(|error| error.to_string())?;
        if identity.version().channel() != (ReleaseChannelV1::ReleaseCandidate { ordinal: 1 })
            || !identity.github_prerelease()
        {
            return Err(format!("RC identity lost channel state: {identity:?}"));
        }
        identity
            .validate_candidate_package_version("cargo-allow", "0.2.0-rc.1")
            .map_err(|error| error.to_string())?;
        ReleaseIdentityV1::validate_independent_stable_package_version(
            "effortless-repo-edit",
            "0.1.0",
            "0.1.0",
        )
        .map_err(|error| error.to_string())
    }

    #[test]
    fn tag_and_candidate_byte_mismatches_fail_closed() -> Result<(), String> {
        let identity = ReleaseIdentityV1::parse("0.2.0-rc.1", "v0.2.0-rc.1", true)
            .map_err(|error| error.to_string())?;
        if !matches!(
            identity.validate_candidate_package_version("cargo-allow", "0.2.0"),
            Err(ReleaseIdentityErrorV1::CandidatePackageVersionMismatch { .. })
        ) {
            return Err("stable candidate bytes were accepted under an RC tag".to_string());
        }
        if !matches!(
            ReleaseIdentityV1::parse("0.2.0-rc.1", "v0.2.0", true),
            Err(ReleaseIdentityErrorV1::TagMismatch { .. })
        ) {
            return Err("stable tag was accepted for RC package identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn github_release_posture_must_follow_the_channel() -> Result<(), String> {
        for (version, tag, prerelease) in [
            ("0.2.0-rc.1", "v0.2.0-rc.1", false),
            ("0.2.0", "v0.2.0", true),
        ] {
            if !matches!(
                ReleaseIdentityV1::parse(version, tag, prerelease),
                Err(ReleaseIdentityErrorV1::GithubPrereleaseMismatch { .. })
            ) {
                return Err(format!(
                    "channel/posture mismatch was accepted: {version} {tag} {prerelease}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn independent_package_line_must_remain_exact_and_stable() -> Result<(), String> {
        if !matches!(
            ReleaseIdentityV1::validate_independent_stable_package_version(
                "effortless-repo-edit",
                "0.1.0",
                "0.2.0-rc.1",
            ),
            Err(ReleaseIdentityErrorV1::IndependentPackageVersionMismatch { .. })
        ) {
            return Err("candidate version leaked into an independent line".to_string());
        }
        if !matches!(
            ReleaseIdentityV1::validate_independent_stable_package_version(
                "effortless-repo-edit",
                "0.1.0-rc.1",
                "0.1.0-rc.1",
            ),
            Err(ReleaseIdentityErrorV1::IndependentPackageLineNotStable { .. })
        ) {
            return Err("independent prerelease line was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn unsupported_or_noncanonical_versions_fail_closed() -> Result<(), String> {
        for malformed in [
            "",
            "0.2",
            "0.2.0.1",
            "00.2.0",
            "0.02.0",
            "0.2.00",
            "0.2.0-alpha.1",
            "0.2.0-rc.0",
            "0.2.0-rc.01",
            "0.2.0-rc.1.extra",
            "0.2.0+build.1",
            " 0.2.0",
        ] {
            if ReleaseVersionV1::parse(malformed).is_ok() {
                return Err(format!("malformed version was accepted: {malformed}"));
            }
        }
        Ok(())
    }
}
