use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::{DependencyInfo, DependencyKind, DependencyRef};
use crate::storage::sqlite::SqliteStorage;

/// Expand the dependency graph starting from `seed_region_ids` up to `depth`
/// hops and return a `DependencyInfo` for each seed (and any transitively
/// discovered) region.
///
/// * `calls` — outgoing deps with kind `Calls`
/// * `type_references` — outgoing deps with kind `TypeReference`
/// * `called_by` — incoming deps with kind `Calls`
pub fn expand_dependencies(
    storage: &SqliteStorage,
    region_ids: &[i64],
    depth: usize,
) -> Result<HashMap<i64, DependencyInfo>> {
    let mut result: HashMap<i64, DependencyInfo> = HashMap::new();

    if depth == 0 || region_ids.is_empty() {
        for &id in region_ids {
            result.entry(id).or_default();
        }
        return Ok(result);
    }

    // BFS queue: (region_id, remaining_depth)
    let mut queue: VecDeque<(i64, usize)> = VecDeque::new();
    let mut visited: HashSet<i64> = HashSet::new();

    for &id in region_ids {
        if visited.insert(id) {
            queue.push_back((id, depth));
        }
    }

    while let Some((region_id, remaining)) = queue.pop_front() {
        let info = result.entry(region_id).or_default();

        // --- Outgoing dependencies ---
        let outgoing = storage.get_dependencies_from(region_id)?;
        for dep in &outgoing {
            let dep_ref = resolve_dep_ref(storage, dep.target_region_id, &dep.target_symbol, dep.target_path.as_deref())?;

            match dep.kind {
                DependencyKind::Calls => info.calls.push(dep_ref),
                DependencyKind::TypeReference => info.type_references.push(dep_ref),
                _ => {} // Imports, Inherits, Implements — not tracked in DependencyInfo
            }

            // Enqueue target region for further expansion if resolved and depth allows.
            if remaining > 1 {
                if let Some(target_id) = dep.target_region_id {
                    if visited.insert(target_id) {
                        queue.push_back((target_id, remaining - 1));
                    }
                }
            }
        }

        // --- Incoming dependencies (called_by) ---
        let incoming = storage.get_dependencies_to(region_id)?;
        for dep in &incoming {
            if dep.kind == DependencyKind::Calls {
                let dep_ref = resolve_dep_ref(
                    storage,
                    Some(dep.source_region_id),
                    &dep.target_symbol, // use the dep's target_symbol as name hint
                    dep.target_path.as_deref(),
                )?;
                // The caller is the source region; look up source for correct name/file.
                let caller_ref = resolve_caller_ref(storage, dep.source_region_id, &dep_ref)?;
                info.called_by.push(caller_ref);

                if remaining > 1 && visited.insert(dep.source_region_id) {
                    queue.push_back((dep.source_region_id, remaining - 1));
                }
            }
        }
    }

    // Ensure every seed has an entry even if it had no deps.
    for &id in region_ids {
        result.entry(id).or_default();
    }

    Ok(result)
}

/// Resolve a `DependencyRef` for an outgoing dependency target.
fn resolve_dep_ref(
    storage: &SqliteStorage,
    target_region_id: Option<i64>,
    target_symbol: &str,
    target_path: Option<&str>,
) -> Result<DependencyRef> {
    if let Some(tid) = target_region_id {
        if let Some(region) = storage.get_region(tid)? {
            if let Some(file) = storage.get_file(region.file_id)? {
                return Ok(DependencyRef {
                    name: region.name.clone(),
                    file: file.path.clone(),
                    lines: [region.start_line, region.end_line],
                });
            }
        }
    }

    // Fallback: unresolved — use symbol name and path hint.
    Ok(DependencyRef {
        name: target_symbol.to_string(),
        file: target_path.unwrap_or("").to_string(),
        lines: [0, 0],
    })
}

/// Resolve a `DependencyRef` for the *caller* (source) side of a Calls edge.
fn resolve_caller_ref(
    storage: &SqliteStorage,
    source_region_id: i64,
    _fallback: &DependencyRef,
) -> Result<DependencyRef> {
    if let Some(region) = storage.get_region(source_region_id)? {
        if let Some(file) = storage.get_file(region.file_id)? {
            return Ok(DependencyRef {
                name: region.name.clone(),
                file: file.path.clone(),
                lines: [region.start_line, region.end_line],
            });
        }
    }

    Ok(DependencyRef {
        name: format!("region:{}", source_region_id),
        file: String::new(),
        lines: [0, 0],
    })
}
