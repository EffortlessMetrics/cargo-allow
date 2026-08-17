"""Key schema identity on the order-independent contract digest."""
from pathlib import Path

BS = chr(92)
p = Path('crates/allow-files/src/changie_lint/fragment_schema.rs')
t = p.read_bytes().decode('utf-8')

old_id = (
    '        "' + BS + '$id' + BS + '": ' + BS + '"cargo-allow.changie-fragment.v1;config={}' + BS + '",' + BS + 'n",'
)
new_id = (
    '        "' + BS + '$id' + BS + '": ' + BS + '"cargo-allow.changie-fragment.v1;contract={}' + BS + '",' + BS + 'n",'
)
assert t.count(old_id) == 1, t.count(old_id)
t = t.replace(old_id, new_id)

old_title = (
    '        "  ' + BS + '"title' + BS + '": ' + BS + '"Changie fragment authoring contract (config {})' + BS + '",' + BS + 'n",'
)
new_title = (
    '        "  ' + BS + '"title' + BS + '": ' + BS + '"Changie fragment authoring contract (contract {})' + BS + '",' + BS + 'n",'
)
assert t.count(old_title) == 1, t.count(old_title)
t = t.replace(old_title, new_title)

old_assoc = (
    '            "cargo-allow.changie-fragment.v1;config={}",'
)
assert t.count(old_assoc) == 1, t.count(old_assoc)
t = t.replace(old_assoc, '            "cargo-allow.changie-fragment.v1;contract={}",')

# both digest sites: config_identity -> digest inside the two format! calls
old_arg1 = '        compiled.config_identity' + chr(10) + '    ));'
count = t.count(old_arg1)
# Replace the two occurrences (id and title) with digest
t = t.replace(old_arg1, '        compiled.digest' + chr(10) + '    ));')

old_arg2 = '            compiled.config_identity'
count2 = t.count(old_arg2)
t = t.replace(old_arg2, '            compiled.digest')

p.write_bytes(t.encode('utf-8'))
print('identity rekeyed:', count, count2)
