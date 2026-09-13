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

/// Ordered facts in a stable feature-owned namespace.
pub trait Persist<F: arut_storage::Fact> {
    fn log(
        &self,
        namespace: &str,
    ) -> Result<std::sync::Arc<dyn arut_storage::FactLog<F>>, arut_storage::StorageError>;
}

/// Recovery storage for ephemeral drafts; drafts are never facts.
pub trait Drafts {
    fn drafts(&self) -> std::sync::Arc<dyn arut_storage::KeyValue>;
}

/// Milliseconds since the Unix epoch, supplied by the host or a test clock.
pub trait Clock {
    fn now(&self) -> u64;
}

pub trait ChatRuntime:
    IdSource + Persist<arut_protocol::chat::v1::ChatFact> + Drafts + Clock + Send + Sync + 'static
{
}
impl<
    R: IdSource + Persist<arut_protocol::chat::v1::ChatFact> + Drafts + Clock + Send + Sync + 'static,
> ChatRuntime for R
{
}

#[cfg(test)]
impl Clock for NativeIds {
    fn now(&self) -> u64 {
        123
    }
}
