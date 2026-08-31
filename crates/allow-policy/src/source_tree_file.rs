use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTreeFilePosture {
    Missing,
    RegularFile(PathBuf),
    Rejected(SourceTreeFileRejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceTreeFileRejection {
    AnchorUnresolved,
    MetadataFailure,
    TargetUnresolved,
    ExternalTarget,
    NonRegular,
    TargetMetadataFailure,
    ExternalComponent,
    UnresolvedComponent,
    ComponentMetadataFailure,
}

impl SourceTreeFileRejection {
    pub(crate) fn source_tree_reason(self) -> &'static str {
        match self {
            Self::AnchorUnresolved => "candidate source-tree anchor could not be resolved",
            Self::MetadataFailure => "candidate metadata could not be inspected",
            Self::TargetUnresolved => "candidate target could not be resolved",
            Self::ExternalTarget => "candidate target resolves outside its source-tree anchor",
            Self::NonRegular => "candidate target is not a regular file",
            Self::TargetMetadataFailure => "candidate target metadata could not be read",
            Self::ExternalComponent => {
                "candidate path contains a component outside its source-tree anchor"
            }
            Self::UnresolvedComponent => "candidate path contains an unresolved symlink component",
            Self::ComponentMetadataFailure => "candidate path component could not be inspected",
        }
    }
}

pub(crate) fn source_tree_file_posture(root: &Path, candidate: &Path) -> SourceTreeFilePosture {
    match fs::symlink_metadata(candidate) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return missing_candidate_posture(root, candidate);
        }
        Err(_) => {
            return SourceTreeFilePosture::Rejected(SourceTreeFileRejection::MetadataFailure);
        }
    }
    let resolved_root = match root.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return SourceTreeFilePosture::Rejected(SourceTreeFileRejection::AnchorUnresolved);
        }
    };
    let target = match candidate.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return SourceTreeFilePosture::Rejected(SourceTreeFileRejection::TargetUnresolved);
        }
    };
    if target.strip_prefix(&resolved_root).is_err() {
        return SourceTreeFilePosture::Rejected(SourceTreeFileRejection::ExternalTarget);
    }
    match fs::metadata(&target) {
        Ok(metadata) if metadata.is_file() => SourceTreeFilePosture::RegularFile(target),
        Ok(_) => SourceTreeFilePosture::Rejected(SourceTreeFileRejection::NonRegular),
        Err(_) => SourceTreeFilePosture::Rejected(SourceTreeFileRejection::TargetMetadataFailure),
    }
}

fn missing_candidate_posture(root: &Path, candidate: &Path) -> SourceTreeFilePosture {
    let relative = match candidate.strip_prefix(root) {
        Ok(path) => path,
        Err(_) => {
            return SourceTreeFilePosture::Rejected(SourceTreeFileRejection::ExternalTarget);
        }
    };
    let mut current = root.to_path_buf();
    let mut resolved_root = None;
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(_) => {
                let anchor = match &resolved_root {
                    Some(path) => path,
                    None => match root.canonicalize() {
                        Ok(path) => resolved_root.insert(path),
                        Err(_) => {
                            return SourceTreeFilePosture::Rejected(
                                SourceTreeFileRejection::AnchorUnresolved,
                            );
                        }
                    },
                };
                match current.canonicalize() {
                    Ok(target) if target.strip_prefix(anchor).is_ok() => {}
                    Ok(_) => {
                        return SourceTreeFilePosture::Rejected(
                            SourceTreeFileRejection::ExternalComponent,
                        );
                    }
                    Err(_) => {
                        return SourceTreeFilePosture::Rejected(
                            SourceTreeFileRejection::UnresolvedComponent,
                        );
                    }
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return SourceTreeFilePosture::Missing;
            }
            Err(_) => {
                return SourceTreeFilePosture::Rejected(
                    SourceTreeFileRejection::ComponentMetadataFailure,
                );
            }
        }
    }
    SourceTreeFilePosture::Missing
}
