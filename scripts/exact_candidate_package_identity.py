"""Exact package/version identity helpers for candidate .crate archives."""


def crate_version_from_filename(package_name: str, crate_file: str) -> str:
    """Return the complete version suffix for an exact Cargo archive filename.

    This function intentionally extracts identity only. Cargo package metadata and
    final-packaged-surface reconciliation remain the semantic version authorities.
    """
    if not package_name:
        raise ValueError("package name must not be empty")
    if "/" in crate_file or "\\" in crate_file:
        raise ValueError(f"crate archive must be a filename, got {crate_file!r}")

    suffix = ".crate"
    if not crate_file.endswith(suffix):
        raise ValueError(f"crate archive must end with {suffix}: {crate_file!r}")

    stem = crate_file[: -len(suffix)]
    prefix = f"{package_name}-"
    if not stem.startswith(prefix):
        raise ValueError(
            f"crate archive {crate_file!r} does not match package {package_name!r}"
        )

    version = stem[len(prefix) :]
    if not version:
        raise ValueError(f"crate archive {crate_file!r} has an empty version")
    return version
