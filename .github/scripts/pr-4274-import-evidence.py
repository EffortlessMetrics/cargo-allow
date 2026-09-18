"""Verify a pinned native proof archive and import its exact bounded outputs."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import zipfile

PRODUCTS = ('cargo-allow', 'shared', 'cargo-intent', 'cargo-proof')
EXPECTED_PATHS = frozenset(
    ['docs/ci/direct-floor-proof.md',
     'docs/dogfood/receipts/package-candidate-v2.example.json'] +
    [f'docs/ci/receipts/direct-floor-proof-{product}-v1{suffix}'
     for product in PRODUCTS for suffix in ('.json', '.selection.md')]
)
MAX_ARCHIVE = 25 * 1024 * 1024
MAX_OUTPUT = 1024 * 1024


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def blob_id(raw: bytes) -> str:
    return hashlib.sha1(f'blob {len(raw)}\0'.encode() + raw).hexdigest()


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, f'duplicate JSON key: {key}')
        result[key] = value
    return result


def verify_archive(archive: Path, digest: str, identity: dict[str, str]) -> dict[str, bytes]:
    require(archive.stat().st_size <= MAX_ARCHIVE, 'archive exceeds bound')
    require(hashlib.sha256(archive.read_bytes()).hexdigest() == digest,
            'archive digest mismatch')
    with zipfile.ZipFile(archive) as zipped:
        infos = zipped.infolist()
        require(len(infos) <= 512, 'too many archive entries')
        require(len({item.filename for item in infos}) == len(infos),
                'duplicate archive entries')
        require(sum(item.file_size for item in infos) <= MAX_ARCHIVE,
                'expanded archive exceeds bound')
        for item in infos:
            path = PurePosixPath(item.filename)
            require(not path.is_absolute() and '..' not in path.parts and
                    '\\' not in item.filename and ':' not in item.filename,
                    'unsafe archive path')
            require(not stat.S_ISLNK(item.external_attr >> 16), 'symlink in archive')
            require(not item.flag_bits & 1, 'encrypted archive entry')
        info = zipped.getinfo('qualification.json')
        require(info.file_size <= 65536, 'qualification manifest exceeds bound')
        manifest = json.loads(zipped.read(info), object_pairs_hook=unique_object)
        require(isinstance(manifest, dict), 'invalid manifest object')
        require(manifest.get('result') == 'passed', 'qualification did not pass')
        for key, expected in identity.items():
            require(manifest.get(key) == expected, f'manifest {key} mismatch')
        require(isinstance(manifest.get('claim_boundary'), str) and
                bool(manifest['claim_boundary'].strip()), 'missing claim boundary')
        rows = manifest.get('files')
        require(isinstance(rows, list) and len(rows) == len(EXPECTED_PATHS),
                'wrong output count')
        require(all(isinstance(row, dict) and isinstance(row.get('path'), str)
                    for row in rows), 'invalid output row')
        require({row['path'] for row in rows} == EXPECTED_PATHS, 'wrong output paths')
        archived_outputs = {name[len('files/'):] for name in zipped.namelist()
                            if name.startswith('files/') and not name.endswith('/')}
        require(archived_outputs == EXPECTED_PATHS, 'unrecorded archive output')
        outputs = {}
        for row in rows:
            name = row['path']
            require(type(row.get('size')) is int and 0 < row['size'] <= MAX_OUTPUT,
                    'invalid output size')
            info = zipped.getinfo('files/' + name)
            require(info.file_size == row['size'], f'output size mismatch: {name}')
            raw = zipped.read(info)
            require(hashlib.sha256(raw).hexdigest() == row.get('sha256'),
                    f'output SHA-256 mismatch: {name}')
            require(blob_id(raw) == row.get('git_blob'), f'output blob mismatch: {name}')
            raw.decode('utf-8')
            require(b'\r' not in raw, f'non-LF output: {name}')
            outputs[name] = raw
        return outputs


def git(*args: str) -> bytes:
    return subprocess.check_output(['git', *args], timeout=60)


def import_outputs(outputs: dict[str, bytes], source: str, tree: str,
                   main: str, message: str) -> dict:
    require(set(outputs) == EXPECTED_PATHS, 'unbounded import')
    require(git('rev-parse', 'HEAD').decode().strip() == source, 'checkout moved')
    require(git('rev-parse', 'HEAD^{tree}').decode().strip() == tree, 'source tree mismatch')
    git('merge-base', '--is-ancestor', main, source)
    require(not git('status', '--porcelain=v1', '--untracked-files=all'), 'dirty source')
    require(not git('branch', '--show-current').strip(), 'source must be detached')
    for path in sorted(outputs):
        require(Path(path).is_file() and not Path(path).is_symlink(), 'output must replace tracked file')
        git('ls-files', '--error-unmatch', '--', path)
        require(all(not parent.is_symlink() for parent in Path(path).parents), 'symlink parent')
    for path, raw in outputs.items():
        Path(path).write_bytes(raw)
    require(set(git('diff', '--name-only').decode().splitlines()) == EXPECTED_PATHS,
            'incomplete or unexpected worktree delta')
    git('diff', '--check')
    git('add', '--', *sorted(EXPECTED_PATHS))
    require(set(git('diff', '--cached', '--name-only').decode().splitlines()) == EXPECTED_PATHS,
            'unexpected index delta')
    require(not git('diff', '--name-only'), 'unstaged delta')
    for path, raw in outputs.items():
        require(git('rev-parse', ':' + path).decode().strip() == blob_id(raw),
                f'staged bytes changed: {path}')
    git('-c', 'commit.gpgSign=false', 'commit', '-m', message)
    head = git('rev-parse', 'HEAD').decode().strip()
    require(git('rev-list', '--parents', '-1', head).decode().split() == [head, source],
            'wrong commit ancestry')
    require(not git('status', '--porcelain=v1', '--untracked-files=all'), 'unclean output')
    return {'source': source, 'main': main, 'head': head,
            'tree': git('rev-parse', 'HEAD^{tree}').decode().strip(),
            'files': [{'path': path, 'git_blob': blob_id(raw),
                       'sha256': hashlib.sha256(raw).hexdigest(), 'size': len(raw)}
                      for path, raw in sorted(outputs.items())]}


def main() -> None:
    identity = {key: os.environ[env] for key, env in (
        ('source_commit', 'SOURCE_HEAD'), ('source_tree', 'SOURCE_TREE'),
        ('main_commit', 'MAIN_HEAD'), ('workflow_run', 'NATIVE_RUN'),
        ('workflow_attempt', 'NATIVE_ATTEMPT'))}
    archive = Path(os.environ['NATIVE_ARCHIVE'])
    outputs = verify_archive(archive, os.environ['NATIVE_DIGEST'], identity)
    result = import_outputs(outputs, identity['source_commit'], identity['source_tree'],
                            identity['main_commit'], os.environ['IMPORT_MESSAGE'])
    result['artifact_id'] = os.environ['NATIVE_ARTIFACT']
    result['archive_sha256'] = os.environ['NATIVE_DIGEST']
    result['claim_boundary'] = 'Verified native artifact import only; no new Rust or final-head qualification.'
    Path(os.environ['IMPORT_RECORD']).write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as stream:
        stream.write('commit=' + result['head'] + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
