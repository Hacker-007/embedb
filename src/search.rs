/// A single result returned by a search operation.
#[derive(Debug, PartialEq)]
pub struct SearchResult {
    /// The user-provided label of the matching vector.
    pub label: String,
    /// A score indicating how relevant this result is to the query.
    pub relevance: f64,
}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.relevance.partial_cmp(&other.relevance)
    }
}

impl Eq for SearchResult {}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        assert!(!self.relevance.is_nan() && !other.relevance.is_nan());

        // We compare in reverse order so we remove results that
        // are the least relevant.
        other
            .relevance
            .partial_cmp(&self.relevance)
            .expect("comparing relevances should succeed")
    }
}
