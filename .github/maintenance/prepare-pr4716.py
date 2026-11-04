import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

HEAD = '8a2090d946392d47e41e0ce7e5907770a92efa3d'
MAIN = 'ba6f8ab086edc0217ae241ec5a9f24fb1b6f7198'
BASE = '601d773a59ad249cedd897287f15ddb02ebc66b5'
EXPECTED = {
    '.allow/revisions/4716-review-disposition.toml',
    'crates/cargo-allow/src/cli.rs',
    'crates/cargo-allow/src/init.rs',
    'crates/cargo-allow/src/core_command_router.rs',
    'crates/cargo-allow/tests/core_command_summary_router_cli.rs',
    'crates/cargo-allow/tests/init_summary_backup_cli.rs',
    'policy/allow.toml',
}

def git(*args, check=True, data=None):
    result = subprocess.run(['git', '-c', 'core.autocrlf=false', *args], input=data,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if check and result.returncode:
        raise RuntimeError(f'git {args[0]} failed: {result.stderr.decode(errors="replace")}')
    return result

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def records(raw):
    doc = tomllib.loads(raw.decode('utf-8'))
    rows = doc['allow']
    result = {row['id']: row for row in rows}
    require(len(result) == len(rows), 'duplicate policy ID')
    return doc, result

def reconcile(base, head, main):
    normalize = lambda value: value.replace(b'\r\n', b'\n')
    require(normalize(head).startswith(normalize(base)), 'head is not an append-only policy delta')
    base_doc, base_rows = records(base)
    head_doc, head_rows = records(head)
    main_doc, main_rows = records(main)
    for key, value in base_rows.items():
        require(head_rows.get(key) == value, f'head changed existing record {key}')
        require(main_rows.get(key) == value, f'main changed existing record {key}')
    added = set(head_rows) - set(base_rows)
    require(added == {'allow-4716-review-disposition', 'allow-4716-review-revision'},
            'unexpected head policy additions')
    require(not added.intersection(main_rows), 'policy ID collision')
    require({k: v for k, v in head_doc.items() if k != 'allow'} ==
            {k: v for k, v in base_doc.items() if k != 'allow'}, 'head changed policy settings')
    suffix = normalize(head)[len(normalize(base)):]
    merged = main + (b'' if main.endswith(b'\n') else b'\n') + suffix
    merged_doc, merged_rows = records(merged)
    require(merged.startswith(main), 'main raw prefix lost')
    require(merged_rows == dict(main_rows, **{key: head_rows[key] for key in added}),
            'merged policy differs semantically')
    require({k: v for k, v in merged_doc.items() if k != 'allow'} ==
            {k: v for k, v in main_doc.items() if k != 'allow'}, 'main settings lost')
    return merged

def prepare():
    require(git('rev-parse', 'HEAD').stdout.decode().strip() == HEAD, 'wrong checkout')
    git('fetch', '--no-tags', 'origin', MAIN)
    require(git('merge-base', '--all', HEAD, MAIN).stdout.decode().strip() == BASE,
            'unexpected merge base')
    changed = set(git('diff', '--name-only', BASE, HEAD).stdout.decode().splitlines())
    require(changed == EXPECTED, 'unexpected PR file set')
    main_changes = set(git('diff', '--name-only', BASE, MAIN).stdout.decode().splitlines())
    require(changed.intersection(main_changes) == {'policy/allow.toml'}, 'unexpected overlapping source')
    original = {ref: git('show', f'{ref}:policy/allow.toml').stdout for ref in [BASE, HEAD, MAIN]}
    policy = reconcile(original[BASE], original[HEAD], original[MAIN])
    git('config', 'user.name', 'cargo-allow integration worker')
    git('config', 'user.email', 'integration-worker@users.noreply.github.com')
    outcome = git('merge', '--no-ff', '--no-commit', MAIN, check=False)
    conflicts = set(git('diff', '--name-only', '--diff-filter=U').stdout.decode().splitlines())
    require(outcome.returncode in (0, 1) and conflicts <= {'policy/allow.toml'}, 'unexpected merge failure')
    path = Path('policy/allow.toml')
    path.write_bytes(policy)
    blob = git('hash-object', '-w', '--no-filters', str(path)).stdout.decode().strip()
    git('update-index', '--add', '--cacheinfo', f'100644,{blob},{path.as_posix()}')
    for name in EXPECTED - {'policy/allow.toml'}:
        require(git('rev-parse', f':{name}').stdout == git('rev-parse', f'{HEAD}:{name}').stdout,
                f'branch file changed during integration: {name}')
    require(set(git('diff', '--cached', '--name-only', MAIN).stdout.decode().splitlines()) == EXPECTED,
            'integration changed unrelated main paths')
    Path(os.environ['RUNNER_TEMP'], 'integration-evidence').mkdir(exist_ok=True)
    report = {'head': HEAD, 'main': MAIN, 'merge_base': BASE, 'expected_paths': sorted(EXPECTED),
              'main_policy_sha256': hashlib.sha256(original[MAIN]).hexdigest(),
              'integrated_policy_sha256': hashlib.sha256(policy).hexdigest(),
              'main_policy_prefix_preserved': True, 'unrelated_main_paths_preserved': True}
    Path(os.environ['RUNNER_TEMP'], 'integration-evidence', 'source.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))

if __name__ == '__main__':
    prepare()
