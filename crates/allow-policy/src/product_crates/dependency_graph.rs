use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyClass {
    Normal,
    Dev,
    Build,
}

impl DependencyClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Dev => "dev",
            Self::Build => "build",
        }
    }

    fn parse(kind: Option<&str>) -> Self {
        match kind {
            Some("dev") => Self::Dev,
            Some("build") => Self::Build,
            _ => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub class: DependencyClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CargoMetadataGraph {
    pub edges: Vec<DependencyEdge>,
}

pub fn load_cargo_metadata_graph(root: &Path) -> CargoAllowResult<CargoMetadataGraph> {
    let manifest_path = root.join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .output()
        .map_err(|err| {
            CargoAllowError::new(format!(
                "failed to invoke cargo metadata at {}: {err}",
                manifest_path.display()
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CargoAllowError::new(format!(
            "cargo metadata failed at {}: {stderr}",
            manifest_path.display()
        )));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|err| {
        CargoAllowError::new(format!("cargo metadata output was not valid UTF-8: {err}"))
    })?;
    parse_cargo_metadata_graph(&stdout)
}

pub fn parse_cargo_metadata_graph(input: &str) -> CargoAllowResult<CargoMetadataGraph> {
    let parsed: MetadataJson = serde_json::from_str(input).map_err(|err| {
        CargoAllowError::new(format!("failed to parse cargo metadata JSON: {err}"))
    })?;
    let mut edges = Vec::new();
    for package in parsed.packages {
        for dependency in package.dependencies {
            edges.push(DependencyEdge {
                from: package.name.clone(),
                to: dependency.name,
                class: DependencyClass::parse(dependency.kind.as_deref()),
            });
        }
    }
    Ok(CargoMetadataGraph { edges })
}

pub fn shortest_dependency_path(
    graph: &CargoMetadataGraph,
    from: &str,
    to: &str,
) -> Option<Vec<String>> {
    if from == to {
        return Some(vec![from.to_string()]);
    }
    let mut adjacency: BTreeMap<&str, Vec<(&str, DependencyClass)>> = BTreeMap::new();
    for edge in &graph.edges {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push((edge.to.as_str(), edge.class));
    }
    let mut queue = std::collections::VecDeque::from([(from, vec![from.to_string()])]);
    let mut visited = BTreeMap::<&str, DependencyClass>::new();
    visited.insert(from, DependencyClass::Normal);
    while let Some((current, path)) = queue.pop_front() {
        let Some(neighbors) = adjacency.get(current) else {
            continue;
        };
        for (next, class) in neighbors {
            if visited.contains_key(next) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(next.to_string());
            if *next == to {
                return Some(next_path);
            }
            visited.insert(next, *class);
            queue.push_back((next, next_path));
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct MetadataJson {
    packages: Vec<MetadataPackage>,
}

#[derive(Debug, Deserialize)]
struct MetadataPackage {
    name: String,
    #[serde(default)]
    dependencies: Vec<MetadataDependency>,
}

#[derive(Debug, Deserialize)]
struct MetadataDependency {
    name: String,
    kind: Option<String>,
}
