use anyhow::{bail, Result};
use instant_distance::{Builder, HnswMap, Search};
use serde::{Deserialize, Serialize};

/// A newtype wrapper around a dense float vector that implements
/// `instant_distance::Point` using cosine distance.
#[derive(Clone, Debug)]
pub struct Point(pub Vec<f32>);

impl instant_distance::Point for Point {
    fn distance(&self, other: &Self) -> f32 {
        // Cosine distance = 1 - cosine_similarity
        // Both vectors are assumed to be L2-normalised, so dot product == cosine similarity.
        let dot: f32 = self
            .0
            .iter()
            .zip(other.0.iter())
            .map(|(a, b)| a * b)
            .sum();
        // Clamp to [-1, 1] to avoid floating-point drift outside that range.
        1.0 - dot.clamp(-1.0, 1.0)
    }
}

/// Serializable snapshot entry for persistence.
#[derive(Serialize, Deserialize)]
struct SnapshotEntry {
    region_id: i64,
    vector: Vec<f32>,
}

/// Serializable snapshot for persistence.
#[derive(Serialize, Deserialize)]
struct Snapshot {
    dimension: usize,
    entries: Vec<SnapshotEntry>,
}

/// An HNSW-backed approximate nearest-neighbour index over region embeddings.
///
/// Usage pattern:
/// 1. `add()` vectors into the staging area.
/// 2. `build()` to construct the HNSW graph.
/// 3. `search()` to query.
pub struct VectorIndex {
    map: Option<HnswMap<Point, i64>>,
    dimension: usize,
    /// Staging area: (vector, region_id) pairs accumulated before `build()`.
    points: Vec<(Point, i64)>,
    /// Snapshot for persistence: (vector, region_id) pairs.
    snapshot: Vec<(Vec<f32>, i64)>,
}

impl VectorIndex {
    /// Create a new, empty index for vectors of the given dimensionality.
    pub fn new(dimension: usize) -> Self {
        Self {
            map: None,
            dimension,
            points: Vec::new(),
            snapshot: Vec::new(),
        }
    }

    /// Returns `true` if no vectors have been added yet.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Stage a (region_id, vector) pair for inclusion in the next `build()` call.
    pub fn add(&mut self, region_id: i64, vector: Vec<f32>) -> Result<()> {
        if vector.len() != self.dimension {
            bail!(
                "vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            );
        }
        self.snapshot.push((vector.clone(), region_id));
        self.points.push((Point(vector), region_id));
        Ok(())
    }

    /// Construct the HNSW graph from all staged points.
    ///
    /// Must be called before `search()`. Staged points are kept so that
    /// incremental rebuilds are possible.
    pub fn build(&mut self) -> Result<()> {
        if self.points.is_empty() {
            self.map = None;
            return Ok(());
        }

        let (pts, values): (Vec<Point>, Vec<i64>) =
            self.points.iter().cloned().unzip();

        let map = Builder::default().build(pts, values);
        self.map = Some(map);
        Ok(())
    }

    /// Search for the `k` nearest neighbours to `query`.
    ///
    /// Returns `(region_id, similarity)` pairs sorted by descending similarity
    /// (i.e., ascending cosine distance). Returns an empty vec if the index has
    /// not been built or contains no points.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(i64, f32)>> {
        if query.len() != self.dimension {
            bail!(
                "query dimension mismatch: expected {}, got {}",
                self.dimension,
                query.len()
            );
        }

        let map = match &self.map {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        let q = Point(query.to_vec());
        let mut search = Search::default();
        use instant_distance::Point as _;
        let results: Vec<(i64, f32)> = map
            .search(&q, &mut search)
            .take(k)
            .map(|item| {
                let distance = q.distance(item.point);
                let similarity = 1.0 - distance;
                (*item.value, similarity)
            })
            .collect();

        Ok(results)
    }

    /// Persist the index to `dir` as `vectors.json`.
    pub fn save(&self, dir: &std::path::Path) -> Result<()> {
        let entries: Vec<SnapshotEntry> = self
            .snapshot
            .iter()
            .map(|(vec, rid)| SnapshotEntry {
                region_id: *rid,
                vector: vec.clone(),
            })
            .collect();

        let snapshot = Snapshot {
            dimension: self.dimension,
            entries,
        };

        let path = dir.join("vectors.json");
        let json = serde_json::to_string(&snapshot)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load an index from `dir/vectors.json`, rebuild it, and return it.
    pub fn load(dir: &std::path::Path, dimension: usize) -> Result<Self> {
        let path = dir.join("vectors.json");
        if !path.exists() {
            return Ok(Self::new(dimension));
        }

        let json = std::fs::read_to_string(&path)?;
        let snapshot: Snapshot = serde_json::from_str(&json)?;

        let mut index = Self::new(snapshot.dimension);
        for entry in snapshot.entries {
            index.add(entry.region_id, entry.vector)?;
        }
        index.build()?;
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_vec(dim: usize, hot: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[hot] = 1.0;
        v
    }

    #[test]
    fn empty_index_returns_no_results() {
        let mut idx = VectorIndex::new(4);
        idx.build().unwrap();
        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn add_and_search_returns_nearest() {
        let dim = 8;
        let mut idx = VectorIndex::new(dim);

        // Three orthogonal unit vectors as "embeddings".
        idx.add(10, unit_vec(dim, 0)).unwrap(); // region_id = 10
        idx.add(20, unit_vec(dim, 1)).unwrap(); // region_id = 20
        idx.add(30, unit_vec(dim, 2)).unwrap(); // region_id = 30

        idx.build().unwrap();

        // Query closest to region 20 (unit vector at index 1).
        let results = idx.search(&unit_vec(dim, 1), 3).unwrap();
        assert!(!results.is_empty(), "should return at least one result");

        // The best match must be region 20 (cosine similarity = 1.0).
        let best = results[0];
        assert_eq!(best.0, 20, "nearest neighbour should be region 20");
        assert!(
            (best.1 - 1.0).abs() < 1e-5,
            "similarity of identical vectors should be ~1.0, got {}",
            best.1
        );

        // Results should be in descending similarity order.
        for w in results.windows(2) {
            assert!(
                w[0].1 >= w[1].1,
                "results should be sorted by descending similarity"
            );
        }
    }

    #[test]
    fn dimension_mismatch_is_rejected() {
        let mut idx = VectorIndex::new(4);
        let err = idx.add(1, vec![1.0, 0.0]).unwrap_err();
        assert!(err.to_string().contains("dimension mismatch"));
    }

    #[test]
    fn is_empty_reflects_added_vectors() {
        let mut idx = VectorIndex::new(4);
        assert!(idx.is_empty(), "new index should be empty");
        idx.add(1, unit_vec(4, 0)).unwrap();
        assert!(!idx.is_empty(), "index with a vector should not be empty");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let dim = 8;
        let mut idx = VectorIndex::new(dim);
        idx.add(10, unit_vec(dim, 0)).unwrap();
        idx.add(20, unit_vec(dim, 1)).unwrap();
        idx.add(30, unit_vec(dim, 2)).unwrap();
        idx.build().unwrap();

        idx.save(dir.path()).unwrap();

        let loaded = VectorIndex::load(dir.path(), dim).unwrap();
        let results = loaded.search(&unit_vec(dim, 1), 3).unwrap();
        assert!(!results.is_empty(), "loaded index should return results");
        assert_eq!(results[0].0, 20, "nearest neighbour should be region 20 after load");
    }
}
