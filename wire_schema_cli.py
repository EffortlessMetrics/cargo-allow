"""Wire the `changie schema --fragments` CLI route (#3623 file)."""
from pathlib import Path

p = Path('crates/cargo-allow/src/changie.rs')
t = p.read_bytes().decode('utf-8')

old = '''pub(crate) enum ChangieCommand {
    /// Run the Rust-native Changie static sensor over one exact source subject.
    Lint(ChangieLintArgs),
}'''
assert t.count(old) == 1
new = old + '''

#[derive(Debug, Clone, Args)]
pub(crate) struct ChangieSchemaArgs {
    /// Emit the repository's config-derived fragment JSON Schema.
    #[arg(long)]
    pub(crate) fragments: bool,
    /// Emit the schema-association descriptor for schema-aware editors
    /// (YAML Language Server, VS Code YAML, coc-yaml, and friends)
    /// instead of the schema itself.
    #[arg(long, requires = "fragments")]
    pub(crate) association: bool,
    #[arg(long, value_enum, default_value_t = ChangieFormat::Json)]
    pub(crate) format: ChangieFormat,
    /// Write output to a file instead of stdout. The source tree is
    /// never mutated.
    #[arg(long)]
    pub(crate) output: Option<std::path::PathBuf>,
    /// Select an explicit repository-relative Changie config path.
    #[arg(long)]
    pub(crate) config: Option<String>,
}'''
t = t.replace(old, new)

old2 = '''    /// Run the Rust-native Changie static sensor over one exact source subject.
    Lint(ChangieLintArgs),
}'''
# The enum now has both variants via the append above; add the variant:
old3 = '''    Lint(ChangieLintArgs),
}'''
assert t.count(old3) == 2  # enum + args struct tail
# only the first occurrence is the enum variant list
t = t.replace(old3, '''    Lint(ChangieLintArgs),
    /// Emit the config-derived fragment JSON Schema or its association.
    Schema(ChangieSchemaArgs),
}''', 1)

# dispatch in cmd_changie
old4 = '''    let ChangieCommand::Lint(lint) = &args.command;'''
assert t.count(old4) == 1
new4 = '''    match &args.command {
        ChangieCommand::Lint(lint) => return cmd_changie_lint(args, lint),
        ChangieCommand::Schema(schema) => return cmd_changie_schema(args, schema),
    }'''
t = t.replace(old4, new4)

# rename the existing body fn and add the schema fn
old5 = 'pub(crate) fn cmd_changie(args: &ChangieArgs) -> CargoAllowResult<()> {'
assert t.count(old5) == 1
t = t.replace(old5, '''fn cmd_changie_schema(args: &ChangieArgs, schema: &ChangieSchemaArgs) -> CargoAllowResult<()> {
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let view = effortless_repo_snapshot::RepositorySourceView::filesystem(&root)
        .map_err(|error| usage_error(format!("saved worktree view: {error}")))?;
    let selection = match &schema.config {
        Some(path) => ChangieConfigSelectionV1::Explicit(path.clone()),
        None => ChangieConfigSelectionV1::DefaultNames,
    };
    let sensor = allow_files::changie_lint::sensor::ChangieSensor;
    let config = crate::changie_source_view::selected_config(&view, &selection)
        .map_err(|error| usage_error(format!("changie schema: {error}")))?;
    let compiled = sensor
        .compile_contract(&config)
        .map_err(|error| usage_error(format!("changie schema: config not compiled: {error:?}")))?;
    let projection = sensor.fragment_schema(&compiled);
    let text = if schema.association {
        let root_dir = crate::changie_source_view::population_root(&config);
        let association = allow_files::changie_lint::fragment_schema::fragment_schema_association(
            &compiled,
            &projection,
            config.source.repo_path(),
            &root_dir,
        );
        render_association(&association)
    } else {
        projection.schema_text
    };
    emit_text(schema.output.as_deref(), &text)?;
    Ok(())
}

fn render_association(
    association: &allow_files::changie_lint::fragment_schema::ChangieFragmentSchemaAssociationV1,
) -> String {
    let mut out = String::new();
    out.push_str("{\\n");
    out.push_str("  \\"schema\\": \\"cargo-allow.changie-fragment-schema-association.v1\\",\\n");
    out.push_str(&format!(
        "  \\"compatibility_generation\\": \\"{}\\",\\n",
        association.compatibility_generation
    ));
    out.push_str(&format!(
        "  \\"config_path\\": \\"{}\\",\\n",
        association.config_path
    ));
    out.push_str(&format!(
        "  \\"config_content_identity\\": \\"{}\\",\\n",
        association.config_content_identity
    ));
    out.push_str(&format!("  \\"schema_id\\": \\"{}\\",\\n", association.schema_id));
    out.push_str(&format!(
        "  \\"schema_digest\\": \\"{}\\",\\n",
        association.schema_digest
    ));
    out.push_str(&format!(
        "  \\"fragment_path_patterns\\": [\\"{}\\"],\\n",
        association.fragment_path_patterns.join("\\", \\"")
    ));
    out.push_str(&format!(
        "  \\"source_subject\\": \\"{}\\",\\n",
        association.source_subject
    ));
    out.push_str(&format!(
        "  \\"completeness\\": \\"{}\\",\\n",
        association.completeness
    ));
    for limitation in &association.limitations {
        out.push_str(&format!(
            "  \\"limitation\\": \\"{limitation}\\",\\n"
        ));
    }
    out.push_str("  \\"config_schema_note\\": \\".changie.yaml and .changie.yml remain associated with Changie's official config schema, not this fragment schema\\"\\n");
    out.push_str("}\\n");
    out
}

fn cmd_changie_lint(args: &ChangieArgs, lint: &ChangieLintArgs) -> CargoAllowResult<()> {'''
p.write_bytes(t.encode('utf-8'))
print('CLI wired')
