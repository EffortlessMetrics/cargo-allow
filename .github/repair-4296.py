#!/usr/bin/env python3
"""Exact-source, bounded custody repair driver; no credentials or network."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import sys

SOURCE = '0025b0bcb10f2ed025f435bb540e1aef530962cd'
MODULE = Path('crates/allow-report/src/artifacts/release_authorization_custody_v1.rs')
TEST = Path('crates/cargo-allow/tests/release_authorization_custody.rs')
DOC = Path('docs/release/release-authorization-custody-v1.md')
NOTE = Path('.changes/Added-20260917-release-authorization-custody.yaml')
EXPECTED = {
    str(MODULE): '7956da5f76b36faab0b80fcc3569fc3d329dec5c',
    str(TEST): '6624f9be176fe3b54e5d048314154d18a23ebc13',
    str(DOC): '6c27289173596d033a68393f20784f9c1d94cdce',
    str(NOTE): 'fcbde5351bf2551d07469f4134276c21c697eca5',
}
OUT = Path('target/custody-repair-4296')
RED_NAMES = [
    'custody_review_expired_selection_cannot_start',
    'custody_review_start_cannot_precede_selection',
    'custody_review_failed_readback_invalidates_selection',
    'custody_review_repeated_readback_is_idempotent',
    'custody_review_mint_rejects_foreign_freeze',
    'custody_review_nonce_secret_marker_is_rejected',
    'custody_review_revocation_secret_marker_is_rejected',
    'custody_review_failed_readback_blocks_start',
    'custody_review_selection_cannot_precede_mint',
]
POSITIVE = 'custody_review_completion_after_expiry_retains_history'

TESTS = r'''

#[test]
fn custody_review_expired_selection_cannot_start() -> Result<(), Box<dyn Error>> {
    let mut record = selected()?;
    let nonce_history = record.consumed_nonces.clone();
    require(
        note_irreversible_start_v1(&mut record, EXPIRES_AT + 1).is_err(),
        "expired selected authority must not start irreversible work",
    )?;
    require(
        record.state == Consumption::Expired
            && record.transitions.len() == 2
            && record.consumed_nonces == nonce_history,
        "expiry must be recorded without erasing selection or nonce history",
    )
}

#[test]
fn custody_review_start_cannot_precede_selection() -> Result<(), Box<dyn Error>> {
    let mut record = selected()?;
    let before = record.clone();
    require(
        note_irreversible_start_v1(&mut record, SELECT_AT - 1).is_err(),
        "an irreversible event cannot be backdated before selection",
    )?;
    require(record == before, "rejected time must not mutate custody history")
}

#[test]
fn custody_review_failed_readback_invalidates_selection() -> Result<(), Box<dyn Error>> {
    for malformed in [false, true] {
        let mut record = minted()?;
        let stored = render_release_authorization_custody_v1(&record)?;
        require(
            note_custody_readback_v1(&mut record, stored.as_bytes()) == CustodyReadbackV1::Match,
            "the initial readback must match",
        )?;
        let bad = if malformed {
            "not JSON".to_string()
        } else {
            stored.replace("auth-0-2-0-001", "auth-0-2-0-999")
        };
        let expected = if malformed { CustodyReadbackV1::Malformed } else { CustodyReadbackV1::Mismatch };
        require(note_custody_readback_v1(&mut record, bad.as_bytes()) == expected,
            "a failed observation must preserve its own result class")?;
        let evidence = record.evidence_digest.clone();
        require(!record.readback_verified && record.readback_digest.is_none(),
            "failed readback must invalidate earlier admission")?;
        require(select_authorization_for_run_v1(&mut record, NONCE, SELECT_AT, &evidence, true).is_err(),
            "earlier success cannot authorize selection after failed readback")?;
        require(record.state == Consumption::Available && record.transitions.is_empty(),
            "readback failure must not fabricate a consumption transition")?;
    }
    Ok(())
}

#[test]
fn custody_review_repeated_readback_is_idempotent() -> Result<(), Box<dyn Error>> {
    let mut record = minted()?;
    let stored = render_release_authorization_custody_v1(&record)?;
    require(note_custody_readback_v1(&mut record, stored.as_bytes()) == CustodyReadbackV1::Match,
        "first observation must match")?;
    let before = record.clone();
    require(note_custody_readback_v1(&mut record, stored.as_bytes()) == CustodyReadbackV1::Match,
        "observation metadata must not change the expected stored custody content")?;
    require(record == before, "identical successful readback must be idempotent")
}

#[test]
fn custody_review_mint_rejects_foreign_freeze() -> Result<(), Box<dyn Error>> {
    let mut init = mint_init(decision()?);
    init.freeze_receipt_digest = digest(987);
    require(mint_authorization_custody_v1(init).is_err(),
        "digest-shaped evidence for a different freeze must not mint custody")
}

#[test]
fn custody_review_nonce_secret_marker_is_rejected() -> Result<(), Box<dyn Error>> {
    let mut document = decision()?;
    document.authority.nonce = "ghp_synthetic_fixture_not_a_credential".to_string();
    require(mint_authorization_custody_v1(mint_init(document)).is_err(),
        "a copied nonce must pass the same secret-marker screen as other operator text")
}

#[test]
fn custody_review_revocation_secret_marker_is_rejected() -> Result<(), Box<dyn Error>> {
    let mut record = minted()?;
    let before = record.clone();
    require(revoke_authorization_custody_v1(&mut record, "token=synthetic_fixture", SELECT_AT).is_err(),
        "revocation must refuse secret markers before storing the reason")?;
    require(record == before, "rejected reason must not enter history")
}

#[test]
fn custody_review_failed_readback_blocks_start() -> Result<(), Box<dyn Error>> {
    let mut record = selected()?;
    require(note_custody_readback_v1(&mut record, b"not JSON") == CustodyReadbackV1::Malformed,
        "failed storage observation must be classified")?;
    let before = record.clone();
    require(note_irreversible_start_v1(&mut record, SELECT_AT + 1).is_err(),
        "a newly failed readback must block start as well as selection")?;
    require(record == before, "rejected start must preserve selected history")
}

#[test]
fn custody_review_selection_cannot_precede_mint() -> Result<(), Box<dyn Error>> {
    let mut record = minted()?;
    let stored = render_release_authorization_custody_v1(&record)?;
    require(note_custody_readback_v1(&mut record, stored.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic observation must match")?;
    let before = record.clone();
    let evidence = record.evidence_digest.clone();
    require(select_authorization_for_run_v1(&mut record, NONCE, MINTED_AT - 1, &evidence, true).is_err(),
        "valid-from alone cannot authorize selection before the record was minted")?;
    require(record == before, "rejected backdated selection must not mutate custody")
}

#[test]
fn custody_review_completion_after_expiry_retains_history() -> Result<(), Box<dyn Error>> {
    let mut record = selected()?;
    note_irreversible_start_v1(&mut record, SELECT_AT + 1).map_err(io::Error::other)?;
    let observation = settle_authorization_consumption_v1(&mut record, true, EXPIRES_AT + 1)
        .map_err(io::Error::other)?;
    require(record.state == Consumption::ConsumedComplete
        && observation.from == Consumption::IrreversibleOperationStarted
        && record.transitions.len() == 3,
        "expiry must not prevent recording the outcome of already-started work")
}
'''


def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f'exact patch anchor has {text.count(old)} occurrences: {old[:100]!r}')
    return text.replace(old, new, 1)


def write(path, text):
    path.write_text(text, encoding='utf-8', newline='\n')


def setup():
    if git('rev-parse', 'HEAD') != SOURCE or git('status', '--porcelain', '--untracked-files=all'):
        raise ValueError('expected the clean exact author head')
    for path, expected in EXPECTED.items():
        if git('hash-object', '--no-filters', path) != expected:
            raise ValueError(f'wrong source bytes: {path}')
    text = TEST.read_text(encoding='utf-8')
    text = replace_once(text,
        '        decision: document,\n        freeze_receipt_digest: digest(50),',
        '        freeze_receipt_digest: document.freeze.receipt_digest.clone(),\n        decision: document,')
    text = replace_once(text, '''    if let Ok(secret) = std::env::var("CARGO_REGISTRY_TOKEN") {
        require(
            secret.is_empty() || !rendered.contains(&secret),
            "custody artifact must be independent of any ambient registry token",
        )?;
    }
''', '')
    if 'std::env::var' in text:
        raise ValueError('fixture must not inspect ambient credential values')
    write(TEST, text + TESTS)
    OUT.mkdir(parents=True, exist_ok=True)


def verify_red():
    text = (OUT / 'red.log').read_text(encoding='utf-8')
    for name in RED_NAMES:
        if not re.search(rf'^test {name} \.\.\. FAILED$', text, re.MULTILINE):
            raise ValueError(f'expected named red failure absent: {name}')
    if not re.search(rf'^test {POSITIVE} \.\.\. ok$', text, re.MULTILINE):
        raise ValueError('positive historical settlement control did not pass')
    if not re.search(r'test result: FAILED\. 1 passed; 9 failed; 0 ignored;', text):
        raise ValueError('unexpected red denominator')
    write(OUT / 'red-control.json', json.dumps({'result': 'expected_failures_observed', 'failed': RED_NAMES,
        'positive': POSITIVE, 'source': SOURCE}, indent=2) + '\n')


def repair():
    text = MODULE.read_text(encoding='utf-8')
    text = replace_once(text, '    validate_mint_freeze(&init.decision.freeze)?;\n', '''    validate_mint_freeze(&init.decision.freeze)?;
    if !init.freeze_receipt_digest.eq_ignore_ascii_case(&init.decision.freeze.receipt_digest) {
        return Err("mint freeze receipt must match the authorized freeze");
    }
''')
    text = replace_once(text, '        init.minted_by.as_str(),\n',
        '        init.minted_by.as_str(),\n        init.decision.authority.nonce.as_str(),\n')
    text = replace_once(text, '    let authorization_digest = authorization_statement_digest(&init.decision)\n', '''    let freeze_text = serde_json::to_string(&init.decision.freeze)
        .map_err(|_| "freeze text validation failed")?;
    if secret_marker(&freeze_text).is_some() {
        return Err("secret material must never enter authorization custody records");
    }
    let authorization_digest = authorization_statement_digest(&init.decision)
''')
    start = text.index('/// Verify independently read-back storage bytes against the minted record.')
    end = text.index('/// Record a successful independent readback on the custody record.', start)
    text = text[:start] + '''/// Verify stored custody content independently of local readback metadata.
/// All identity, state, and history fields must match. Only the two local
/// observation fields are excluded to make repeated readback idempotent.
/// The observation still retains a digest of the exact transport bytes.
pub fn verify_custody_readback_v1(
    record: &CargoAllowReleaseAuthorizationCustodyV1,
    readback_json: &[u8],
) -> CustodyReadbackV1 {
    let mut observed: CargoAllowReleaseAuthorizationCustodyV1 =
        match serde_json::from_slice(readback_json) {
            Ok(value) => value,
            Err(_) => return CustodyReadbackV1::Malformed,
        };
    let mut expected = record.clone();
    for value in [&mut observed, &mut expected] {
        value.readback_verified = false;
        value.readback_digest = None;
    }
    if observed == expected {
        CustodyReadbackV1::Match
    } else {
        CustodyReadbackV1::Mismatch
    }
}

''' + text[end:]
    text = replace_once(text, '/// Record a successful independent readback on the custody record.',
        '/// Retain successful readback metadata or invalidate admission on failure.')
    text = replace_once(text, '        verdict => verdict,', '''        verdict => {
            record.readback_verified = false;
            record.readback_digest = None;
            verdict
        }''')
    text = replace_once(text, '    let checked = transition_authorization_consumption(record.state, next)?;', '''    let previous_time = record.transitions.last().map_or(
        record.mint.minted_at_unix_seconds,
        |transition| transition.at_unix_seconds,
    );
    if at_unix_seconds < previous_time {
        return Err("custody transitions cannot precede minting or the previous transition");
    }
    let checked = transition_authorization_consumption(record.state, next)?;''')
    text = replace_once(text, '''    if record.state != Consumption::SelectedForRun {
        return Err("only a selected authorization can start the irreversible operation");
    }
    advance_custody_state(''', '''    if record.state != Consumption::SelectedForRun {
        return Err("only a selected authorization can start the irreversible operation");
    }
    if now_unix_seconds > record.expires_at_unix_seconds {
        advance_custody_state(record, Consumption::Expired, now_unix_seconds, "expired-before-start")?;
        return Err("selected authorization expired before irreversible start");
    }
    if now_unix_seconds < record.valid_from_unix_seconds {
        return Err("authorization is not yet valid");
    }
    if !record.readback_verified || record.readback_digest.as_deref().is_none_or(|value| !digest_shape(value)) {
        return Err("irreversible start requires a current successful readback");
    }
    advance_custody_state(''')
    text = replace_once(text, '''    if reason.trim().is_empty() {
        return Err("revocation requires a reason");
    }
    advance_custody_state''', '''    if reason.trim().is_empty() {
        return Err("revocation requires a reason");
    }
    if secret_marker(reason).is_some() {
        return Err("secret material must never enter authorization custody records");
    }
    advance_custody_state''')
    text = text.replace('/// identity and digests only; the type cannot carry secret material because no\n/// such field exists.',
        '/// identity and digests only. Mint and transition validation reject known\n/// secret markers; field names alone are not a redaction guarantee.')
    text = text.replace('/// digests only; the shape cannot carry secret material.',
        '/// digests only; validated construction screens known secret markers.')
    write(MODULE, text)
    text = DOC.read_text(encoding='utf-8')
    text = replace_once(text, 'The constructor refuses unless **all** of these hold:',
        'The constructor refuses unless **all** of these hold:\n\n- the mint freeze-receipt digest matches the immutable decision’s freeze-receipt digest;')
    text = replace_once(text, 'The payload type has no secret\nfields because the protocol has no secret fields.',
        'Copied nonce/freeze text and revocation reasons are screened for known secret\nmarkers; identity-only field names are not proof of arbitrary-secret redaction.')
    text = replace_once(text, '   scanned for secret markers and for any ambient registry token value;',
        '   scanned using synthetic marker inputs, without reading any ambient token;')
    text = replace_once(text, '    output is proven independent of any ambient `CARGO_REGISTRY_TOKEN`.',
        '    output checks never read `CARGO_REGISTRY_TOKEN` or any real credential.')
    text += '''
## Transition and readback regression bounds

A selected authorization is checked again at irreversible start. Expiry is
recorded without clearing the selected nonce/history; backdated transitions
are refused before mutation. An operation already started while valid may
still record its final outcome after the authorization expires.

Readback comparison covers identity, state, and history, excluding only local
`readback_verified` and `readback_digest` metadata. Re-observing identical stored
bytes is idempotent. A failed observation invalidates earlier admission for
both selection and start; its return value retains Mismatch versus Malformed.
The successful transport-byte digest is not a distributed lock or a signature.

These are pure protocol checks. Authoritative provider readback, durable CAS/
lease integration, conservative file-location handling, and consumption schema
completion remain separate review obligations. This repair does not authenticate
an operator, prove distributed exactly-once behavior, or authorize publication.
'''
    write(DOC, text)
    write(NOTE, NOTE.read_text(encoding='utf-8') + '  Reject mismatched freeze links, stale readback admission, expired or\n  backdated starts, and known secret markers using credential-free regressions.\n')


def retain():
    changed = set(git('diff', '--name-only').splitlines())
    if changed != set(EXPECTED) or git('ls-files', '--others', '--exclude-standard'):
        raise ValueError(f'unexpected changes: {sorted(changed)}')
    if git('rev-parse', 'HEAD') != SOURCE:
        raise ValueError('source moved')
    green = (OUT / 'green.log').read_text(encoding='utf-8')
    for name in RED_NAMES + [POSITIVE]:
        if not re.search(rf'^test {name} \.\.\. ok$', green, re.MULTILINE):
            raise ValueError(f'green control absent: {name}')
    if 'std::env::var' in TEST.read_text(encoding='utf-8'):
        raise ValueError('ambient test credential probe remains')
    rows = []
    for name in sorted(changed):
        raw = Path(name).read_bytes()
        destination = OUT / 'files' / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(raw)
        rows.append({'path': name, 'size': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
            'git_blob': hashlib.sha1(f'blob {len(raw)}\0'.encode() + raw).hexdigest()})
    (OUT / 'repair.patch').write_bytes(subprocess.check_output(['git', 'diff', '--binary']))
    write(OUT / 'qualification.json', json.dumps({'source': SOURCE, 'source_tree': git('rev-parse','HEAD^{tree}'),
        'result': 'passed', 'red_controls': RED_NAMES, 'positive_control': POSITIVE,
        'files': rows, 'claim_boundary': 'Focused model regressions only; no authorization, publication, token read or tag operation.'}, indent=2) + '\n')


if __name__ == '__main__':
    modes = {'setup': setup, 'verify-red': verify_red, 'repair': repair, 'retain': retain}
    if len(sys.argv) != 2 or sys.argv[1] not in modes:
        raise SystemExit('expected setup|verify-red|repair|retain')
    modes[sys.argv[1]]()
