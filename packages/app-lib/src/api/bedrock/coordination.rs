//! All local Bedrock mutations and official launch/import requests share one lock.
use super::{BedrockError, ErrorCode, Result};
use std::sync::{Mutex, MutexGuard};
static MUTATION: Mutex<()> = Mutex::new(());
pub(super) fn mutation() -> Result<MutexGuard<'static, ()>> {
    MUTATION
        .lock()
        .map_err(|e| BedrockError::new(ErrorCode::DataUnavailable, e))
}
