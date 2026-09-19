from __future__ import annotations

from pathlib import Path
from textwrap import dedent


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one occurrence, found {count}: {old[:120]!r}"
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

replace_once(
    authority,
    dedent(
        '''\
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "kind", content = "id", rename_all = "snake_case")]
        pub enum CargoAllowReleaseOperationEventSubjectV1 {
        '''
    ),
    dedent(
        '''\
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum CargoAllowReleaseOperationTimestampSourceV1 {
            WorkflowRuntime,
            ProviderMetadata,
            RepositoryMetadata,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "kind", content = "id", rename_all = "snake_case")]
        pub enum CargoAllowReleaseOperationEventSubjectV1 {
        '''
    ),
)

replace_once(
    authority,
    dedent(
        '''\
            pub artifact_digest: Option<String>,
            pub observed_at_unix_seconds: u64,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        '''
    ),
    dedent(
        '''\
            pub artifact_digest: Option<String>,
            pub timestamp_source: CargoAllowReleaseOperationTimestampSourceV1,
            pub observed_at_unix_seconds: u64,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        '''
    ),
)

replace_once(
    authority,
    dedent(
        '''\
            pub artifact_digest: Option<String>,
            pub observed_at_unix_seconds: u64,
            pub claim_boundary: String,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        '''
    ),
    dedent(
        '''\
            pub artifact_digest: Option<String>,
            pub timestamp_source: CargoAllowReleaseOperationTimestampSourceV1,
            pub observed_at_unix_seconds: u64,
            pub claim_boundary: String,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        '''
    ),
)

replace_once(
    authority,
    dedent(
        '''\
            artifact_digest: &'a Option<String>,
            observed_at_unix_seconds: u64,
            claim_boundary: &'a str,
        }
        '''
    ),
    dedent(
        '''\
            artifact_digest: &'a Option<String>,
            timestamp_source: CargoAllowReleaseOperationTimestampSourceV1,
            observed_at_unix_seconds: u64,
            claim_boundary: &'a str,
        }
        '''
    ),
)

replace_once(
    authority,
    dedent(
        '''\
                artifact_digest: &event.artifact_digest,
                observed_at_unix_seconds: event.observed_at_unix_seconds,
                claim_boundary: &event.claim_boundary,
        '''
    ),
    dedent(
        '''\
                artifact_digest: &event.artifact_digest,
                timestamp_source: event.timestamp_source,
                observed_at_unix_seconds: event.observed_at_unix_seconds,
                claim_boundary: &event.claim_boundary,
        '''
    ),
)

marker = dedent(
    '''\
    fn validate_event_envelope_fields(
        identity: &CargoAllowReleaseOperationIdentityV1,
        event: &CargoAllowReleaseOperationEventV1,
    ) -> Result<(), &'static str> {
    '''
)
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

replace_once(
    authority,
    dedent(
        '''\
                artifact_digest: init.artifact_digest.clone(),
                observed_at_unix_seconds: init.observed_at_unix_seconds,
                claim_boundary: CLAIM_BOUNDARY.to_string(),
        '''
    ),
    dedent(
        '''\
                artifact_digest: init.artifact_digest.clone(),
                timestamp_source: init.timestamp_source,
                observed_at_unix_seconds: init.observed_at_unix_seconds,
                claim_boundary: CLAIM_BOUNDARY.to_string(),
        '''
    ),
)

replace_once(
    authority,
    dedent(
        '''\
                artifact_digest: init.artifact_digest,
                observed_at_unix_seconds: init.observed_at_unix_seconds,
                claim_boundary: CLAIM_BOUNDARY.to_string(),
        '''
    ),
    dedent(
        '''\
                artifact_digest: init.artifact_digest,
                timestamp_source: init.timestamp_source,
                observed_at_unix_seconds: init.observed_at_unix_seconds,
                claim_boundary: CLAIM_BOUNDARY.to_string(),
        '''
    ),
)

replace_once(
    authority,
    dedent(
        '''\
                    artifact_digest: event.artifact_digest.clone(),
                    observed_at_unix_seconds: event.observed_at_unix_seconds,
                };
        '''
    ),
    dedent(
        '''\
                    artifact_digest: event.artifact_digest.clone(),
                    timestamp_source: event.timestamp_source,
                    observed_at_unix_seconds: event.observed_at_unix_seconds,
                };
        '''
    ),
)

for path in (artifacts, lib):
    replace_once(
        path,
        dedent(
            '''\
                CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
                CargoAllowReleaseOperationStateV1, RELEASE_OPERATION_ASSET_SELECTION,
        '''
        ),
        dedent(
            '''\
                CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
                CargoAllowReleaseOperationStateV1, CargoAllowReleaseOperationTimestampSourceV1,
                RELEASE_OPERATION_ASSET_SELECTION,
        '''
        ),
    )

replace_once(
    contract,
    dedent(
        '''\
            CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
            RELEASE_AUTHORIZATION_SELECTION, RELEASE_OPERATION_ASSET_SELECTION,
        '''
    ),
    dedent(
        '''\
            CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
            CargoAllowReleaseOperationTimestampSourceV1, RELEASE_AUTHORIZATION_SELECTION,
            RELEASE_OPERATION_ASSET_SELECTION,
        '''
    ),
)

replace_once(
    tests,
    dedent(
        '''\
            CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
            CargoAllowReleaseOperationStateV1, RELEASE_AUTHORIZATION_SELECTION,
        '''
    ),
    dedent(
        '''\
            CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
            CargoAllowReleaseOperationStateV1, CargoAllowReleaseOperationTimestampSourceV1,
            RELEASE_AUTHORIZATION_SELECTION,
        '''
    ),
)

replace_once(
    contract,
    "        payload_digest: digest(1000 + sequence_hint),\n",
    dedent(
        '''\
                payload_digest: if class
                    == CargoAllowReleaseOperationEventClassV1::AuthorizationSelected
                {
                    identity.authorization_digest.clone()
                } else {
                    digest(1000 + sequence_hint)
                },
        '''
    ),
)
replace_once(
    contract,
    dedent(
        '''\
                artifact_digest: Some(digest(2000 + sequence_hint)),
                observed_at_unix_seconds: 1_790_000_000 + sequence_hint,
        '''
    ),
    dedent(
        '''\
                artifact_digest: Some(digest(2000 + sequence_hint)),
                timestamp_source: match class {
                    CargoAllowReleaseOperationEventClassV1::TagObservedExact
                    | CargoAllowReleaseOperationEventClassV1::PackageRowObservedExact
                    | CargoAllowReleaseOperationEventClassV1::GitHubDraftObservedExact
                    | CargoAllowReleaseOperationEventClassV1::AssetObservedExact
                    | CargoAllowReleaseOperationEventClassV1::PublicReleaseObservedExact
                    | CargoAllowReleaseOperationEventClassV1::ContainmentObservedExact => {
                        CargoAllowReleaseOperationTimestampSourceV1::ProviderMetadata
                    }
                    CargoAllowReleaseOperationEventClassV1::RepositoryReconciled => {
                        CargoAllowReleaseOperationTimestampSourceV1::RepositoryMetadata
                    }
                    _ => CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime,
                },
                observed_at_unix_seconds: 1_790_000_000 + sequence_hint,
        '''
    ),
)

replace_once(
    tests,
    "        payload_digest: digest(1_000 + ordinal),\n",
    dedent(
        '''\
                payload_digest: if class
                    == CargoAllowReleaseOperationEventClassV1::AuthorizationSelected
                {
                    identity.authorization_digest.clone()
                } else {
                    digest(1_000 + ordinal)
                },
        '''
    ),
)
replace_once(
    tests,
    dedent(
        '''\
                artifact_digest,
                observed_at_unix_seconds: 1_790_000_000 + ordinal,
        '''
    ),
    dedent(
        '''\
                artifact_digest,
                timestamp_source: match class {
                    CargoAllowReleaseOperationEventClassV1::TagObservedExact
                    | CargoAllowReleaseOperationEventClassV1::PackageRowObservedExact
                    | CargoAllowReleaseOperationEventClassV1::GitHubDraftObservedExact
                    | CargoAllowReleaseOperationEventClassV1::AssetObservedExact
                    | CargoAllowReleaseOperationEventClassV1::PublicReleaseObservedExact
                    | CargoAllowReleaseOperationEventClassV1::ContainmentObservedExact => {
                        CargoAllowReleaseOperationTimestampSourceV1::ProviderMetadata
                    }
                    CargoAllowReleaseOperationEventClassV1::RepositoryReconciled => {
                        CargoAllowReleaseOperationTimestampSourceV1::RepositoryMetadata
                    }
                    _ => CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime,
                },
                observed_at_unix_seconds: 1_790_000_000 + ordinal,
        '''
    ),
)

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

replace_once(
    schema,
    dedent(
        '''\
            "state": {
              "enum": [
        '''
    ),
    dedent(
        '''\
            "timestamp_source": {
              "enum": ["workflow_runtime", "provider_metadata", "repository_metadata"]
            },
            "state": {
              "enum": [
        '''
    ),
)
replace_once(
    schema,
    dedent(
        '''\
                "request_boundary", "response_posture", "semantic_result", "artifact_digest",
                "observed_at_unix_seconds", "claim_boundary"
        '''
    ),
    dedent(
        '''\
                "request_boundary", "response_posture", "semantic_result", "artifact_digest",
                "timestamp_source", "observed_at_unix_seconds", "claim_boundary"
        '''
    ),
)
replace_once(
    schema,
    dedent(
        '''\
                "artifact_digest": {
                  "oneOf": [{ "type": "null" }, { "$ref": "#/$defs/digest" }]
                },
                "observed_at_unix_seconds": { "type": "integer", "minimum": 1 },
        '''
    ),
    dedent(
        '''\
                "artifact_digest": {
                  "oneOf": [{ "type": "null" }, { "$ref": "#/$defs/digest" }]
                },
                "timestamp_source": { "$ref": "#/$defs/timestamp_source" },
                "observed_at_unix_seconds": { "type": "integer", "minimum": 1 },
        '''
    ),
)

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
