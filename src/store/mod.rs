use anyhow::Result;
use qdrant_client::qdrant::{
    value::Kind, CreateCollectionBuilder, Distance, PointStruct, ScoredPoint,
    SearchPointsBuilder, UpsertPointsBuilder, Value, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use std::collections::HashMap;

pub const COLLECTION: &str = "docs";

pub struct Store {
    client: Qdrant,
}

pub struct Chunk {
    pub id: u64,
    pub text: String,
    pub source: String,
    pub vector: Vec<f32>,
}

pub struct SearchHit {
    pub text: String,
    pub source: String,
    pub score: f32,
}

impl Store {
    pub async fn is_healthy(&self) -> bool {
        self.client.health_check().await.is_ok()
    }

    pub fn connect(url: &str) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;
        Ok(Self { client })
    }

    pub async fn ensure_collection(&self, dim: u64) -> Result<()> {
        if !self.client.collection_exists(COLLECTION).await? {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(COLLECTION)
                        .vectors_config(VectorParamsBuilder::new(dim, Distance::Cosine)),
                )
                .await?;
        }
        Ok(())
    }

    pub async fn upsert_chunks(&self, chunks: Vec<Chunk>) -> Result<()> {
        let points: Vec<PointStruct> = chunks
            .into_iter()
            .map(|c| {
                let mut payload: HashMap<String, Value> = HashMap::new();
                payload.insert("text".to_string(), c.text.into());
                payload.insert("source".to_string(), c.source.into());
                PointStruct::new(c.id, c.vector, payload)
            })
            .collect();
        self.client
            .upsert_points(UpsertPointsBuilder::new(COLLECTION, points))
            .await?;
        Ok(())
    }

    pub async fn search(&self, vector: Vec<f32>, top_k: u64) -> Result<Vec<SearchHit>> {
        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(COLLECTION, vector, top_k).with_payload(true),
            )
            .await?;
        Ok(results.result.into_iter().map(to_hit).collect())
    }
}

fn to_hit(point: ScoredPoint) -> SearchHit {
    let text = get_str(&point, "text");
    let source = get_str(&point, "source");
    SearchHit {
        text,
        source,
        score: point.score,
    }
}

fn get_str(point: &ScoredPoint, key: &str) -> String {
    point
        .payload
        .get(key)
        .and_then(|v| v.kind.clone())
        .map(|k| match k {
            Kind::StringValue(s) => s,
            other => format!("{other:?}"),
        })
        .unwrap_or_default()
}
