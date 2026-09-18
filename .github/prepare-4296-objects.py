#!/usr/bin/env python3
"""Prepare detached repair objects only; never update a branch or tag."""
import base64
import hashlib
import json
import os
from pathlib import Path
import urllib.error
import urllib.request

SOURCE = '0025b0bcb10f2ed025f435bb540e1aef530962cd'
API = 'https://api.github.com/repos/EffortlessMetrics/cargo-allow'
PATHS = {
    'crates/allow-report/src/artifacts/release_authorization_custody_v1.rs',
    'crates/cargo-allow/tests/release_authorization_custody.rs',
    'docs/release/release-authorization-custody-v1.md',
    '.changes/Added-20260917-release-authorization-custody.yaml',
}


def request(method, route, value=None):
    # Only inert Git objects and readback. There is deliberately no refs endpoint.
    allowed = {'/pulls/4296', f'/git/commits/{SOURCE}', '/git/blobs', '/git/trees', '/git/commits'}
    if route not in allowed or method not in {'GET', 'POST'}:
        raise ValueError('unsupported object operation')
    data = None if value is None else json.dumps(value).encode()
    req = urllib.request.Request(API + route, data=data, method=method, headers={
        'Authorization': 'Bearer ' + os.environ['GH_TOKEN'],
        'Accept': 'application/vnd.github+json',
        'Content-Type': 'application/json',
        'X-GitHub-Api-Version': '2022-11-28',
        'User-Agent': 'cargo-allow-bounded-repair-4296',
    })
    try:
        with urllib.request.urlopen(req, timeout=60) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        raise RuntimeError(f'Git object request failed with HTTP {error.code}') from None


root = Path('qualification')
q = json.loads((root / 'qualification.json').read_bytes())
if q['source'] != SOURCE or q['result'] != 'passed':
    raise ValueError('invalid qualification subject or result')
rows = q['files']
if len(rows) != 4 or {r['path'] for r in rows} != PATHS:
    raise ValueError('unexpected repair write set')
pr = request('GET', '/pulls/4296')
if pr['state'] != 'open' or not pr['draft'] or pr['head']['sha'] != SOURCE:
    raise ValueError('PR moved or is not the expected open draft; no objects prepared')
source = request('GET', f'/git/commits/{SOURCE}')
if source['tree']['sha'] != q['source_tree']:
    raise ValueError('source tree mismatch')
entries = []
for row in rows:
    path = root / 'files' / row['path']
    if path.is_symlink():
        raise ValueError('symlinks are not repair inputs')
    raw = path.read_bytes()
    blob = hashlib.sha1(f'blob {len(raw)}\0'.encode() + raw).hexdigest()
    if len(raw) != row['size'] or hashlib.sha256(raw).hexdigest() != row['sha256'] or blob != row['git_blob']:
        raise ValueError('repair bytes differ from tested bytes')
    created = request('POST', '/git/blobs', {'encoding': 'base64', 'content': base64.b64encode(raw).decode()})
    if created['sha'] != blob:
        raise ValueError('GitHub returned a different blob')
    entries.append({'path': row['path'], 'mode': '100644', 'type': 'blob', 'sha': blob})
tree = request('POST', '/git/trees', {'base_tree': q['source_tree'], 'tree': entries})
commit = request('POST', '/git/commits', {
    'message': 'fix(release): fail closed on stale custody observations and start time (#3927)\n\nNine named regressions fail on the predecessor and pass after the bounded repair. Preserve post-expiry settlement of already-started work. Synthetic fixtures only; no publication credential reads. Prepared by read-only qualification plus detached object retention; branch integration and full current-pair review remain separate.',
    'tree': tree['sha'], 'parents': [SOURCE],
})
out = Path('prepared')
out.mkdir(exist_ok=True)
(out / 'prepared-commit.json').write_text(json.dumps({
    'source': SOURCE, 'source_tree': q['source_tree'], 'commit': commit['sha'], 'tree': tree['sha'],
    'workflow_run': os.environ['GITHUB_RUN_ID'], 'workflow_attempt': os.environ['GITHUB_RUN_ATTEMPT'],
    'qualification_sha256': hashlib.sha256((root / 'qualification.json').read_bytes()).hexdigest(),
    'files': rows, 'ref_updated': False,
}, indent=2) + '\n', encoding='utf-8')
print('Prepared detached commit:', commit['sha'])
print('No branch, tag, release, or publication operation was performed.')
