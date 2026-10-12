//! Errors raised by the pure domain model.

/// A state machine refused a transition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{entity} {id} cannot move from {from} to {to}")]
pub struct TransitionError {
    pub entity: &'static str,
    pub id: String,
    pub from: String,
    pub to: String,
}
