from pathlib import Path

script = Path("/tmp/pr-4302-current-head-repair-v1.py")
source = script.read_text(encoding="utf-8")
anchor = 'lib_path = Path("crates/allow-report/src/lib.rs")\n'
injection = r'''artifacts_path = Path("crates/allow-report/src/artifacts.rs")
artifacts = artifacts_path.read_text(encoding="utf-8")
artifacts = replace_once(
    artifacts,
    """    digest_publication_checkpoint_body_v1, digest_publication_checkpoint_bytes_v1,
    digest_publication_checkpoint_v1, record_checkpoint_readback_v1,
""",
    """    digest_publication_checkpoint_body_v1, digest_publication_checkpoint_bytes_v1,
    digest_publication_checkpoint_link_v1, digest_publication_checkpoint_v1,
    record_checkpoint_readback_v1,
""",
    "artifacts-module export for stable checkpoint link digest",
)
artifacts_path.write_text(artifacts, encoding="utf-8", newline="\n")

'''
if source.count(anchor) != 1:
    raise AssertionError("lib export anchor moved before artifacts-module export injection")
source = source.replace(anchor, injection + anchor, 1)
script.write_text(source, encoding="utf-8", newline="\n")
