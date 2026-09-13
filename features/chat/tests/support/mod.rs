pub(crate) struct NativeIds;
impl arut_feature_chat::ports::IdSource for NativeIds {
    fn new_id(&self) -> String {
        uuid::Uuid::now_v7().to_string()
    }
}
