use thiserror::Error;

#[derive(Debug, Error)]
pub enum WikiwalkError {
  #[error("database error: {0}")]
  DatabaseError(String),
}
