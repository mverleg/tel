use crate::Error;
use ::std::fmt;
use ::std::hash;

trait InputState: fmt::Debug + PartialEq + Eq {}

/// Query represents a request for a step to be performed, such as
/// reading one file or optimizing one input.
///
/// This query is used as a cache key, so equality & hash should
/// cover anything that may change the answer.
trait Query: fmt::Debug + PartialEq + Eq + hash::Hash {}
//TODO @mark: eq and hash should depend on all dependencies too right?

/// Answer to a `Query`, as given by a `Step`.
///
/// If an answer is the same as given a previous time, then subsequent
/// steps will reuse their cache. So equality & hash should cover everything.
trait Answer: fmt::Debug + PartialEq + Eq {}

/// Step that takes a `Query` and provides an `Answer` for it.
///
/// This is the trait that should be implemented to do the actual logic.
/// All the caching and orchestration is handled outside of this.
//TODO @mark: async
trait Step<Q: Query> {
    type S: InputState;
    type A: Answer;

    /// Once per run, when this step is needed, the current input state is checked.
    ///
    /// If this step depends on any external input, then this should return a summary
    /// of this input, e.g. a file timestamp or hash. If this value changed then the
    /// step is performed, otherwise the cached value may be used.
    ///
    /// If the step does not depend on anything external, as most steps do not, then
    /// state can simply be unit `()`.
    fn input_state(query: Q) -> Self::S;

    /// Perform whatever action is needed to answer the query.
    ///
    /// * If this uses any external input (anything not in the arguments), then
    ///   `input_state` _must_ return a value that changes when the external input
    ///   changes. Otherwise, the Answer will remain cached even if input changes.
    /// * Any number of other queries may be done. The result of this step will remain
    ///
    fn perform(query: Q, state: &Self::S) -> Stat<Self::A>;
}

#[derive(Debug)]
struct Stat<T> {
    value: Result<T, Error>,
    msgs: Vec<String>,
    //TODO @mark: tinyvec
}
