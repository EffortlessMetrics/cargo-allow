"""Wire fragment validation into changie_lint: rules, module, call."""
from pathlib import Path

p = Path('crates/allow-files/src/changie_lint.rs')
t = p.read_bytes().decode('utf-8')

# 1. new rule variants
old = '''    FragmentPathNotDiscovered,
    FragmentEntryUnsupported,
    FragmentMalformed,
}'''
assert t.count(old) == 1
new = '''    FragmentPathNotDiscovered,
    FragmentEntryUnsupported,
    FragmentMalformed,
    FragmentKindMissing,
    FragmentKindUnknown,
    FragmentComponentMissing,
    FragmentComponentUnknown,
    FragmentProjectMissing,
    FragmentProjectUnknown,
    FragmentBodyMissing,
    FragmentBodyWrongType,
    FragmentBodyTooShort,
    FragmentBodyTooLong,
    FragmentTimeMissing,
    FragmentTimeInvalid,
    FragmentCustomMissing,
    FragmentCustomWrongType,
    FragmentCustomOutOfRange,
    FragmentCustomUnknownValue,
    FragmentCustomUnconfigured,
}'''
t = t.replace(old, new)

old = '''            Self::FragmentPathNotDiscovered => "changie.fragment.path_not_discovered",
            Self::FragmentEntryUnsupported => "changie.fragment.entry_unsupported",
            Self::FragmentMalformed => "changie.fragment.malformed",
        }'''
assert t.count(old) == 1
new = '''            Self::FragmentPathNotDiscovered => "changie.fragment.path_not_discovered",
            Self::FragmentEntryUnsupported => "changie.fragment.entry_unsupported",
            Self::FragmentMalformed => "changie.fragment.malformed",
            Self::FragmentKindMissing => "changie.fragment.kind_missing",
            Self::FragmentKindUnknown => "changie.fragment.kind_unknown",
            Self::FragmentComponentMissing => "changie.fragment.component_missing",
            Self::FragmentComponentUnknown => "changie.fragment.component_unknown",
            Self::FragmentProjectMissing => "changie.fragment.project_missing",
            Self::FragmentProjectUnknown => "changie.fragment.project_unknown",
            Self::FragmentBodyMissing => "changie.fragment.body_missing",
            Self::FragmentBodyWrongType => "changie.fragment.body_wrong_type",
            Self::FragmentBodyTooShort => "changie.fragment.body_too_short",
            Self::FragmentBodyTooLong => "changie.fragment.body_too_long",
            Self::FragmentTimeMissing => "changie.fragment.time_missing",
            Self::FragmentTimeInvalid => "changie.fragment.time_invalid",
            Self::FragmentCustomMissing => "changie.fragment.custom_missing",
            Self::FragmentCustomWrongType => "changie.fragment.custom_wrong_type",
            Self::FragmentCustomOutOfRange => "changie.fragment.custom_out_of_range",
            Self::FragmentCustomUnknownValue => "changie.fragment.custom_unknown_value",
            Self::FragmentCustomUnconfigured => "changie.fragment.custom_unconfigured",
        }'''
t = t.replace(old, new)

# 2. module include before tests
old = '''#[cfg(test)]
#[path = "changie_lint_tests.rs"]
mod tests;'''
assert t.count(old) == 1
new = '''mod fragment_rules;

#[cfg(test)]
#[path = "changie_lint_tests.rs"]
mod tests;'''
t = t.replace(old, new)

# 3. call fragment validation inside classify_entries where fragments are
#    consumed (after the malformed-diagnostic passthrough)
old = '''        // A malformed fragment stays in the population report.
        if let Some(fragment) = entry.fragment.as_ref()
            && !fragment.diagnostics.is_empty()
        {
            for diagnostic in &fragment.diagnostics {'''
assert t.count(old) == 1
new = '''        // Semantic fragment rules run for every supplied fragment
        // document, discovered or not (#3589 PR B2).
        if let Some(fragment) = entry.fragment.as_ref() {
            fragment_rules::validate_fragment(config, entry, fragment, diagnostics);
        }
        // A malformed fragment stays in the population report.
        if let Some(fragment) = entry.fragment.as_ref()
            && !fragment.diagnostics.is_empty()
        {
            for diagnostic in &fragment.diagnostics {'''
t = t.replace(old, new)

# 4. classify_entries regains its config parameter usage (it already has it)
p.write_bytes(t.encode('utf-8'))

# rename the file to the declared module name
Path('crates/allow-files/src/changie_lint_fragments.rs').rename(
    'crates/allow-files/src/changie_lint/fragment_rules.rs'
)
print('wired; moved to changie_lint/fragment_rules.rs')
