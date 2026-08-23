use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{ReleaseChannelV1, ReleaseIdentityV1, ReleaseVersionV1};
use clap::Parser;
use serde::Serialize;

const RELEASE_IDENTITY_SCHEMA: &str = "cargo-allow.release-identity.v1";

/// Validate and project one release identity for repository automation.
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReleaseIdentityArgs {
    /// Canonical stable or numbered release-candidate version.
    #[arg(long)]
    pub(super) version: String,
    /// Observed Git tag. Omit only for nonpublishing rehearsal; the canonical tag is derived.
    #[arg(long)]
    pub(super) tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReleaseIdentityProjectionV1 {
    schema: &'static str,
    result: &'static str,
    version: String,
    tag: String,
    tag_source: &'static str,
    channel: &'static str,
    rc_ordinal: Option<u32>,
    github_prerelease: bool,
}

pub(super) fn cmd_release_identity(args: &ReleaseIdentityArgs) -> CargoAllowResult<()> {
    let projection = build_release_identity_projection(&args.version, args.tag.as_deref())?;
    let rendered = serde_json::to_string_pretty(&projection).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to render release identity projection: {error}"),
        )
    })?;
    println!("{rendered}");
    Ok(())
}

fn build_release_identity_projection(
    version: &str,
    observed_tag: Option<&str>,
) -> CargoAllowResult<ReleaseIdentityProjectionV1> {
    let parsed_version = ReleaseVersionV1::parse(version).map_err(invalid_release_identity)?;
    let github_prerelease = parsed_version.channel().github_prerelease();
    let (tag, tag_source) = match observed_tag {
        Some(tag) => (tag.to_string(), "observed"),
        None => (parsed_version.tag(), "derived"),
    };
    let identity = ReleaseIdentityV1::parse(version, &tag, github_prerelease)
        .map_err(invalid_release_identity)?;
    let (channel, rc_ordinal) = match identity.version().channel() {
        ReleaseChannelV1::Stable => ("stable", None),
        ReleaseChannelV1::ReleaseCandidate { ordinal } => ("release_candidate", Some(ordinal)),
    };

    Ok(ReleaseIdentityProjectionV1 {
        schema: RELEASE_IDENTITY_SCHEMA,
        result: "validated",
        version: identity.version().as_str().to_string(),
        tag: identity.tag().to_string(),
        tag_source,
        channel,
        rc_ordinal,
        github_prerelease,
    })
}

fn invalid_release_identity(error: allow_report::ReleaseIdentityErrorV1) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::InvalidConfig,
        format!("release identity validation failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn stable_rehearsal_derives_the_canonical_tag() -> Result<(), String> {
        let projection =
            build_release_identity_projection("0.2.0", None).map_err(|error| error.to_string())?;
        if projection
            != (ReleaseIdentityProjectionV1 {
                schema: RELEASE_IDENTITY_SCHEMA,
                result: "validated",
                version: "0.2.0".to_string(),
                tag: "v0.2.0".to_string(),
                tag_source: "derived",
                channel: "stable",
                rc_ordinal: None,
                github_prerelease: false,
            })
        {
            return Err(format!("unexpected stable projection: {projection:?}"));
        }
        Ok(())
    }

    #[test]
    fn observed_rc_tag_projects_prerelease_posture() -> Result<(), String> {
        let projection = build_release_identity_projection("0.2.0-rc.1", Some("v0.2.0-rc.1"))
            .map_err(|error| error.to_string())?;
        if projection.tag_source != "observed"
            || projection.channel != "release_candidate"
            || projection.rc_ordinal != Some(1)
            || !projection.github_prerelease
        {
            return Err(format!("unexpected RC projection: {projection:?}"));
        }
        Ok(())
    }

    #[test]
    fn observed_tag_mismatch_fails_closed() -> Result<(), String> {
        let error = build_release_identity_projection("0.2.0-rc.1", Some("v0.2.0"))
            .err()
            .ok_or_else(|| "mismatched observed tag was accepted".to_string())?;
        if error.kind() != CargoAllowErrorKind::InvalidConfig
            || !error.to_string().contains("does not match expected tag")
        {
            return Err(format!("unexpected mismatch error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn unsupported_prerelease_fails_closed() -> Result<(), String> {
        let error = build_release_identity_projection("0.2.0-beta.1", None)
            .err()
            .ok_or_else(|| "unsupported prerelease was accepted".to_string())?;
        if error.kind() != CargoAllowErrorKind::InvalidConfig
            || !error
                .to_string()
                .contains("only supported prerelease form is rc.N")
        {
            return Err(format!("unexpected prerelease error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn command_is_installed_but_hidden_from_product_help() -> Result<(), String> {
        let mut command = crate::cli::CargoAllowCli::command();
        command.build();
        let release_identity = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "release-identity")
            .ok_or_else(|| "release-identity command is not installed".to_string())?;
        if !release_identity.is_hide_set() {
            return Err("release-identity must remain hidden".to_string());
        }
        let help = command.render_help().to_string();
        if help.contains("release-identity") {
            return Err("hidden release-identity leaked into root help".to_string());
        }
        Ok(())
    }
}
