use core::future::Future;
use core::pin::Pin;
use futures_core::Stream;
use futures_util::StreamExt;
use prost::Message;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

pub type RpcFuture<T> = Pin<Box<dyn Future<Output = Result<T, Status>> + Send + 'static>>;
pub type RpcStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send + 'static>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata(BTreeMap<String, Vec<u8>>);

impl Metadata {
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<Vec<u8>>) {
        self.0.insert(name.into(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.0.get(name).map(Vec::as_slice)
    }
}

#[derive(Debug)]
pub struct Request<T> {
    pub message: T,
    pub metadata: Metadata,
}

impl<T> Request<T> {
    pub fn new(message: T) -> Self {
        Self {
            message,
            metadata: Metadata::default(),
        }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Request<U> {
        Request {
            message: map(self.message),
            metadata: self.metadata,
        }
    }
}

impl<T, E> Request<Result<T, E>> {
    pub fn transpose(self) -> Result<Request<T>, E> {
        Ok(Request {
            message: self.message?,
            metadata: self.metadata,
        })
    }
}

#[derive(Debug)]
pub struct Response<T> {
    pub message: T,
    pub metadata: Metadata,
}

impl<T> Response<T> {
    pub fn new(message: T) -> Self {
        Self {
            message,
            metadata: Metadata::default(),
        }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Response<U> {
        Response {
            message: map(self.message),
            metadata: self.metadata,
        }
    }
}

impl<T, E> Response<Result<T, E>> {
    pub fn transpose(self) -> Result<Response<T>, E> {
        Ok(Response {
            message: self.message?,
            metadata: self.metadata,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Cancelled,
    InvalidArgument,
    DeadlineExceeded,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Internal,
    Unavailable,
    Unauthenticated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub code: Code,
    pub message: String,
    pub details: Vec<u8>,
}

impl Status {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(Code::InvalidArgument, message)
    }

    pub fn unimplemented(procedure: &str) -> Self {
        Self::new(
            Code::Unimplemented,
            format!("unknown procedure {procedure}"),
        )
    }
}

impl core::fmt::Display for Status {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for Status {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingKind {
    Unary,
    Server,
    Client,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodDescriptor {
    pub name: &'static str,
    pub procedure: &'static str,
    pub input: &'static str,
    pub output: &'static str,
    pub streaming: StreamingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceDescriptor {
    pub name: &'static str,
    pub package: &'static str,
    pub version: &'static str,
    pub methods: &'static [MethodDescriptor],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRuntimeMetadata {
    pub available: bool,
    pub unavailable_reason: String,
    pub permissions: Vec<String>,
    pub limits: BTreeMap<String, u64>,
    pub extensions: BTreeMap<String, String>,
}

impl Default for ServiceRuntimeMetadata {
    fn default() -> Self {
        Self {
            available: true,
            unavailable_reason: String::new(),
            permissions: Vec::new(),
            limits: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ServiceMetadata {
    inner: Arc<RwLock<ServiceRuntimeMetadata>>,
}

impl ServiceMetadata {
    pub fn new(metadata: ServiceRuntimeMetadata) -> Self {
        Self {
            inner: Arc::new(RwLock::new(metadata)),
        }
    }

    pub fn get(&self) -> ServiceRuntimeMetadata {
        self.inner
            .read()
            .expect("service metadata lock poisoned")
            .clone()
    }

    pub fn set(&self, metadata: ServiceRuntimeMetadata) {
        *self.inner.write().expect("service metadata lock poisoned") = metadata;
    }
}

#[derive(Clone)]
pub struct ServiceRegistration {
    pub descriptor: &'static ServiceDescriptor,
    pub metadata: ServiceMetadata,
}

impl ServiceRegistration {
    pub fn new(descriptor: &'static ServiceDescriptor, metadata: ServiceMetadata) -> Self {
        Self {
            descriptor,
            metadata,
        }
    }
}

pub trait RpcChannel: Send + Sync + 'static {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>>;

    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>>;

    fn client_stream(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>>;

    fn bidirectional(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>>;
}

pub trait RpcService: RpcChannel {
    fn descriptor(&self) -> &'static ServiceDescriptor;
}

#[derive(Default)]
pub struct RpcRegistry {
    routes: HashMap<&'static str, Arc<dyn RpcService>>,
    services: Vec<(Arc<dyn RpcService>, ServiceMetadata)>,
}

impl RpcRegistry {
    pub fn register(self, service: Arc<dyn RpcService>) -> Result<Self, Status> {
        self.register_with_metadata(service, ServiceMetadata::default())
    }

    pub fn register_with_metadata(
        mut self,
        service: Arc<dyn RpcService>,
        metadata: ServiceMetadata,
    ) -> Result<Self, Status> {
        for method in service.descriptor().methods {
            if self
                .routes
                .insert(method.procedure, service.clone())
                .is_some()
            {
                return Err(Status::new(
                    Code::AlreadyExists,
                    format!("procedure {} is already registered", method.procedure),
                ));
            }
        }
        self.services.push((service, metadata));
        Ok(self)
    }

    pub fn registrations(&self) -> impl ExactSizeIterator<Item = ServiceRegistration> + '_ {
        self.services
            .iter()
            .map(|(service, metadata)| ServiceRegistration {
                descriptor: service.descriptor(),
                metadata: metadata.clone(),
            })
    }

    fn route(&self, procedure: &str) -> Result<&Arc<dyn RpcService>, Status> {
        self.routes
            .get(procedure)
            .ok_or_else(|| Status::unimplemented(procedure))
    }
}

impl RpcChannel for RpcRegistry {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        match self.route(procedure) {
            Ok(service) => service.unary(procedure, request),
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }

    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        match self.route(procedure) {
            Ok(service) => service.server_stream(procedure, request),
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }

    fn client_stream(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        match self.route(procedure) {
            Ok(service) => service.client_stream(procedure, request),
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }

    fn bidirectional(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        match self.route(procedure) {
            Ok(service) => service.bidirectional(procedure, request),
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }
}

pub fn encode<T: Message>(message: T) -> Vec<u8> {
    message.encode_to_vec()
}

pub fn decode<T: Message + Default>(bytes: &[u8]) -> Result<T, Status> {
    T::decode(bytes).map_err(|error| Status::invalid_argument(error.to_string()))
}

pub fn map_stream<T, U>(
    stream: RpcStream<T>,
    map: impl Fn(T) -> Result<U, Status> + Send + Sync + 'static,
) -> RpcStream<U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    let map = Arc::new(map);
    Box::pin(stream.map(move |item| item.and_then(|item| map(item))))
}

/// Runs work on an executor owned by the composition root.
pub trait Spawner: Send + Sync + 'static {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyService;

    impl RpcChannel for EmptyService {
        fn unary(
            &self,
            procedure: &str,
            _request: Request<Vec<u8>>,
        ) -> RpcFuture<Response<Vec<u8>>> {
            let error = Status::unimplemented(procedure);
            Box::pin(async move { Err(error) })
        }

        fn server_stream(
            &self,
            procedure: &str,
            _request: Request<Vec<u8>>,
        ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
            let error = Status::unimplemented(procedure);
            Box::pin(async move { Err(error) })
        }

        fn client_stream(
            &self,
            procedure: &str,
            _request: Request<RpcStream<Vec<u8>>>,
        ) -> RpcFuture<Response<Vec<u8>>> {
            let error = Status::unimplemented(procedure);
            Box::pin(async move { Err(error) })
        }

        fn bidirectional(
            &self,
            procedure: &str,
            _request: Request<RpcStream<Vec<u8>>>,
        ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
            let error = Status::unimplemented(procedure);
            Box::pin(async move { Err(error) })
        }
    }

    static METHODS: &[MethodDescriptor] = &[MethodDescriptor {
        name: "Call",
        procedure: "/test.Service/Call",
        input: "test.Request",
        output: "test.Response",
        streaming: StreamingKind::Unary,
    }];
    static DESCRIPTOR: ServiceDescriptor = ServiceDescriptor {
        name: "Service",
        package: "test",
        version: "test",
        methods: METHODS,
    };

    impl RpcService for EmptyService {
        fn descriptor(&self) -> &'static ServiceDescriptor {
            &DESCRIPTOR
        }
    }

    #[test]
    fn rejects_duplicate_procedure_registration() {
        let metadata = ServiceMetadata::default();
        let registry = RpcRegistry::default()
            .register_with_metadata(Arc::new(EmptyService), metadata.clone())
            .unwrap();
        metadata.set(ServiceRuntimeMetadata {
            available: false,
            unavailable_reason: "offline".into(),
            ..ServiceRuntimeMetadata::default()
        });
        assert_eq!(
            registry.registrations().next().unwrap().metadata.get(),
            ServiceRuntimeMetadata {
                available: false,
                unavailable_reason: "offline".into(),
                ..ServiceRuntimeMetadata::default()
            }
        );
        let error = registry.register(Arc::new(EmptyService)).err().unwrap();
        assert_eq!(error.code, Code::AlreadyExists);
    }
}
