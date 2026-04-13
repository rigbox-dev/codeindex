use std::collections::HashMap;

/// Reciprocal Rank Fusion over multiple ranked lists.
///
/// Each input list is a `Vec<(region_id, score)>` **already sorted by
/// descending score** (rank 0 = best).  The RRF formula used is:
///
/// ```text
/// rrf_score(region) = Σ_list  1 / (k + rank + 1)
/// ```
///
/// where `k = 60` is the standard smoothing constant.
///
/// Returns a merged list sorted by **descending** fused score.
pub fn reciprocal_rank_fusion(lists: &[Vec<(i64, f64)>]) -> Vec<(i64, f64)> {
    const K: f64 = 60.0;

    let mut scores: HashMap<i64, f64> = HashMap::new();

    for list in lists {
        for (rank, &(region_id, _score)) in list.iter().enumerate() {
            let rrf = 1.0 / (K + rank as f64 + 1.0);
            *scores.entry(region_id).or_insert(0.0) += rrf;
        }
    }

    let mut merged: Vec<(i64, f64)> = scores.into_iter().collect();
    // Sort descending by fused score; break ties by region_id for determinism.
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_list_preserves_order() {
        let list = vec![(1, 0.9), (2, 0.7), (3, 0.5)];
        let fused = reciprocal_rank_fusion(&[list]);
        // Region 1 was rank 0, so it should have the highest RRF score.
        assert_eq!(fused[0].0, 1);
        assert_eq!(fused[1].0, 2);
        assert_eq!(fused[2].0, 3);
    }

    #[test]
    fn overlapping_items_get_boosted() {
        // Region 2 appears in both lists (once at rank 0, once at rank 1).
        // Region 1 appears only in list A at rank 1.
        // Region 3 appears only in list B at rank 0.
        let list_a = vec![(1, 0.9), (2, 0.8)];
        let list_b = vec![(3, 0.9), (2, 0.8)];
        let fused = reciprocal_rank_fusion(&[list_a, list_b]);

        // Find positions
        let pos = |id: i64| fused.iter().position(|(r, _)| *r == id).unwrap();

        // Region 2 appears in both lists, so its fused score is the sum of two
        // RRF contributions; it should beat either single-list item.
        assert!(
            pos(2) < pos(1),
            "region 2 (in both lists) should rank above region 1 (single list)"
        );
        assert!(
            pos(2) < pos(3),
            "region 2 (in both lists) should rank above region 3 (single list)"
        );
    }

    #[test]
    fn empty_lists_return_empty() {
        let fused = reciprocal_rank_fusion(&[]);
        assert!(fused.is_empty());

        let fused2 = reciprocal_rank_fusion(&[vec![], vec![]]);
        assert!(fused2.is_empty());
    }
}
