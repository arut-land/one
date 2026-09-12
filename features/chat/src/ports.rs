/// IDs are supplied by the composition root. Browsers supply time and entropy.
pub trait IdSource: Send + Sync + 'static {
    fn new_id(&self) -> String;
}
pub struct NativeIds;
impl IdSource for NativeIds {
    fn new_id(&self) -> String {
        #[cfg(not(target_arch = "wasm32"))]
        {
            uuid::Uuid::now_v7().to_string()
        }
        #[cfg(target_arch = "wasm32")]
        {
            panic!("browser composition must supply IdSource")
        }
    }
}
