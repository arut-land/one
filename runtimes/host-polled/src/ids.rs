//! Native clock and entropy implementation of the feature ID port.
use arut_feature_chat::ports::IdSource;
/// Available only where the platform has a clock and entropy of its own.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeIds;
#[cfg(not(target_arch = "wasm32"))]
impl IdSource for NativeIds {
    fn new_id(&self) -> String {
        uuid::Uuid::now_v7().to_string()
    }
}
