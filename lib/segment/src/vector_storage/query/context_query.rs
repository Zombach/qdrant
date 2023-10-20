use common::types::ScoreType;

use super::discovery_query::DiscoveryPair;
use super::{Query, TransformInto};
use crate::data_types::vectors::{QueryVector, VectorType};

impl<T> DiscoveryPair<T> {
    /// In the first stage of discovery search, the objective is to get the best entry point
    /// for the search. This is done by using a smooth loss function instead of hard ranking
    /// to approach the best zone, once the best zone is reached, score will be same for all
    /// points inside that zone.
    /// e.g.:
    ///                   │
    ///                   │
    ///                   │    +0
    ///                   │             +0
    ///                   │
    ///         n         │         p
    ///                   │
    ///   ─►          ─►  │
    ///  -0.4        -0.1 │   +0
    ///                   │
    ///
    pub fn loss_by(&self, similarity: impl Fn(&T) -> ScoreType) -> ScoreType {
        const MARGIN: ScoreType = ScoreType::EPSILON;

        let positive = similarity(&self.positive);
        let negative = similarity(&self.negative);

        let difference = positive - negative - MARGIN;

        ScoreType::min(difference, 0.0)
    }
}

#[derive(Debug, Clone)]
pub struct ContextQuery<T> {
    pub pairs: Vec<DiscoveryPair<T>>,
}

impl<T> ContextQuery<T> {
    pub fn new(pairs: Vec<DiscoveryPair<T>>) -> Self {
        Self { pairs }
    }

    pub fn flat_iter(&self) -> impl Iterator<Item = &T> {
        self.pairs.iter().flat_map(|pair| pair.iter())
    }
}

impl<T, U> TransformInto<ContextQuery<U>, T, U> for ContextQuery<T> {
    fn transform<F>(self, mut f: F) -> ContextQuery<U>
    where
        F: FnMut(T) -> U,
    {
        ContextQuery::new(
            self.pairs
                .into_iter()
                .map(|pair| pair.transform(&mut f))
                .collect(),
        )
    }
}

impl<T> Query<T> for ContextQuery<T> {
    fn score_by(&self, similarity: impl Fn(&T) -> ScoreType) -> ScoreType {
        self.pairs
            .iter()
            .map(|pair| pair.loss_by(&similarity))
            .sum()
    }
}

impl From<ContextQuery<VectorType>> for QueryVector {
    fn from(query: ContextQuery<VectorType>) -> Self {
        QueryVector::Context(query)
    }
}

#[cfg(test)]
mod test {

    use common::types::ScoreType;
    use rstest::rstest;

    use super::*;

    fn dummy_similarity(x: &i32) -> ScoreType {
        *x as ScoreType
    }

    /// Test that the score is calculated correctly
    ///
    /// for reference:
    #[rstest]
    #[case::no_pairs(vec![], 0.0)] // having no input always scores 0
    #[case::on_negative(vec![(0, 1)], -1.0)]
    #[case::on_positive(vec![(1, 0)], 0.0)]
    #[case::on_both(vec![(1, 0), (0, 1)], -1.0)]
    #[case::positive_positive_negative(vec![(1,0),(1,0),(0,1)], -1.0)]
    #[case::positive_negative_negative(vec![(1,0),(0,1),(0,1)], -2.0)]
    #[case::only_positives(vec![(2,-1),(-1,-3),(4,0)], 0.0)]
    #[case::only_negatives(vec![(-5,-4),(-1,3),(0,2)], -7.0)]
    fn scoring(#[case] pairs: Vec<(i32, i32)>, #[case] expected: f32) {
        let pairs = pairs.into_iter().map(DiscoveryPair::from).collect();

        let query = ContextQuery::new(pairs);

        let score = query.score_by(dummy_similarity);

        assert!(
            score > expected - 0.00001 && score < expected + 0.00001,
            "score: {}, expected: {}",
            score,
            expected
        );
    }
}