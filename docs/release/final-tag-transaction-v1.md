# Final tag transaction v1

Issue [#3930](https://github.com/EffortlessMetrics/cargo-allow/issues/3930)
owns the exactly-once annotated tag transaction for the final release, under
#2502/#3760/#3927. The [schema](../schemas/cargo-allow.final-tag-transaction.v1.schema.json),
public `allow-report` model (`final_tag_transaction_v1`), and canonical JSON
renderer give the first irreversible final-release action one exact,
crash-consistent owner. They do not create or push the tag, read credentials,
upload packages, or execute publication.

## Transaction

```text
remote preflight (reachable + absent) + held lease + selected authorization
        ▼
begin (CreatedLocally, bound to the custody commit/tree)
        ▼
record intent (journal head + checkpoint agree) ──► PushIntentDurable
        ▼
start (bounded attempts) ──► PushStarted
        ▼
response observed → PushResponseObserved ── or ──► PushResponseUnknown
        ▼ read-only reconcile
exact ──► RemoteObservedExact (gate opens, no second push)
conflict ──► RemoteConflict (incident, stops; never deleted/replaced)
absent ──► PushIntentDurable (one more careful attempt, within bound)
unreachable ──► ProviderUnavailable (never inferred absent)
```

- The remote ref and tag object are observed **before** local creation. A
  pre-existing lightweight tag, moved tag, wrong object, or wrong peeled
  commit/tree stops the transaction; resume reconciles, never overwrites.
- The tag must peel to the exact custody subject: construction binds the
  custody commit/tree supplied from the #2501 custody record, so a tag built
  from moving `main` cannot validate.
- The push begins only after the journal head and the independently
  read-back checkpoint bind the same push intent; disagreement refuses.
- A lost response is recorded unknown and reconciled by observation. Exact
  observation continues without another push; blind retry is refused; the
  attempt bound sends exhaustion to operator decision.
- Once the remote tag is observed it is immutable for this version. A clean
  authorization is never reused to repair a conflicting or moved tag; any
  package-byte-changing repair needs a new version, freeze, authorization,
  and tag.

## Release gate

`tag_release_gate_open_v1` is true in exactly one state:
`RemoteObservedExact`. Package and token work in #2502 remain unreachable
until then. A process exit, local ref, push stdout, or tag-shaped remote ref
is insufficient by itself.

## Dry-run proof (synthetic only)

```sh
cargo test -p cargo-allow final_tag_transaction --locked -- --nocapture
cargo test -p cargo-allow final_tag_transaction_unknown_response --locked -- --nocapture
cargo test -p cargo-allow release_tag_immutability --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

Fixtures are synthetic and side-effect-free: no git invocation, no network
access, no tag creation, and no live-state mutation. The module performs no
environment, filesystem, tag, or upload operations.

## Consumers and proof

- #2502 maps the custody (#3927) and lease (#3925) records onto the
  transaction's digest and state attestations, performs the local
  construction and the push, and supplies journal/checkpoint/remote
  observations.
- #3921/#3922 own the journal and remote-checkpoint stores the transaction
  binds.
- Recovery and incident records consume the same transaction identity;
  conflicting or moved tags stop as incidents under #2509.

The evidence inventory retains the tag transaction as typed model
validation. The transaction owns the first irreversible action's
consistency; it does not perform it.
