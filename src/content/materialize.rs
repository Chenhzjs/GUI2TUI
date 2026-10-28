//! Complete public Text acquisition, independent of Human Reader residency.
use crate::{
    backend::{AtspiBackend, BackendError},
    modality::CancellationToken,
    semantic::{RuntimeNodeId, SemanticCache},
    transcompile::{
        InteractionScopes,
        document::{TextRead, secret_source},
    },
};
use std::{collections::BTreeMap, time::Instant};

#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct FullTextMetrics {
    pub sources: usize,
    pub unavailable: usize,
    pub bytes: usize,
    pub peak_in_flight: usize,
    pub wall_ms: f64,
}

pub async fn read_public_texts(
    backend: &AtspiBackend,
    cache: &SemanticCache,
    scopes: &InteractionScopes,
    cancel: &CancellationToken,
) -> Result<(BTreeMap<RuntimeNodeId, TextRead>, FullTextMetrics), String> {
    let started = Instant::now();
    let mut sources: Vec<_> = cache
        .nodes()
        .filter(|n| {
            scopes.allows_node(n.runtime_id)
                && !secret_source(cache, n.runtime_id)
                && n.debug.interfaces.iter().any(|i| i == "Text")
        })
        .map(|n| (n.runtime_id, n.backend_locator.clone()))
        .collect();
    sources.sort_by_key(|(id, _)| *id);
    let mut metrics = FullTextMetrics {
        sources: sources.len(),
        ..Default::default()
    };
    let mut pending = sources.into_iter();
    let mut reads = tokio::task::JoinSet::new();
    let mut results = BTreeMap::new();
    loop {
        if cancel.is_cancelled() {
            return Err("Public text materialization cancelled".into());
        }
        // Existing 32-way schedule, no source-count limit or extra concurrency.
        while reads.len() < 32 {
            let Some((id, locator)) = pending.next() else {
                break;
            };
            let backend = backend.clone();
            reads.spawn(async move { (id, backend.read_semantic_text_block(&locator).await) });
        }
        metrics.peak_in_flight = metrics.peak_in_flight.max(reads.len());
        let Some(read) = reads.join_next().await else {
            break;
        };
        let (id, result) = read.map_err(|_| "Public text worker failed".to_owned())?;
        let read = match result {
            Ok(range) => {
                metrics.bytes += range.text.len();
                TextRead::Complete(range.text)
            }
            Err(BackendError::SecretContentDisabled(_)) => {
                return Err("Text target became sensitive; observe again".into());
            }
            Err(_) => {
                metrics.unavailable += 1;
                TextRead::Unavailable
            }
        };
        results.insert(id, read);
    }
    metrics.wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok((results, metrics))
}
