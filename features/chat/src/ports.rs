/// IDs are supplied by the composition root. Browsers supply time and entropy.
pub trait IdSource: Send + Sync + 'static {
    fn new_id(&self) -> String;
}

#[cfg(test)]
/// Available only where the platform has a clock and entropy of its own.
pub(crate) struct NativeIds;
#[cfg(test)]
impl IdSource for NativeIds {
    fn new_id(&self) -> String {
        uuid::Uuid::now_v7().to_string()
    }
}
