use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::extractors::auth::AuthenticatedUser;
use crate::middleware::RequestContext;
use crate::repositories::proof as repo;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProofLatestResponse {
    snapshot_id: Uuid,
    merkle_root: String,
    generated_at: String,
    liabilities: Vec<ProofLiabilityResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProofLiabilityResponse {
    asset: String,
    total: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MyProofResponse {
    snapshot_id: Uuid,
    merkle_root: String,
    generated_at: String,
    entries: Vec<MyProofEntryResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MyProofEntryResponse {
    asset: String,
    total: String,
    leaf_payload: String,
    leaf_hash: String,
    sibling_hashes: Vec<String>,
}

/// `GET /proof-of-reserves/latest`
///
/// # Errors
///
/// Returns [`ApiError`] when the proof snapshot cannot be generated or loaded.
pub async fn latest_proof(
    State(state): State<AppState>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let snapshot = repo::ensure_current_snapshot(&state.db)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "proof.latest.db_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;
    Ok(Json(latest_response(snapshot)))
}

/// `GET /proof-of-reserves/me`
///
/// # Errors
///
/// Returns [`ApiError`] when the caller is unauthenticated or the proof cannot
/// be generated.
pub async fn my_proof(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let snapshot = repo::ensure_current_snapshot(&state.db)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "proof.me.db_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;
    let entries = repo::user_proof_entries(&snapshot, caller.user_id)
        .into_iter()
        .map(|entry| MyProofEntryResponse {
            asset: entry.asset,
            total: entry.total,
            leaf_payload: entry.leaf_payload,
            leaf_hash: entry.leaf_hash,
            sibling_hashes: entry.sibling_hashes,
        })
        .collect();

    Ok(Json(MyProofResponse {
        snapshot_id: snapshot.id,
        merkle_root: snapshot.merkle_root,
        generated_at: snapshot.generated_at.to_string(),
        entries,
    }))
}

fn latest_response(snapshot: repo::ProofSnapshot) -> ProofLatestResponse {
    let liabilities = repo::summarize_liabilities(&snapshot)
        .into_iter()
        .map(|summary| ProofLiabilityResponse {
            asset: summary.asset,
            total: summary.total,
        })
        .collect();
    ProofLatestResponse {
        snapshot_id: snapshot.id,
        merkle_root: snapshot.merkle_root,
        generated_at: snapshot.generated_at.to_string(),
        liabilities,
    }
}
