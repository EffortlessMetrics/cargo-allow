//! Pure base/head comparison producing the closed movement vocabulary.
//!
//! Everything here is deterministic over the parsed inputs: indexes are
//! ordered maps, pairings follow sorted order, and no comparison depends on
//! manifest key order or lockfile record order.

use super::inputs::{
    ParsedLockPackage, ParsedRequirement, RequirementKey, RequirementOperator, WorkspaceSpecs,
    collect_workspace_specs, compare_lock_versions, parse_lockfile, parse_manifest,
    requirement_satisfied,
};
use super::{
    DependencyGraphDeltaKindV1, DependencyGraphDeltaRequestV1, DependencyGraphDeltaRowV1,
    DependencyGraphEdgeClassV1,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// Result of one full request classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassificationOutcome {
    pub rows: Vec<DependencyGraphDeltaRowV1>,
    pub instrument_failure: bool,
}

type SlotKey = (DependencyGraphEdgeClassV1, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideLabel {
    Base,
    Head,
}

impl SideLabel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Head => "head",
        }
    }
}

/// Classify one request into movement rows.
pub(crate) fn classify_request(request: &DependencyGraphDeltaRequestV1) -> ClassificationOutcome {
    let mut rows = Vec::new();
    if let Some(outcome) = validate_denominators(request, &mut rows) {
        return outcome;
    }

    let (base_requirements, head_requirements, base_lock, head_lock) =
        match parse_all_sides(request, &mut rows) {
            Some(parsed) => parsed,
            None => {
                return ClassificationOutcome {
                    rows,
                    instrument_failure: true,
                };
            }
        };

    compare_manifest_surfaces(&base_requirements, &head_requirements, &mut rows);
    check_manifest_lock_agreement(SideLabel::Base, &base_requirements, &base_lock, &mut rows);
    check_manifest_lock_agreement(SideLabel::Head, &head_requirements, &head_lock, &mut rows);
    lock_only_movements(
        &base_requirements,
        &head_requirements,
        &base_lock,
        &head_lock,
        &mut rows,
    );
    compare_lockfiles(&base_lock, &head_lock, &mut rows);

    ClassificationOutcome {
        rows,
        instrument_failure: false,
    }
}

/// Missing, empty, and zero-denominator inputs can never become clean results.
fn validate_denominators(
    request: &DependencyGraphDeltaRequestV1,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) -> Option<ClassificationOutcome> {
    let mut failed = false;
    for (label, side) in [
        (SideLabel::Base, &request.base),
        (SideLabel::Head, &request.head),
    ] {
        if side.manifests.is_empty() {
            rows.push(failure_row(
                &request.product,
                format!("{}_manifest_set_empty", label.as_str()),
                None,
            ));
            failed = true;
        }
        match &side.lockfile {
            None => {
                rows.push(failure_row(
                    &request.product,
                    format!("{}_lockfile_missing", label.as_str()),
                    None,
                ));
                failed = true;
            }
            Some(text) if text.trim().is_empty() => {
                rows.push(failure_row(
                    &request.product,
                    format!("{}_lockfile_empty", label.as_str()),
                    None,
                ));
                failed = true;
            }
            Some(_) => {}
        }
    }
    failed.then_some(ClassificationOutcome {
        rows: std::mem::take(rows),
        instrument_failure: true,
    })
}

type ParsedSides = (
    Vec<ParsedRequirement>,
    Vec<ParsedRequirement>,
    Vec<ParsedLockPackage>,
    Vec<ParsedLockPackage>,
);

fn parse_all_sides(
    request: &DependencyGraphDeltaRequestV1,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) -> Option<ParsedSides> {
    let base_requirements =
        parse_side_requirements(SideLabel::Base, &request.base.manifests, rows)?;
    let head_requirements =
        parse_side_requirements(SideLabel::Head, &request.head.manifests, rows)?;
    let base_lock = parse_side_lockfile(
        SideLabel::Base,
        &request.product,
        &request.base.lockfile,
        rows,
    )?;
    let head_lock = parse_side_lockfile(
        SideLabel::Head,
        &request.product,
        &request.head.lockfile,
        rows,
    )?;
    Some((base_requirements, head_requirements, base_lock, head_lock))
}

fn parse_side_requirements(
    label: SideLabel,
    manifests: &BTreeMap<String, String>,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) -> Option<Vec<ParsedRequirement>> {
    let mut workspace_specs = WorkspaceSpecs::new();
    for (path, text) in manifests {
        if let Err(detail) = collect_workspace_specs(text, &mut workspace_specs) {
            rows.push(failure_row("", detail, Some(path.clone())));
            return None;
        }
    }
    let mut requirements = Vec::new();
    for (path, text) in manifests {
        match parse_manifest(path, text, &workspace_specs) {
            Ok(mut parsed) => requirements.append(&mut parsed),
            Err(detail) => {
                rows.push(failure_row("", detail, Some(path.clone())));
                return None;
            }
        }
    }
    let _ = label;
    Some(requirements)
}

fn parse_side_lockfile(
    label: SideLabel,
    product: &str,
    lockfile: &Option<String>,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) -> Option<Vec<ParsedLockPackage>> {
    let text = lockfile.as_ref()?;
    match parse_lockfile(text) {
        Ok(packages) => {
            if packages.is_empty() {
                rows.push(failure_row(
                    product,
                    format!("{}_lockfile_zero_packages", label.as_str()),
                    None,
                ));
                return None;
            }
            Some(packages)
        }
        Err(detail) => {
            rows.push(failure_row(product, detail, None));
            None
        }
    }
}

fn failure_row(
    package: &str,
    detail: String,
    manifest_path: Option<String>,
) -> DependencyGraphDeltaRowV1 {
    DependencyGraphDeltaRowV1 {
        kind: DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure,
        package: package.to_string(),
        dependency_class: DependencyGraphEdgeClassV1::Normal,
        target: String::new(),
        manifest_path,
        base_version: None,
        head_version: None,
        base_requirement: None,
        head_requirement: None,
        base_source: None,
        head_source: None,
        detail,
    }
}

fn row(
    kind: DependencyGraphDeltaKindV1,
    package: &str,
    dependency_class: DependencyGraphEdgeClassV1,
    target: &str,
) -> DependencyGraphDeltaRowV1 {
    DependencyGraphDeltaRowV1 {
        kind,
        package: package.to_string(),
        dependency_class,
        target: target.to_string(),
        manifest_path: None,
        base_version: None,
        head_version: None,
        base_requirement: None,
        head_requirement: None,
        base_source: None,
        head_source: None,
        detail: String::new(),
    }
}

fn detail_token(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

/// Compare the direct requirement surfaces slot by slot.
fn compare_manifest_surfaces(
    base: &[ParsedRequirement],
    head: &[ParsedRequirement],
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    let base_index = manifest_index(base);
    let head_index = manifest_index(head);
    let names: BTreeSet<&String> = base_index.keys().chain(head_index.keys()).collect();
    for name in names {
        let base_slots = base_index.get(name);
        let head_slots = head_index.get(name);
        let base_keys: BTreeSet<&SlotKey> = base_slots
            .map(|slots| slots.keys().collect())
            .unwrap_or_default();
        let head_keys: BTreeSet<&SlotKey> = head_slots
            .map(|slots| slots.keys().collect())
            .unwrap_or_default();
        for slot in base_keys.intersection(&head_keys) {
            let base_requirement = base_slots.and_then(|slots| slots.get(*slot));
            let head_requirement = head_slots.and_then(|slots| slots.get(*slot));
            if let (Some(base_requirement), Some(head_requirement)) =
                (base_requirement, head_requirement)
            {
                compare_requirements(base_requirement, head_requirement, rows);
            }
        }
        let base_only: Vec<&SlotKey> = base_keys.difference(&head_keys).copied().collect();
        let head_only: Vec<&SlotKey> = head_keys.difference(&base_keys).copied().collect();
        let paired = base_only.len().min(head_only.len());
        for index in 0..paired {
            let base_slot = base_only
                .get(index)
                .and_then(|slot| base_slots.and_then(|slots| slots.get(*slot)));
            let head_slot = head_only
                .get(index)
                .and_then(|slot| head_slots.and_then(|slots| slots.get(*slot)));
            if let (Some(base_requirement), Some(head_requirement)) = (base_slot, head_slot) {
                let mut moved = row(
                    DependencyGraphDeltaKindV1::TargetOrDependencyClassChanged,
                    &head_requirement.display_name,
                    head_requirement.class,
                    &head_requirement.target,
                );
                moved.manifest_path = Some(head_requirement.path.clone());
                moved.base_requirement = Some(base_requirement.requirements.join(", "));
                moved.head_requirement = Some(head_requirement.requirements.join(", "));
                moved.detail = format!(
                    "site_moved_from_{}_target_{}_to_{}_target_{}",
                    base_requirement.class.as_str(),
                    detail_token(&base_requirement.target),
                    head_requirement.class.as_str(),
                    detail_token(&head_requirement.target),
                );
                rows.push(moved);
            }
        }
        for slot in base_only.iter().skip(paired) {
            if let Some(base_requirement) = base_slots.and_then(|slots| slots.get(*slot)) {
                let mut removed = row(
                    DependencyGraphDeltaKindV1::DirectRequirementRemoved,
                    &base_requirement.display_name,
                    base_requirement.class,
                    &base_requirement.target,
                );
                removed.manifest_path = Some(base_requirement.path.clone());
                removed.base_requirement = Some(base_requirement.requirements.join(", "));
                removed.detail = "direct_requirement_removed".to_string();
                rows.push(removed);
            }
        }
        for slot in head_only.iter().skip(paired) {
            if let Some(head_requirement) = head_slots.and_then(|slots| slots.get(*slot)) {
                let mut added = row(
                    DependencyGraphDeltaKindV1::DirectRequirementAdded,
                    &head_requirement.display_name,
                    head_requirement.class,
                    &head_requirement.target,
                );
                added.manifest_path = Some(head_requirement.path.clone());
                added.head_requirement = Some(head_requirement.requirements.join(", "));
                added.detail = "direct_requirement_added".to_string();
                rows.push(added);
            }
        }
    }
}

fn manifest_index(
    requirements: &[ParsedRequirement],
) -> BTreeMap<String, BTreeMap<SlotKey, &ParsedRequirement>> {
    let mut index: BTreeMap<String, BTreeMap<SlotKey, &ParsedRequirement>> = BTreeMap::new();
    for requirement in requirements {
        index
            .entry(requirement.name_key.clone())
            .or_default()
            .insert((requirement.class, requirement.target.clone()), requirement);
    }
    index
}

/// Compare one matched requirement slot across sides.
fn compare_requirements(
    base: &ParsedRequirement,
    head: &ParsedRequirement,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    if base.canonical != head.canonical {
        let mut moved = row(
            requirement_movement_kind(base, head),
            &head.display_name,
            head.class,
            &head.target,
        );
        moved.manifest_path = Some(head.path.clone());
        moved.base_requirement = Some(base.requirements.join(", "));
        moved.head_requirement = Some(head.requirements.join(", "));
        moved.detail = format!(
            "requirement_moved_{}_to_{}",
            base.canonical_label(),
            head.canonical_label()
        );
        rows.push(moved);
    }
    if base.source_spec != head.source_spec {
        let mut changed = row(
            DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
            &head.display_name,
            head.class,
            &head.target,
        );
        changed.manifest_path = Some(head.path.clone());
        changed.base_source = base.source_spec.clone();
        changed.head_source = head.source_spec.clone();
        changed.detail = "manifest_source_spec_changed".to_string();
        rows.push(changed);
    }
    if base.features != head.features {
        let gained: Vec<String> = head.features.difference(&base.features).cloned().collect();
        let lost: Vec<String> = base.features.difference(&head.features).cloned().collect();
        let mut changed = row(
            DependencyGraphDeltaKindV1::FeatureActivationChanged,
            &head.display_name,
            head.class,
            &head.target,
        );
        changed.manifest_path = Some(head.path.clone());
        changed.detail = format!(
            "manifest_features_gained_{}_lost_{}",
            csv_or_none(&gained),
            csv_or_none(&lost)
        );
        rows.push(changed);
    }
    if base.default_features != head.default_features {
        let mut changed = row(
            DependencyGraphDeltaKindV1::FeatureActivationChanged,
            &head.display_name,
            head.class,
            &head.target,
        );
        changed.manifest_path = Some(head.path.clone());
        changed.detail = if head.default_features {
            "manifest_default_features_enabled".to_string()
        } else {
            "manifest_default_features_disabled".to_string()
        };
        rows.push(changed);
    }
    if base.optional != head.optional {
        let mut changed = row(
            DependencyGraphDeltaKindV1::FeatureActivationChanged,
            &head.display_name,
            head.class,
            &head.target,
        );
        changed.manifest_path = Some(head.path.clone());
        changed.detail = if head.optional {
            "requirement_became_optional".to_string()
        } else {
            "requirement_became_mandatory".to_string()
        };
        rows.push(changed);
    }
}

fn requirement_movement_kind(
    base: &ParsedRequirement,
    head: &ParsedRequirement,
) -> DependencyGraphDeltaKindV1 {
    if base.canonical.len() == 1 && head.canonical.len() == 1 {
        let base_key = base.canonical.first().copied();
        let head_key = head.canonical.first().copied();
        if let (Some(base_key), Some(head_key)) = (base_key, head_key) {
            return single_requirement_movement_kind(base_key, head_key);
        }
    }
    requirement_set_movement_kind(base, head)
}

fn single_requirement_movement_kind(
    base_key: RequirementKey,
    head_key: RequirementKey,
) -> DependencyGraphDeltaKindV1 {
    // Exact pins classify by pin direction, not floor arithmetic: pinning an
    // in-range version is a narrowing, leaving a pin upward is a raise.
    match (base_key.operator, head_key.operator) {
        (RequirementOperator::Exact, RequirementOperator::Exact) => {
            single_floor_movement_kind(base_key, head_key)
        }
        (_, RequirementOperator::Exact) => {
            if head_key.floor < base_key.floor {
                DependencyGraphDeltaKindV1::DirectRequirementLowered
            } else {
                DependencyGraphDeltaKindV1::RequirementRangeNarrowed
            }
        }
        (RequirementOperator::Exact, _) => {
            if head_key.floor > base_key.floor {
                DependencyGraphDeltaKindV1::DirectRequirementRaised
            } else {
                DependencyGraphDeltaKindV1::RequirementRangeBroadened
            }
        }
        _ => single_floor_movement_kind(base_key, head_key),
    }
}

fn single_floor_movement_kind(
    base_key: RequirementKey,
    head_key: RequirementKey,
) -> DependencyGraphDeltaKindV1 {
    match head_key.floor.cmp(&base_key.floor) {
        Ordering::Greater => DependencyGraphDeltaKindV1::DirectRequirementRaised,
        Ordering::Less => DependencyGraphDeltaKindV1::DirectRequirementLowered,
        Ordering::Equal => match head_key.operator.cmp(&base_key.operator) {
            Ordering::Greater => DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
            Ordering::Less => DependencyGraphDeltaKindV1::RequirementRangeBroadened,
            // Equal operators with equal floors cannot differ canonically.
            Ordering::Equal => DependencyGraphDeltaKindV1::RequirementRangeBroadened,
        },
    }
}

fn requirement_set_movement_kind(
    base: &ParsedRequirement,
    head: &ParsedRequirement,
) -> DependencyGraphDeltaKindV1 {
    let base_floors: BTreeSet<(u64, u64, u64)> =
        base.canonical.iter().map(|key| key.floor).collect();
    let head_floors: BTreeSet<(u64, u64, u64)> =
        head.canonical.iter().map(|key| key.floor).collect();
    if head_floors.is_superset(&base_floors) && head_floors != base_floors {
        return DependencyGraphDeltaKindV1::RequirementRangeBroadened;
    }
    if base_floors.is_superset(&head_floors) && head_floors != base_floors {
        return DependencyGraphDeltaKindV1::RequirementRangeNarrowed;
    }
    let base_minmax = min_max_floor(&base_floors);
    let head_minmax = min_max_floor(&head_floors);
    if let (Some((base_min, base_max)), Some((head_min, head_max))) = (base_minmax, head_minmax) {
        return match (head_min.cmp(&base_min), head_max.cmp(&base_max)) {
            (Ordering::Less, _) => DependencyGraphDeltaKindV1::RequirementRangeBroadened,
            (Ordering::Greater, _) => DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
            (Ordering::Equal, Ordering::Greater) => {
                DependencyGraphDeltaKindV1::RequirementRangeBroadened
            }
            (Ordering::Equal, Ordering::Less) => {
                DependencyGraphDeltaKindV1::RequirementRangeNarrowed
            }
            (Ordering::Equal, Ordering::Equal) => {
                // Same floor span with different operator strictness.
                let base_strictest = strictest_operator(base);
                let head_strictest = strictest_operator(head);
                if head_strictest > base_strictest {
                    DependencyGraphDeltaKindV1::RequirementRangeNarrowed
                } else {
                    DependencyGraphDeltaKindV1::RequirementRangeBroadened
                }
            }
        };
    }
    DependencyGraphDeltaKindV1::RequirementRangeBroadened
}

/// Semver version triple `(major, minor, patch)`.
type VersionTriple = (u64, u64, u64);

fn min_max_floor(floors: &BTreeSet<VersionTriple>) -> Option<(VersionTriple, VersionTriple)> {
    let min = floors.iter().next().copied();
    let max = floors.iter().next_back().copied();
    match (min, max) {
        (Some(min), Some(max)) => Some((min, max)),
        _ => None,
    }
}

fn strictest_operator(requirement: &ParsedRequirement) -> RequirementOperator {
    requirement
        .canonical
        .iter()
        .map(|key| key.operator)
        .max()
        .unwrap_or(RequirementOperator::Star)
}

/// Every direct requirement must be satisfied by its side's lockfile.
fn check_manifest_lock_agreement(
    label: SideLabel,
    requirements: &[ParsedRequirement],
    lock: &[ParsedLockPackage],
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    for requirement in requirements {
        let candidates: Vec<&ParsedLockPackage> = lock
            .iter()
            .filter(|package| package.name_key == requirement.package_key)
            .collect();
        if candidates.is_empty() {
            let mut mismatch = row(
                DependencyGraphDeltaKindV1::ManifestLockMismatch,
                &requirement.display_name,
                requirement.class,
                &requirement.target,
            );
            mismatch.manifest_path = Some(requirement.path.clone());
            push_side_requirement(&mut mismatch, label, &requirement.requirements.join(", "));
            mismatch.detail = format!("package_absent_from_{}_lockfile", label.as_str());
            rows.push(mismatch);
            continue;
        }
        let satisfied = requirement.canonical.iter().any(|key| {
            candidates
                .iter()
                .any(|package| requirement_satisfied(key, &package.version))
        });
        if !satisfied {
            let mut mismatch = row(
                DependencyGraphDeltaKindV1::ManifestLockMismatch,
                &requirement.display_name,
                requirement.class,
                &requirement.target,
            );
            mismatch.manifest_path = Some(requirement.path.clone());
            push_side_requirement(&mut mismatch, label, &requirement.requirements.join(", "));
            let resolved = min_version(&candidates);
            push_side_version(&mut mismatch, label, resolved.as_deref());
            mismatch.detail = format!("requirement_unsatisfied_in_{}_lockfile", label.as_str());
            rows.push(mismatch);
        }
    }
}

fn push_side_requirement(
    moved_row: &mut DependencyGraphDeltaRowV1,
    label: SideLabel,
    requirement: &str,
) {
    match label {
        SideLabel::Base => moved_row.base_requirement = Some(requirement.to_string()),
        SideLabel::Head => moved_row.head_requirement = Some(requirement.to_string()),
    }
}

fn push_side_version(
    moved_row: &mut DependencyGraphDeltaRowV1,
    label: SideLabel,
    version: Option<&str>,
) {
    let owned = version.map(str::to_string);
    match label {
        SideLabel::Base => moved_row.base_version = owned,
        SideLabel::Head => moved_row.head_version = owned,
    }
}

/// Minimum lockfile version among candidates, deterministic.
fn min_version(candidates: &[&ParsedLockPackage]) -> Option<String> {
    candidates
        .iter()
        .map(|package| package.version.clone())
        .min_by(|left, right| compare_lock_versions(left, right))
}

/// Minimum lockfile version satisfying the requirement, for resolution movement.
fn resolve_min_satisfying(
    requirement: &ParsedRequirement,
    lock: &[ParsedLockPackage],
) -> Option<String> {
    let candidates: Vec<&ParsedLockPackage> = lock
        .iter()
        .filter(|package| package.name_key == requirement.package_key)
        .filter(|package| {
            requirement
                .canonical
                .iter()
                .any(|key| requirement_satisfied(key, &package.version))
        })
        .collect();
    min_version(&candidates)
}

/// Requirement text unchanged but the lockfile resolution moved anyway.
fn lock_only_movements(
    base_requirements: &[ParsedRequirement],
    head_requirements: &[ParsedRequirement],
    base_lock: &[ParsedLockPackage],
    head_lock: &[ParsedLockPackage],
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    let base_index = manifest_index(base_requirements);
    for head_requirement in head_requirements {
        let Some(base_requirement) = base_index
            .get(&head_requirement.name_key)
            .and_then(|slots| {
                slots.get(&(head_requirement.class, head_requirement.target.clone()))
            })
        else {
            continue;
        };
        if base_requirement.canonical != head_requirement.canonical {
            continue;
        }
        let base_resolved = resolve_min_satisfying(base_requirement, base_lock);
        let head_resolved = resolve_min_satisfying(head_requirement, head_lock);
        if let (Some(base_resolved), Some(head_resolved)) = (base_resolved, head_resolved)
            && base_resolved != head_resolved
        {
            let mut moved = row(
                DependencyGraphDeltaKindV1::LockOnlyResolutionChanged,
                &head_requirement.display_name,
                head_requirement.class,
                &head_requirement.target,
            );
            moved.manifest_path = Some(head_requirement.path.clone());
            moved.base_version = Some(base_resolved);
            moved.head_version = Some(head_resolved);
            moved.base_requirement = Some(base_requirement.requirements.join(", "));
            moved.head_requirement = Some(head_requirement.requirements.join(", "));
            moved.detail = "manifest_requirement_unchanged_lockfile_resolution_moved".to_string();
            rows.push(moved);
        }
    }
}

/// Resolved package graph movement: adds, removals, upgrades, downgrades,
/// source/checksum drift, duplicate-version movement, and edge changes.
fn compare_lockfiles(
    base_lock: &[ParsedLockPackage],
    head_lock: &[ParsedLockPackage],
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    let mut names: BTreeSet<&String> = BTreeSet::new();
    names.extend(base_lock.iter().map(|package| &package.name_key));
    names.extend(head_lock.iter().map(|package| &package.name_key));
    let known_names: BTreeSet<String> = names.iter().map(|name| (*name).clone()).collect();
    for name in names {
        let base_packages: Vec<&ParsedLockPackage> = base_lock
            .iter()
            .filter(|package| &package.name_key == name)
            .collect();
        let head_packages: Vec<&ParsedLockPackage> = head_lock
            .iter()
            .filter(|package| &package.name_key == name)
            .collect();
        let display = head_packages
            .first()
            .map(|package| package.name.clone())
            .unwrap_or_else(|| {
                base_packages
                    .first()
                    .map(|package| package.name.clone())
                    .unwrap_or_default()
            });
        compare_package_versions(&display, &base_packages, &head_packages, &known_names, rows);
    }
}

fn compare_package_versions(
    display: &str,
    base_packages: &[&ParsedLockPackage],
    head_packages: &[&ParsedLockPackage],
    known_names: &BTreeSet<String>,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    let base_versions: BTreeSet<&str> = base_packages
        .iter()
        .map(|package| package.version.as_str())
        .collect();
    let head_versions: BTreeSet<&str> = head_packages
        .iter()
        .map(|package| package.version.as_str())
        .collect();
    if base_versions.len().max(head_versions.len()) >= 2
        && base_versions.len() != head_versions.len()
    {
        let mut moved = row(
            DependencyGraphDeltaKindV1::DuplicateVersionMovement,
            display,
            DependencyGraphEdgeClassV1::Normal,
            "",
        );
        moved.detail = format!(
            "duplicate_version_count_moved_{}_to_{}",
            base_versions.len(),
            head_versions.len()
        );
        rows.push(moved);
    }

    let mut base_unmatched: BTreeMap<(String, String, String), &ParsedLockPackage> =
        BTreeMap::new();
    for package in base_packages {
        base_unmatched.insert(identity_key(package), package);
    }
    let mut head_unmatched: BTreeMap<(String, String, String), &ParsedLockPackage> =
        BTreeMap::new();
    for package in head_packages {
        head_unmatched.insert(identity_key(package), package);
    }
    let matched_keys: Vec<(String, String, String)> = base_unmatched
        .keys()
        .filter(|key| head_unmatched.contains_key(*key))
        .cloned()
        .collect();
    for key in matched_keys {
        let base_package = base_unmatched.remove(&key);
        let head_package = head_unmatched.remove(&key);
        if let (Some(base_package), Some(head_package)) = (base_package, head_package) {
            compare_lock_edges(base_package, head_package, known_names, rows);
        }
    }
    let base_only: Vec<&ParsedLockPackage> = base_unmatched.values().copied().collect();
    let head_only: Vec<&ParsedLockPackage> = head_unmatched.values().copied().collect();
    let paired = base_only.len().min(head_only.len());
    for index in 0..paired {
        let base_package = base_only.get(index).copied();
        let head_package = head_only.get(index).copied();
        if let (Some(base_package), Some(head_package)) = (base_package, head_package) {
            match compare_lock_versions(&base_package.version, &head_package.version) {
                Ordering::Equal => {
                    let mut changed = row(
                        DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
                        display,
                        DependencyGraphEdgeClassV1::Normal,
                        "",
                    );
                    changed.base_version = Some(base_package.version.clone());
                    changed.head_version = Some(head_package.version.clone());
                    changed.base_source = base_package.source.clone();
                    changed.head_source = head_package.source.clone();
                    changed.detail = "source_or_checksum_changed_same_version".to_string();
                    rows.push(changed);
                }
                Ordering::Less => {
                    push_version_movement(
                        DependencyGraphDeltaKindV1::PackageUpgraded,
                        display,
                        base_package,
                        head_package,
                        rows,
                    );
                }
                Ordering::Greater => {
                    push_version_movement(
                        DependencyGraphDeltaKindV1::PackageDowngraded,
                        display,
                        base_package,
                        head_package,
                        rows,
                    );
                }
            }
        }
    }
    for package in base_only.iter().skip(paired) {
        let mut removed = row(
            DependencyGraphDeltaKindV1::PackageRemoved,
            display,
            DependencyGraphEdgeClassV1::Normal,
            "",
        );
        removed.base_version = Some(package.version.clone());
        removed.base_source = package.source.clone();
        removed.detail = "package_removed_from_lockfile".to_string();
        rows.push(removed);
    }
    for package in head_only.iter().skip(paired) {
        let mut added = row(
            DependencyGraphDeltaKindV1::PackageAdded,
            display,
            DependencyGraphEdgeClassV1::Normal,
            "",
        );
        added.head_version = Some(package.version.clone());
        added.head_source = package.source.clone();
        added.detail = "package_added_to_lockfile".to_string();
        rows.push(added);
    }
}

fn identity_key(package: &ParsedLockPackage) -> (String, String, String) {
    (
        package.version.clone(),
        package.source.clone().unwrap_or_default(),
        package.checksum.clone().unwrap_or_default(),
    )
}

fn push_version_movement(
    kind: DependencyGraphDeltaKindV1,
    display: &str,
    base_package: &ParsedLockPackage,
    head_package: &ParsedLockPackage,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    let mut moved = row(kind, display, DependencyGraphEdgeClassV1::Normal, "");
    moved.base_version = Some(base_package.version.clone());
    moved.head_version = Some(head_package.version.clone());
    moved.base_source = base_package.source.clone();
    moved.head_source = head_package.source.clone();
    moved.detail = format!(
        "package_version_moved_{}_to_{}",
        detail_token(&base_package.version),
        detail_token(&head_package.version)
    );
    rows.push(moved);
}

/// Edge movement for an exactly matched package: feature-activation edges and
/// whole-edge changes stay visible even when name and version did not move.
///
/// An added/removed edge counts as feature activation when it carries a
/// feature marker or points at a package that already exists in the graph;
/// anything else is a lock-only resolution edge movement.
fn compare_lock_edges(
    base_package: &ParsedLockPackage,
    head_package: &ParsedLockPackage,
    known_names: &BTreeSet<String>,
    rows: &mut Vec<DependencyGraphDeltaRowV1>,
) {
    let added: Vec<&super::inputs::LockEdge> =
        head_package.edges.difference(&base_package.edges).collect();
    let removed: Vec<&super::inputs::LockEdge> =
        base_package.edges.difference(&head_package.edges).collect();
    if added.is_empty() && removed.is_empty() {
        return;
    }
    let all_feature_edges = added.iter().chain(removed.iter()).all(|edge| {
        edge.feature.is_some() || edge.optional_activation || known_names.contains(&edge.name_key)
    });
    let label = |edges: &[&super::inputs::LockEdge]| -> String {
        let labels: Vec<String> = edges.iter().map(|edge| edge_label(edge)).collect();
        csv_or_none(&labels)
    };
    let mut changed = row(
        if all_feature_edges {
            DependencyGraphDeltaKindV1::FeatureActivationChanged
        } else {
            DependencyGraphDeltaKindV1::LockOnlyResolutionChanged
        },
        &head_package.name,
        DependencyGraphEdgeClassV1::Normal,
        "",
    );
    changed.base_version = Some(base_package.version.clone());
    changed.head_version = Some(head_package.version.clone());
    changed.detail = if all_feature_edges {
        format!(
            "lockfile_edge_features_added_{}_removed_{}",
            label(&added),
            label(&removed)
        )
    } else {
        format!(
            "lockfile_edges_added_{}_removed_{}",
            label(&added),
            label(&removed)
        )
    };
    rows.push(changed);
}

fn edge_label(edge: &super::inputs::LockEdge) -> String {
    match (&edge.feature, edge.optional_activation) {
        (Some(feature), true) => format!("{}?{}", edge.name_key, feature),
        (Some(feature), false) => format!("{}:{}", edge.name_key, feature),
        (None, true) => format!("{}?", edge.name_key),
        (None, false) => edge.name_key.clone(),
    }
}

fn csv_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(",")
    }
}
