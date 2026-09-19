from __future__ import annotations

import re
from pathlib import Path
from textwrap import dedent


def sub_once(path: Path, pattern: str, replacement: str, *, flags: int = 0) -> None:
    text = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one structural match, found {count}: {pattern[:120]!r}"
        )
    path.write_text(updated, encoding="utf-8", newline="\n")


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one literal match, found {count}: {old[:120]!r}"
        )
    path.write_text(text.replace(old, new), encoding="utf-8", newline="\n")


authority = Path(
    "crates/allow-report/src/artifacts/release_operation_authority_v1.rs"
)
artifacts = Path("crates/allow-report/src/artifacts.rs")
lib = Path("crates/allow-report/src/lib.rs")
contract = Path("crates/allow-report/tests/release_operation_authority_contract.rs")
tests = Path("crates/cargo-allow/tests/release_operation_authority.rs")
schema = Path("docs/schemas/cargo-allow.release-operation-authority.v1.schema.json")
docs = Path("docs/release/release-operation-authority-v1.md")
inventory = Path("policy/evidence-surface-inventory.toml")

# Add typed timestamp provenance before the event subject contract.
sub_once(
    authority,
    r'(?m)^(#\[derive\(Debug, Clone, PartialEq, Eq, Serialize, Deserialize\)\]\n'
    r'#\[serde\(tag = "kind", content = "id", rename_all = "snake_case"\)\]\n'
    r'pub enum CargoAllowReleaseOperationEventSubjectV1 \{)',
    dedent(
        '''\
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum CargoAllowReleaseOperationTimestampSourceV1 {
            WorkflowRuntime,
            ProviderMetadata,
            RepositoryMetadata,
        }

        \\1'''
    ),
)

# Bind timestamp provenance into event construction, retained events, and digests.
for struct_name in (
    "CargoAllowReleaseOperationEventInitV1",
    "CargoAllowReleaseOperationEventV1",
):
    sub_once(
        authority,
        rf'(?s)(pub struct {struct_name} \{{.*?\n\s+pub artifact_digest: Option<String>,\n)'
        r'(\s+pub observed_at_unix_seconds: u64,)',
        r'\1    pub timestamp_source: CargoAllowReleaseOperationTimestampSourceV1,\n\2',
    )

sub_once(
    authority,
    r"(?s)(struct EventDigestBody<'a> \{.*?\n\s+artifact_digest: &'a Option<String>,\n)"
    r'(\s+observed_at_unix_seconds: u64,)',
    r'\1    timestamp_source: CargoAllowReleaseOperationTimestampSourceV1,\n\2',
)
sub_once(
    authority,
    r'(\n\s+artifact_digest: &event\.artifact_digest,\n)(\s+observed_at_unix_seconds: event\.observed_at_unix_seconds,)',
    r'\1        timestamp_source: event.timestamp_source,\n\2',
)

# Validate provenance by event class.
marker = "fn validate_event_envelope_fields(\n"
helper = dedent(
    '''\
    fn timestamp_source_is_compatible(
        event_class: CargoAllowReleaseOperationEventClassV1,
        source: CargoAllowReleaseOperationTimestampSourceV1,
    ) -> bool {
        use CargoAllowReleaseOperationEventClassV1 as Event;
        use CargoAllowReleaseOperationTimestampSourceV1 as Source;

        match event_class {
            Event::TagObservedExact
            | Event::PackageRowObservedExact
            | Event::GitHubDraftObservedExact
            | Event::AssetObservedExact
            | Event::PublicReleaseObservedExact
            | Event::ContainmentObservedExact => source == Source::ProviderMetadata,
            Event::RepositoryReconciled => source == Source::RepositoryMetadata,
            _ => source == Source::WorkflowRuntime,
        }
    }

    '''
)
replace_once(authority, marker, helper + marker)
replace_once(
    authority,
    dedent(
        '''\
            if event.authority_class != identity.authority_kind {
                return Err("operation event authority class does not match the immutable operation");
            }
        '''
    ),
    dedent(
        '''\
            if event.authority_class != identity.authority_kind {
                return Err("operation event authority class does not match the immutable operation");
            }
            if !timestamp_source_is_compatible(event.event_class, event.timestamp_source) {
                return Err("operation event timestamp source does not match its event class");
            }
        '''
    ),
)

# Authorization selection must name the immutable selected authorization.
replace_once(
    authority,
    dedent(
        '''\
                Event::AuthorizationSelected => {
                    let ready = match identity.operation_class {
        '''
    ),
    dedent(
        '''\
                Event::AuthorizationSelected => {
                    if init.payload_digest != identity.authorization_digest {
                        return Err(
                            "authorization selection must bind the immutable authorization digest",
                        );
                    }
                    let ready = match identity.operation_class {
        '''
    ),
)

# Carry provenance through probe construction, retained construction, and reload validation.
sub_once(
    authority,
    r'(\n\s+artifact_digest: init\.artifact_digest\.clone\(\),\n)(\s+observed_at_unix_seconds: init\.observed_at_unix_seconds,)',
    r'\1        timestamp_source: init.timestamp_source,\n\2',
)
sub_once(
    authority,
    r'(\n\s+artifact_digest: init\.artifact_digest,\n)(\s+observed_at_unix_seconds: init\.observed_at_unix_seconds,)',
    r'\1        timestamp_source: init.timestamp_source,\n\2',
)
sub_once(
    authority,
    r'(\n\s+artifact_digest: event\.artifact_digest\.clone\(\),\n)(\s+observed_at_unix_seconds: event\.observed_at_unix_seconds,)',
    r'\1            timestamp_source: event.timestamp_source,\n\2',
)

# Publish the new enum from both artifact and crate-root surfaces.
for path in (artifacts, lib):
    sub_once(
        path,
        r'(CargoAllowReleaseOperationStateV1,)(\s+RELEASE_OPERATION_ASSET_SELECTION,)',
        r'\1 CargoAllowReleaseOperationTimestampSourceV1,\2',
    )

# Import it in both focused test contracts.
sub_once(
    contract,
    r'(CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,\n\s*)'
    r'(RELEASE_AUTHORIZATION_SELECTION,)',
    r'\1CargoAllowReleaseOperationTimestampSourceV1, \2',
)
sub_once(
    tests,
    r'(CargoAllowReleaseOperationStateV1,)(\s+RELEASE_AUTHORIZATION_SELECTION,)',
    r'\1 CargoAllowReleaseOperationTimestampSourceV1,\2',
)

# Focused helpers generate exact authorization identity and class-compatible timestamps.
for path, ordinal_expr in ((contract, "sequence_hint"), (tests, "ordinal")):
    sub_once(
        path,
        rf'(?m)^(\s*)payload_digest: digest\((?:1_?000) \+ {ordinal_expr}\),$',
        r'''\1payload_digest: if class
\1    == CargoAllowReleaseOperationEventClassV1::AuthorizationSelected
\1{
\1    identity.authorization_digest.clone()
\1} else {
\1    digest(1_000 + ''' + ordinal_expr + r''')
\1},''',
    )
    sub_once(
        path,
        rf'(?m)^(\s*)observed_at_unix_seconds: 1_790_000_000 \+ {ordinal_expr},$',
        r'''\1timestamp_source: match class {
\1    CargoAllowReleaseOperationEventClassV1::TagObservedExact
\1    | CargoAllowReleaseOperationEventClassV1::PackageRowObservedExact
\1    | CargoAllowReleaseOperationEventClassV1::GitHubDraftObservedExact
\1    | CargoAllowReleaseOperationEventClassV1::AssetObservedExact
\1    | CargoAllowReleaseOperationEventClassV1::PublicReleaseObservedExact
\1    | CargoAllowReleaseOperationEventClassV1::ContainmentObservedExact => {
\1        CargoAllowReleaseOperationTimestampSourceV1::ProviderMetadata
\1    }
\1    CargoAllowReleaseOperationEventClassV1::RepositoryReconciled => {
\1        CargoAllowReleaseOperationTimestampSourceV1::RepositoryMetadata
\1    }
\1    _ => CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime,
\1},
\1observed_at_unix_seconds: 1_790_000_000 + ''' + ordinal_expr + r''',''',
    )

# Hostile fixtures prove wrong authorization and forged timestamp provenance fail.
test_marker = dedent(
    '''\
    #[test]
    fn release_operation_request_correlation_is_phase_bound() -> Result<(), Box<dyn Error>> {
    '''
)
hostile_test = dedent(
    '''\
    #[test]
    fn release_operation_authorization_and_timestamp_provenance_are_bound(
    ) -> Result<(), Box<dyn Error>> {
        use CargoAllowReleaseOperationEventClassV1 as Event;
        use CargoAllowReleaseOperationEventSubjectV1::Operation;
        use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
        use CargoAllowReleaseOperationTimestampSourceV1 as Source;

        let identity = identity()?;
        let mut events = Vec::new();
        append(
            &identity,
            &mut events,
            Event::OperationSelected,
            Operation,
            1,
        )?;

        let mut wrong_authorization = event_init(
            &identity,
            Event::AuthorizationSelected,
            Operation,
            ResultClass::Exact,
            2,
        );
        wrong_authorization.payload_digest = digest(9_999);
        require(
            append_release_operation_event_v1(
                &identity,
                &events,
                wrong_authorization,
            )
            .is_err(),
            "authorization selection must bind the immutable authorization digest",
        )?;

        append(
            &identity,
            &mut events,
            Event::AuthorizationSelected,
            Operation,
            2,
        )?;
        let mut wrong_source = event_init(
            &identity,
            Event::LeaseAcquired,
            Operation,
            ResultClass::Exact,
            3,
        );
        wrong_source.timestamp_source = Source::ProviderMetadata;
        require(
            append_release_operation_event_v1(&identity, &events, wrong_source).is_err(),
            "a local transition must not claim provider timestamp provenance",
        )
    }

    '''
)
replace_once(tests, test_marker, hostile_test + test_marker)

# Schema retains the new required provenance field.
replace_once(
    schema,
    '    "state": {\n      "enum": [\n',
    '    "timestamp_source": {\n'
    '      "enum": ["workflow_runtime", "provider_metadata", "repository_metadata"]\n'
    '    },\n'
    '    "state": {\n      "enum": [\n',
)
replace_once(
    schema,
    '        "request_boundary", "response_posture", "semantic_result", "artifact_digest",\n'
    '        "observed_at_unix_seconds", "claim_boundary"\n',
    '        "request_boundary", "response_posture", "semantic_result", "artifact_digest",\n'
    '        "timestamp_source", "observed_at_unix_seconds", "claim_boundary"\n',
)
replace_once(
    schema,
    '        "artifact_digest": {\n'
    '          "oneOf": [{ "type": "null" }, { "$ref": "#/$defs/digest" }]\n'
    '        },\n'
    '        "observed_at_unix_seconds": { "type": "integer", "minimum": 1 },\n',
    '        "artifact_digest": {\n'
    '          "oneOf": [{ "type": "null" }, { "$ref": "#/$defs/digest" }]\n'
    '        },\n'
    '        "timestamp_source": { "$ref": "#/$defs/timestamp_source" },\n'
    '        "observed_at_unix_seconds": { "type": "integer", "minimum": 1 },\n',
)

# Keep operator documentation and evidence claims exact.
replace_once(
    docs,
    dedent(
        '''\
        Event time is monotonic and no event may be appended after operation expiry.
        Aggregate evaluation carries its own explicit evaluation timestamp; evaluation
        after expiry is Stale, so a historically complete chain cannot be reused as a
        current clean verdict.
        '''
    ),
    dedent(
        '''\
        Event time is monotonic and no event may be appended after operation expiry.
        Every event also retains whether its timestamp came from workflow runtime,
        provider metadata, or repository metadata. Local transitions require workflow
        runtime provenance; provider and repository observations reject incompatible
        provenance labels. Aggregate evaluation carries its own explicit evaluation
        timestamp; evaluation after expiry is Stale, so a historically complete chain
        cannot be reused as a current clean verdict.
        '''
    ),
)
replace_once(
    docs,
    dedent(
        '''\
        Loaded/deserialized histories are revalidated before append, head compilation,
        or evaluation. Validation rejects a foreign operation, skipped/duplicate
        sequence, changed previous digest, changed payload without a recomputed event
        digest, malformed producer/authority envelope, invalid subject, or invalid
        transition.
        '''
    ),
    dedent(
        '''\
        Loaded/deserialized histories are revalidated before append, head compilation,
        or evaluation. Validation rejects a foreign operation, skipped/duplicate
        sequence, changed previous digest, changed payload without a recomputed event
        digest, malformed producer/authority envelope, invalid subject, or invalid
        transition. AuthorizationSelected must reference the immutable operation
        authorization digest; another valid-looking authorization cannot unlock the
        operation.
        '''
    ),
)
replace_once(
    inventory,
    'claimed_acceptance_row = "exact subject identity, internally computed event chain, package/asset denominator byte binding, explicit irreversible-request correlation, predecessor-head proof for non-clean APIs, and externally observed containment-without-publication law"\n',
    'claimed_acceptance_row = "exact subject identity, internally computed event chain, authorization-digest and timestamp-provenance binding, package/asset denominator byte binding, phase-bound irreversible-request correlation, predecessor-head proof for non-clean APIs, and externally observed containment-without-publication law"\n',
)
