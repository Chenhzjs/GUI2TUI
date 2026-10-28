//! Optional source-backed organization, shared by Human and Agent consumers.
//! Differences are observations, never task relevance, authority or nominees.
use super::TuiDocument;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum AnnotationMode {
    #[default]
    Off,
    Relations,
    Contrast,
}

struct Feature {
    kind: &'static str,
    value: Value,
    sources: Vec<u64>,
}

fn semantic(node: &Value) -> Value {
    // Identity/order/geometry are references, never distinguishing values.
    json!({"role":node["role"],"label":node["label"],"description":node["description"]})
}

fn clean(node: &Value) -> bool {
    node["partial"] != true
}

fn chain(index: &HashMap<u64, &Value>, target: u64) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = Some(target);
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        let Some(node) = index.get(&id) else {
            break;
        };
        ids.push(id);
        current = node["parent"].as_u64();
    }
    ids.reverse();
    ids
}

fn features(
    base: &Value,
    index: &HashMap<u64, &Value>,
    paths: &[u64],
    target: u64,
) -> BTreeMap<&'static str, Feature> {
    let mut out = BTreeMap::new();
    let node = index[&target];
    if paths.iter().all(|id| clean(index[id])) {
        out.insert(
            "ancestor_context",
            Feature {
                kind: "ancestry",
                value: json!(
                    paths[..paths.len() - 1]
                        .iter()
                        .map(|id| semantic(index[id]))
                        .collect::<Vec<_>>()
                ),
                sources: paths[..paths.len() - 1].to_vec(),
            },
        );
    }
    let roots: BTreeSet<_> = base["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["root"].as_u64())
        .collect();
    if let Some(owner) = paths.iter().rev().find(|id| roots.contains(id)) {
        if clean(index[owner]) && index[owner]["boundary_only"] != true {
            out.insert(
                "content_owner",
                Feature {
                    kind: "content_membership",
                    value: semantic(index[owner]),
                    sources: vec![*owner],
                },
            );
        }
    }
    if clean(node) {
        if !node["description"].is_null() {
            out.insert(
                "target_description",
                Feature {
                    kind: "public_description",
                    value: node["description"].clone(),
                    sources: vec![target],
                },
            );
        }
        let mut states = node["states"].as_array().cloned().unwrap_or_default();
        states.sort_by_key(Value::to_string);
        out.insert(
            "states",
            Feature {
                kind: "public_states",
                value: json!(states),
                sources: vec![target],
            },
        );
        let text = base["text_payloads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["source"] == target && p["read"]["status"] == "complete");
        if let Some(text) = text {
            out.insert(
                "value",
                Feature {
                    kind: "complete_public_text",
                    value: text["read"]["text"].clone(),
                    sources: vec![target],
                },
            );
        } else if !node["value"].is_null() {
            out.insert(
                "value",
                Feature {
                    kind: "semantic_cache_value",
                    value: node["value"].clone(),
                    sources: vec![target],
                },
            );
        }
    }
    if let Some(row) = base["relations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == target && r["status"] == "known")
    {
        let mut values = Vec::new();
        let mut refs = vec![target];
        let mut complete = true;
        for edge in row["edges"].as_array().into_iter().flatten() {
            let mut targets = Vec::new();
            for endpoint in edge["targets"].as_array().into_iter().flatten() {
                // A boundary/unread endpoint is not an observed empty relation.
                if let Some(id) = endpoint["node"]
                    .as_u64()
                    .filter(|id| endpoint["boundary"].is_null() && index.contains_key(id))
                {
                    if clean(index[&id]) && index[&id]["boundary_only"] != true {
                        refs.push(id);
                        targets.push(semantic(index[&id]));
                    } else {
                        complete = false;
                    }
                } else {
                    complete = false;
                }
            }
            targets.sort_by_key(Value::to_string);
            values.push(json!({"kind":edge["kind"],"targets":targets}));
        }
        values.sort_by_key(Value::to_string);
        if complete {
            out.insert(
                "public_relations",
                Feature {
                    kind: "known_relations",
                    value: json!(values),
                    sources: refs,
                },
            );
        }
    }
    out
}

fn signature(op: &Value, node: &Value) -> String {
    let mut domain = op["parameter_domain"].clone();
    if let Some(fields) = domain.as_object_mut() {
        fields.remove("current");
    }
    let description = node["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"] == op["name"])
        .map(|a| a["description"].clone());
    json!({"role":node["role"],"public_label":node["label"],"kind":op["kind"],"name":op["name"],"description":description,
        "parameter":op["parameter"],"parameter_domain":domain,"selected":op["selected"],"increase":op["increase"],"multiline":op["multiline"]}).to_string()
}

/// Reset and rebuild for this observation. Full facts and operation bindings
/// are untouched. Serializing the already privacy-checked base reuses the one
/// public schema, rather than creating another semantic projection.
pub fn apply(document: &mut TuiDocument, mode: AnnotationMode) {
    document.decision_annotations = None;
    if mode == AnnotationMode::Off {
        return;
    }
    let base = super::wire::to_agent_document(document);
    document.decision_annotations = Some(organize(&base, mode));
}

/// Replay the same organizer over one frozen public wire projection. These
/// structural checks neither authenticate a producer nor grant execution.
pub fn from_public_projection(
    base: &Value,
    mode: AnnotationMode,
) -> Result<Option<Value>, &'static str> {
    if base["version"] != super::wire::DOCUMENT_VERSION {
        return Err("Expected the versioned public document projection");
    }
    for field in [
        "nodes",
        "operations",
        "content",
        "text_payloads",
        "relations",
    ] {
        if !base[field].is_array() {
            return Err("Missing public document inventory");
        }
    }
    let nodes = base["nodes"].as_array().unwrap();
    let ids: Option<BTreeSet<_>> = nodes.iter().map(|n| n["id"].as_u64()).collect();
    let ids = ids.ok_or("Invalid public source ID")?;
    if ids.len() != nodes.len() || nodes.iter().any(|n| n["redacted"] == true) {
        return Err("Duplicate or redacted public source");
    }
    let present = |value: &Value| value.as_u64().is_some_and(|id| ids.contains(&id));
    if base["session"].as_str().is_none_or(str::is_empty)
        || base["generation"].as_u64().is_none_or(|g| g == 0)
        || base["observation_id"].as_u64().is_none()
        || !present(&base["interaction_scope"])
        || !present(&base["readable_scope"])
    {
        return Err("Missing public observation provenance");
    }
    for node in nodes {
        if (!node["parent"].is_null() && !present(&node["parent"]))
            || !node["children"]
                .as_array()
                .is_some_and(|v| v.iter().all(&present))
        {
            return Err("Missing public ancestry endpoint");
        }
    }
    let operations = base["operations"].as_array().unwrap();
    let names: Option<BTreeSet<_>> = operations.iter().map(|o| o["id"].as_str()).collect();
    if names.is_none_or(|names| names.len() != operations.len())
        || operations.iter().any(|o| !present(&o["target"]))
    {
        return Err("Missing or ambiguous public operation endpoint");
    }
    if base["text_payloads"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| !present(&p["source"]))
        || base["relations"].as_array().unwrap().iter().any(|r| {
            !present(&r["source"])
                || r["edges"].as_array().into_iter().flatten().any(|e| {
                    e["targets"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|t| !t["node"].is_null() && !present(&t["node"]))
                })
        })
    {
        return Err("Missing public payload or relation endpoint");
    }
    Ok((mode != AnnotationMode::Off).then(|| organize(base, mode)))
}

fn organize(base: &Value, mode: AnnotationMode) -> Value {
    let nodes = base["nodes"].as_array().unwrap();
    let index: HashMap<_, _> = nodes
        .iter()
        .map(|n| (n["id"].as_u64().unwrap(), n))
        .collect();
    let operations = base["operations"].as_array().unwrap();
    let mut contexts = Vec::new();
    let mut groups: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    let mut feature_index = BTreeMap::new();
    let mut hints: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for op in operations {
        let target = op["target"].as_u64().unwrap();
        let paths = chain(&index, target);
        feature_index
            .entry(target)
            .or_insert_with(|| features(base, &index, &paths, target));
        let mut relations = Vec::new();
        for source in &paths {
            let read = base["relations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["source"] == *source)
                .cloned()
                .unwrap_or(json!({"source":source,"status":"unknown"}));
            let ids: BTreeSet<_> = read["edges"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|e| e["targets"].as_array().into_iter().flatten())
                .filter_map(|e| e["node"].as_u64())
                .collect();
            let read = if base["relations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["source"] == *source)
            {
                json!({"relation_ref":source})
            } else {
                // Boundary-only ancestors have no public relation read. Keep
                // the original explicit unknown, rather than invent a ref.
                read
            };
            relations.push(json!({"source":source,"read":read,"endpoints":ids.iter().filter(|id|index.contains_key(id)).map(|id|json!({"node_ref":id})).collect::<Vec<_>>()}));
        }
        contexts.push(json!({"operation_id":op["id"],"target":target,"ancestors":paths[..paths.len()-1].iter().map(|id|json!({"node_ref":id})).collect::<Vec<_>>(),"relations":relations}));
        let labels: Vec<_> = paths[..paths.len() - 1]
            .iter()
            .filter_map(|id| index[id]["label"].as_str())
            .filter(|s| !s.is_empty())
            .collect();
        hints
            .entry(target)
            .or_insert_with(|| vec![format!("Public ancestry: {}", labels.join(" / "))]);
        groups
            .entry(signature(op, index[&target]))
            .or_default()
            .push(op);
    }
    let mut cohorts = Vec::new();
    if mode == AnnotationMode::Contrast {
        for (description, members) in groups {
            let targets: Vec<_> = members
                .iter()
                .map(|m| m["target"].as_u64().unwrap())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if targets.len() < 2 {
                continue;
            }
            let mut unresolved = BTreeSet::new();
            for (i, a) in targets.iter().enumerate() {
                for b in &targets[i + 1..] {
                    unresolved.insert((*a, *b));
                }
            }
            let mut selected = Vec::new();
            // Greedy source-pair coverage, with a declared tie order. This does
            // not estimate task relevance and never chooses a target for users.
            let order = [
                "content_owner",
                "ancestor_context",
                "public_relations",
                "target_description",
                "states",
                "value",
            ];
            loop {
                let mut best = None;
                let mut best_pairs = Vec::new();
                for name in order {
                    let covered: Vec<_> = unresolved
                        .iter()
                        .copied()
                        .filter(|(a, b)| {
                            match (feature_index[a].get(name), feature_index[b].get(name)) {
                                (Some(a), Some(b)) => a.kind == b.kind && a.value != b.value,
                                _ => false,
                            }
                        })
                        .collect();
                    if covered.len() > best_pairs.len() {
                        best = Some(name);
                        best_pairs = covered;
                    }
                }
                let Some(name) = best else {
                    break;
                };
                let observations:Vec<_>=targets.iter().map(|target|{
                    match feature_index[target].get(name) {
                        Some(f)=>json!({"target":target,"status":"known","source_kind":f.kind,"value":f.value,"source_refs":f.sources}),
                        None=>json!({"target":target,"status":"unavailable"}),
                    }
                }).collect();
                for row in &observations {
                    let target = row["target"].as_u64().unwrap();
                    hints.entry(target).or_default().push(format!(
                        "Observed difference ({name}, task relevance unknown): {}",
                        row["value"]
                    ));
                }
                for pair in &best_pairs {
                    unresolved.remove(pair);
                }
                selected.push(
                    json!({"feature":name,"observations":observations,"covered_pairs":best_pairs}),
                );
            }
            for (a, b) in &unresolved {
                for id in [a, b] {
                    hints.entry(*id).or_default().push(
                        "Declared comparison features unresolved; task identity is not established"
                            .into(),
                    );
                }
            }
            cohorts.push(json!({"description":serde_json::from_str::<Value>(&description).unwrap(),
                "members":members.iter().map(|m|json!({"operation_id":m["id"],"target":m["target"]})).collect::<Vec<_>>(),
                "distinguishing_facts":selected,"unresolved_peers":unresolved,"task_relevance":"not_evaluated"}));
        }
    }
    let mut ambiguous = Vec::new();
    for n in nodes {
        let mut counts = BTreeMap::new();
        for a in n["actions"].as_array().into_iter().flatten() {
            if let Some(name) = a["name"].as_str().filter(|s| !s.is_empty()) {
                *counts.entry(name).or_insert(0usize) += 1;
            }
        }
        for (name, count) in counts {
            if count > 1 {
                ambiguous.push(json!({"source":n["id"],"name":name,"advertised_count":count,"status":"unresolved_named_action"}));
            }
        }
    }
    json!({"version":"tui.decision_annotations.v2","context_representation":"canonical_document_refs.v1",
        "context_reference_sources":{"node_ref":"document.nodes[id]","relation_ref":"document.relations[source]"},"mode":if mode==AnnotationMode::Contrast{"contrast"}else{"relations"},
        "observation_id":base["observation_id"],"session":base["session"],"generation":base["generation"],
        "readable_scope":base["readable_scope"],"interaction_scope":base["interaction_scope"],
        "meaning":"Observed public differences only; task relevance not evaluated; no nominee, capability or authority change",
        "contexts":contexts,"cohorts":cohorts,"action_ambiguities":ambiguous,
        "human_hints":hints.into_iter().map(|(target,text)|json!({"target":target,"text":text})).collect::<Vec<_>>()})
}

pub fn human_hint(
    document: &TuiDocument,
    target: crate::semantic::RuntimeNodeId,
) -> Option<String> {
    document.decision_annotations.as_ref()?["human_hints"]
        .as_array()?
        .iter()
        .find(|h| h["target"] == target.get())?["text"]
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" · ")
        })
}
