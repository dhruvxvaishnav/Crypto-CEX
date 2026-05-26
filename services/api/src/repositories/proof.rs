use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

const EMPTY_ROOT_PAYLOAD: &[u8] = b"aether:por:v1:empty";
const INTERNAL_PREFIX: &str = "aether:por:v1:node";
const LEAF_PREFIX: &str = "aether:por:v1:leaf";

/// A user liability committed into a proof-of-reserves snapshot.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofLiability {
    /// User ID included in the leaf payload.
    pub user_id: Uuid,
    /// Asset symbol.
    pub asset: String,
    /// Available plus locked balance in canonical decimal form.
    pub total: String,
}

/// Latest committed proof-of-reserves snapshot.
#[derive(Debug, Clone)]
pub struct ProofSnapshot {
    /// Snapshot row ID.
    pub id: Uuid,
    /// Merkle root as lowercase hex.
    pub merkle_root: String,
    /// Liabilities committed by the root.
    pub liabilities: Vec<ProofLiability>,
    /// Snapshot generation timestamp.
    pub generated_at: OffsetDateTime,
}

/// Per-asset public liability summary.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LiabilitySummary {
    /// Asset symbol.
    pub asset: String,
    /// Total committed liability for the asset.
    pub total: String,
}

/// User-specific Merkle proof entry.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UserProofEntry {
    /// Asset symbol.
    pub asset: String,
    /// User liability amount for this asset.
    pub total: String,
    /// Canonical payload hashed into the leaf.
    pub leaf_payload: String,
    /// Leaf hash as lowercase hex.
    pub leaf_hash: String,
    /// Sibling hashes from leaf to root.
    pub sibling_hashes: Vec<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProofSnapshotRow {
    id: Uuid,
    merkle_root: String,
    liabilities: Json<Vec<ProofLiability>>,
    generated_at: OffsetDateTime,
}

#[derive(Debug, sqlx::FromRow)]
struct LiabilityRow {
    user_id: Uuid,
    asset: String,
    total: Decimal,
}

/// Returns the latest committed snapshot, creating one when liabilities changed
/// or no snapshot exists yet.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn ensure_current_snapshot(pool: &PgPool) -> Result<ProofSnapshot, sqlx::Error> {
    let liabilities = current_liabilities(pool).await?;
    let tree = MerkleTree::new(liabilities.clone());
    if let Some(latest) = latest_snapshot(pool).await? {
        if latest.merkle_root == tree.root {
            return Ok(latest);
        }
    }
    insert_snapshot(pool, tree.root, liabilities).await
}

/// Aggregates snapshot liabilities by asset.
#[must_use]
pub fn summarize_liabilities(snapshot: &ProofSnapshot) -> Vec<LiabilitySummary> {
    let mut totals = std::collections::BTreeMap::<String, Decimal>::new();
    for liability in &snapshot.liabilities {
        let amount = liability.total.parse::<Decimal>().unwrap_or(Decimal::ZERO);
        totals
            .entry(liability.asset.clone())
            .and_modify(|total| *total += amount)
            .or_insert(amount);
    }
    totals
        .into_iter()
        .map(|(asset, total)| LiabilitySummary {
            asset,
            total: canonical_decimal(total),
        })
        .collect()
}

/// Builds all Merkle proof entries for one user.
#[must_use]
pub fn user_proof_entries(snapshot: &ProofSnapshot, user_id: Uuid) -> Vec<UserProofEntry> {
    MerkleTree::new(snapshot.liabilities.clone()).entries_for_user(user_id)
}

/// Verifies a Merkle proof against a root.
#[must_use]
pub fn verify_proof(leaf_payload: &str, sibling_hashes: &[String], root: &str) -> bool {
    let mut current = hash_hex(leaf_payload.as_bytes());
    for sibling in sibling_hashes {
        current = hash_pair(&current, sibling);
    }
    current == root
}

async fn latest_snapshot(pool: &PgPool) -> Result<Option<ProofSnapshot>, sqlx::Error> {
    sqlx::query_as::<_, ProofSnapshotRow>(
        r"
        SELECT id, merkle_root, liabilities, generated_at
        FROM proof_of_reserves
        ORDER BY generated_at DESC
        LIMIT 1
        ",
    )
    .fetch_optional(pool)
    .await
    .map(|row| row.map(snapshot_from_row))
}

async fn current_liabilities(pool: &PgPool) -> Result<Vec<ProofLiability>, sqlx::Error> {
    let rows = sqlx::query_as::<_, LiabilityRow>(
        r"
        SELECT
            b.user_id,
            a.symbol AS asset,
            b.available + b.locked AS total
        FROM balances b
        JOIN assets a ON a.id = b.asset_id
        JOIN users u ON u.id = b.user_id
        WHERE b.available + b.locked > 0
          AND u.status <> 'closed'
          AND u.is_mm = false
        ORDER BY a.symbol, b.user_id
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ProofLiability {
            user_id: row.user_id,
            asset: row.asset,
            total: canonical_decimal(row.total),
        })
        .collect())
}

async fn insert_snapshot(
    pool: &PgPool,
    merkle_root: String,
    liabilities: Vec<ProofLiability>,
) -> Result<ProofSnapshot, sqlx::Error> {
    sqlx::query_as::<_, ProofSnapshotRow>(
        r"
        INSERT INTO proof_of_reserves (merkle_root, liabilities)
        VALUES ($1, $2)
        RETURNING id, merkle_root, liabilities, generated_at
        ",
    )
    .bind(merkle_root)
    .bind(Json(liabilities))
    .fetch_one(pool)
    .await
    .map(snapshot_from_row)
}

fn snapshot_from_row(row: ProofSnapshotRow) -> ProofSnapshot {
    ProofSnapshot {
        id: row.id,
        merkle_root: row.merkle_root,
        liabilities: row.liabilities.0,
        generated_at: row.generated_at,
    }
}

#[derive(Debug, Clone)]
struct MerkleTree {
    leaves: Vec<MerkleLeaf>,
    root: String,
}

#[derive(Debug, Clone)]
struct MerkleLeaf {
    liability: ProofLiability,
    payload: String,
    hash: String,
}

impl MerkleTree {
    fn new(mut liabilities: Vec<ProofLiability>) -> Self {
        liabilities.sort_by(|a, b| {
            a.asset
                .cmp(&b.asset)
                .then_with(|| a.user_id.cmp(&b.user_id))
                .then_with(|| a.total.cmp(&b.total))
        });
        let leaves: Vec<MerkleLeaf> = liabilities
            .into_iter()
            .map(|liability| {
                let payload = leaf_payload(&liability);
                let hash = hash_hex(payload.as_bytes());
                MerkleLeaf {
                    liability,
                    payload,
                    hash,
                }
            })
            .collect();
        let hashes = leaves.iter().map(|leaf| leaf.hash.clone()).collect();
        let root = merkle_root(hashes);
        Self { leaves, root }
    }

    fn entries_for_user(&self, user_id: Uuid) -> Vec<UserProofEntry> {
        let hashes: Vec<String> = self.leaves.iter().map(|leaf| leaf.hash.clone()).collect();
        self.leaves
            .iter()
            .enumerate()
            .filter(|(_, leaf)| leaf.liability.user_id == user_id)
            .map(|(index, leaf)| UserProofEntry {
                asset: leaf.liability.asset.clone(),
                total: leaf.liability.total.clone(),
                leaf_payload: leaf.payload.clone(),
                leaf_hash: leaf.hash.clone(),
                sibling_hashes: proof_for_index(&hashes, index),
            })
            .collect()
    }
}

fn leaf_payload(liability: &ProofLiability) -> String {
    format!(
        "{LEAF_PREFIX}:{}:{}:{}",
        liability.user_id, liability.asset, liability.total
    )
}

fn merkle_root(mut level: Vec<String>) -> String {
    if level.is_empty() {
        return hash_hex(EMPTY_ROOT_PAYLOAD);
    }
    while level.len() > 1 {
        level = next_level(&level);
    }
    level
        .into_iter()
        .next()
        .unwrap_or_else(|| hash_hex(EMPTY_ROOT_PAYLOAD))
}

fn next_level(level: &[String]) -> Vec<String> {
    level
        .chunks(2)
        .map(|pair| match pair {
            [left, right] => hash_pair(left, right),
            [single] => single.clone(),
            _ => hash_hex(EMPTY_ROOT_PAYLOAD),
        })
        .collect()
}

fn proof_for_index(leaves: &[String], mut index: usize) -> Vec<String> {
    let mut proof = Vec::new();
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        let sibling_index = if index.is_multiple_of(2) {
            index.saturating_add(1)
        } else {
            index.saturating_sub(1)
        };
        if let Some(sibling) = level.get(sibling_index) {
            proof.push(sibling.clone());
        }
        level = next_level(&level);
        index /= 2;
    }
    proof
}

fn hash_pair(left: &str, right: &str) -> String {
    let (first, second) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    hash_hex(format!("{INTERNAL_PREFIX}:{first}:{second}").as_bytes())
}

fn hash_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    hex_lower(&digest)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let high = HEX.get(usize::from(byte >> 4)).copied().unwrap_or(b'0');
        let low = HEX.get(usize::from(byte & 0x0f)).copied().unwrap_or(b'0');
        out.push(char::from(high));
        out.push(char::from(low));
    }
    out
}

fn canonical_decimal(value: Decimal) -> String {
    value.normalize().to_string()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn merkle_root_is_deterministic_for_sorted_liabilities() {
        let user_a = Uuid::from_u128(1);
        let user_b = Uuid::from_u128(2);
        let first = vec![
            liability(user_b, "ETH", "2.500000000000000000"),
            liability(user_a, "BTC", "0.100000000000000000"),
        ];
        let second = vec![
            liability(user_a, "BTC", "0.100000000000000000"),
            liability(user_b, "ETH", "2.500000000000000000"),
        ];

        let first_tree = MerkleTree::new(first);
        let second_tree = MerkleTree::new(second);

        assert_eq!(first_tree.root, second_tree.root);
    }

    #[test]
    fn proof_entries_verify_against_snapshot_root() {
        let user_a = Uuid::from_u128(1);
        let user_b = Uuid::from_u128(2);
        let liabilities = vec![
            liability(user_a, "BTC", "0.1"),
            liability(user_a, "USDT", "100"),
            liability(user_b, "ETH", "2.5"),
        ];
        let tree = MerkleTree::new(liabilities.clone());
        let snapshot = ProofSnapshot {
            id: Uuid::from_u128(99),
            merkle_root: tree.root,
            liabilities,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };

        let entries = user_proof_entries(&snapshot, user_a);

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| verify_proof(
            &entry.leaf_payload,
            &entry.sibling_hashes,
            &snapshot.merkle_root
        )));
    }

    #[test]
    fn summaries_group_totals_by_asset() {
        let snapshot = ProofSnapshot {
            id: Uuid::from_u128(99),
            merkle_root: "root".to_owned(),
            liabilities: vec![
                liability(Uuid::from_u128(1), "USDT", "100.500000000000000000"),
                liability(Uuid::from_u128(2), "USDT", "25.250000000000000000"),
                liability(Uuid::from_u128(3), "BTC", "0.100000000000000000"),
            ],
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };

        let summaries = summarize_liabilities(&snapshot);

        assert_eq!(
            summaries,
            vec![
                LiabilitySummary {
                    asset: "BTC".to_owned(),
                    total: "0.1".to_owned()
                },
                LiabilitySummary {
                    asset: "USDT".to_owned(),
                    total: "125.75".to_owned()
                }
            ]
        );
    }

    fn liability(user_id: Uuid, asset: &str, total: &str) -> ProofLiability {
        let decimal = Decimal::from_str(total).unwrap_or(Decimal::ZERO);
        ProofLiability {
            user_id,
            asset: asset.to_owned(),
            total: canonical_decimal(decimal),
        }
    }
}
