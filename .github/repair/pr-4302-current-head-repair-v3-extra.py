from pathlib import Path

script = Path("/tmp/pr-4302-current-head-repair-v1.py")
source = script.read_text(encoding="utf-8")
anchor = 'source_path.write_text(source, encoding="utf-8", newline="\\n")\n\ncheckpoint_path = Path('
injection = r'''lib_path = Path("crates/allow-report/src/lib.rs")
lib = lib_path.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    """    digest_publication_checkpoint_body_v1, digest_publication_checkpoint_bytes_v1,
    digest_publication_checkpoint_v1, record_checkpoint_readback_v1,
""",
    """    digest_publication_checkpoint_body_v1, digest_publication_checkpoint_bytes_v1,
    digest_publication_checkpoint_link_v1, digest_publication_checkpoint_v1,
    record_checkpoint_readback_v1,
""",
    "root re-export for stable checkpoint link digest",
)
lib_path.write_text(lib, encoding="utf-8", newline="\n")

'''
if source.count(anchor) != 1:
    raise AssertionError("source write anchor moved before root export injection")
source = source.replace(anchor, anchor.replace("\n\ncheckpoint_path", "\n\n" + injection + "checkpoint_path"), 1)
script.write_text(source, encoding="utf-8", newline="\n")
