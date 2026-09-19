from pathlib import Path

repair = Path("/tmp/pr-4302-repair-v2.py")
source = repair.read_text(encoding="utf-8")
old = '''    finish = text.find(end, begin)
    if finish < 0:
        raise AssertionError(f"{label}: end marker missing")
'''
new = '''    finish = text.find(end, begin)
    if finish < 0 and label == "current-row dependant gate":
        finish = len(text)
    if finish < 0:
        raise AssertionError(f"{label}: end marker missing")
'''
if source.count(old) != 1:
    raise AssertionError("replace_between hotfix target moved")
repair.write_text(source.replace(old, new, 1), encoding="utf-8", newline="\n")
