//! # RPC
//!
//! The transport-neutral vocabulary generated service code and every channel
//! share: requests, responses, metadata, typed statuses, streams, the
//! object-safe `RpcChannel`, and the registry that routes procedures to
//! services. Nothing here knows a wire format or a product rule.
//!
//! Dispatch is instrumented: every routed procedure runs inside a span naming
//! the procedure and its streaming kind, and an unroutable one records why. No
//! request or response bytes are ever recorded, and no crate here installs a
//! subscriber -- that is the composition root's call.
//!
//! It also holds the two execution ports a composition root supplies, because
//! they are what a channel's callers need and no library crate may create an
//! executor of its own: `Spawner` and `LocalSpawner` run futures, and
//! `Cancellation` is the per-scope token tree that stops them.

use core::future::Future;
use core::pin::Pin;
use futures_core::Stream;
use futures_util::StreamExt;
use prost::Message;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
pub use tokio_util::sync::CancellationToken;
use tokio_util::sync::DropGuard;
use tracing::Instrument;

pub type RpcFuture<T> = Pin<Box<dyn Future<Output = Result<T, Status>> + Send + 'static>>;
pub type RpcStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send + 'static>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata(BTreeMap<String, Vec<u8>>);

impl Metadata {
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }
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

/// An I/O failure is a transport that stopped answering, which is what
/// `Unavailable` names. `tokio_util::codec::Decoder` requires this conversion
/// of every decoder error type, and transports want it for their own reads.
impl From<std::io::Error> for Status {
    fn from(error: std::io::Error) -> Self {
        Self::new(Code::Unavailable, error.to_string())
    }
}

/// A typed reason carried in [`Status::details`] as a single tag byte.
///
/// The tag is the variant's index in `ALL`, so one const drives the wire byte,
/// `Display`, and the locale test. Both ends of a `Status` carrying one come
/// from the same build, so the index is stable where it is read; a tag this
/// binary does not know about reads back as `None` and the caller falls back to
/// [`Self::CODE`] and the message.
pub trait StatusDetail: Copy + PartialEq + Sized + 'static {
    /// The coarse code a caller that does not know this type still handles.
    const CODE: Code;
    /// Every variant, in tag order.
    const ALL: &'static [Self];

    /// Wrap this reason in a `Status`.
    #[must_use]
    fn into_status(self, message: impl Into<String>) -> Status {
        let tag = Self::ALL
            .iter()
            .position(|variant| *variant == self)
            .expect("every variant of a StatusDetail is listed in ALL");
        Status {
            code: Self::CODE,
            message: message.into(),
            details: vec![u8::try_from(tag).expect("a StatusDetail has at most 256 variants")],
        }
    }

    /// The typed reason behind `status`, if it carries one this build knows.
    #[must_use]
    fn from_status(status: &Status) -> Option<Self> {
        if status.code != Self::CODE {
            return None;
        }
        match status.details.as_slice() {
            [tag] => Self::ALL.get(usize::from(*tag)).copied(),
            _ => None,
        }
    }
}

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

/// A package version, as ADR 0015's two-minor window compares them.
///
/// `major` is the Protobuf package suffix (`arut.chat.v1` is major 1). There is
/// no source of `minor` yet -- the generator emits 0 until a proto option or a
/// generator table supplies one, which is the prerequisite for the negotiation
/// in ROADMAP Phase 1 item 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

/// How many minor versions apart a session and a node may be (ADR 0015).
pub const COMPATIBILITY_WINDOW: u32 = 2;

/// Whether a session at `local` may talk to a node at `remote` (ADR 0015).
#[must_use]
pub const fn compatible(local: Version, remote: Version) -> bool {
    local.major == remote.major && local.minor.abs_diff(remote.minor) <= COMPATIBILITY_WINDOW
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceDescriptor {
    pub name: &'static str,
    pub package: &'static str,
    pub version: Version,
    pub methods: &'static [MethodDescriptor],
}

/// One generated marker type per service, carrying its descriptor as a const so
/// a lookup names the service by type rather than by a hand-written string.
pub trait Service: 'static {
    const DESCRIPTOR: ServiceDescriptor;
}

/// The services a node serves, named as a tuple of [`Service`] markers.
///
/// A feature declares its own set beside its composition function; a manifest
/// is then derived from the type, with no registration table to walk and no
/// order to sort (the tuple is the order).
pub trait ServiceSet {
    const DESCRIPTORS: &'static [ServiceDescriptor];
}

macro_rules! service_set {
    ($($name:ident),+) => {
        impl<$($name: Service),+> ServiceSet for ($($name,)+) {
            const DESCRIPTORS: &'static [ServiceDescriptor] =
                &[$(<$name as Service>::DESCRIPTOR),+];
        }
    };
}
service_set!(A);
service_set!(A, B);
service_set!(A, B, C);
service_set!(A, B, C, D);
service_set!(A, B, C, D, E);
service_set!(A, B, C, D, E, F);
service_set!(A, B, C, D, E, F, G);
service_set!(A, B, C, D, E, F, G, H);

/// All four Protobuf streaming shapes (ADR 0008), of which a channel implements
/// the ones its wire can carry.
///
/// The two request-streaming shapes default to `Unimplemented` because no
/// transport in the tree carries them: Connect over HTTP cannot, and the
/// registry only fans out to services that also cannot. The iroh
/// bi-directional stream of ADR 0019 arrives as one `bidirectional` override,
/// not as a change here.
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
        _request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        let status = Status::unimplemented(procedure);
        Box::pin(async move { Err(status) })
    }

    fn bidirectional(
        &self,
        procedure: &str,
        _request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let status = Status::unimplemented(procedure);
        Box::pin(async move { Err(status) })
    }
}

/// What a layer does around a call: schedule it, offload it, or attach the
/// metadata ADR 0009 keeps out of feature code.
///
/// `Wrapped<W>` is monomorphised over the impl, so a generic method is fine
/// here and `Wrapped<W>: RpcChannel` is still object-safe.
pub trait Wrap: Send + Sync + 'static {
    fn wrap<T: Send + 'static>(
        &self,
        call: Box<dyn FnOnce() -> RpcFuture<T> + Send>,
    ) -> RpcFuture<T>;
}

/// One channel behind one [`Wrap`].
pub struct Wrapped<W> {
    inner: Arc<dyn RpcChannel>,
    wrap: W,
}

impl<W> Wrapped<W> {
    pub fn new(inner: Arc<dyn RpcChannel>, wrap: W) -> Self {
        Self { inner, wrap }
    }
}

impl<W: Wrap> RpcChannel for Wrapped<W> {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        let (inner, procedure) = (self.inner.clone(), procedure.to_owned());
        self.wrap
            .wrap(Box::new(move || inner.unary(&procedure, request)))
    }

    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let (inner, procedure) = (self.inner.clone(), procedure.to_owned());
        self.wrap
            .wrap(Box::new(move || inner.server_stream(&procedure, request)))
    }
}

pub trait RpcService: RpcChannel {
    fn descriptor(&self) -> &'static ServiceDescriptor;
}

#[derive(Default)]
pub struct RpcRegistry {
    routes: HashMap<&'static str, Arc<dyn RpcService>>,
}

impl RpcRegistry {
    /// # Errors
    /// Returns `AlreadyExists` if another service already claims a procedure.
    pub fn register(mut self, service: Arc<dyn RpcService>) -> Result<Self, Status> {
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
        Ok(self)
    }

    fn route(&self, procedure: &str) -> Result<&Arc<dyn RpcService>, Status> {
        self.routes
            .get(procedure)
            .ok_or_else(|| Status::unimplemented(procedure))
    }
}

/// One span per dispatched call, naming the procedure and never its bytes.
fn dispatch(kind: &'static str, procedure: &str) -> tracing::Span {
    tracing::debug_span!("rpc.dispatch", kind, procedure)
}

/// Records the miss inside the same span shape a routed call would have used.
fn unroutable<T: Send + 'static>(span: &tracing::Span, error: Status) -> RpcFuture<T> {
    span.in_scope(|| tracing::debug!(code = ?error.code, "no service claims this procedure"));
    Box::pin(async move { Err(error) })
}

impl RpcChannel for RpcRegistry {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        let span = dispatch("unary", procedure);
        match self.route(procedure) {
            Ok(service) => Box::pin(service.unary(procedure, request).instrument(span)),
            Err(error) => unroutable(&span, error),
        }
    }

    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let span = dispatch("server_stream", procedure);
        match self.route(procedure) {
            Ok(service) => Box::pin(service.server_stream(procedure, request).instrument(span)),
            Err(error) => unroutable(&span, error),
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
    Box::pin(stream.map(move |item| item.and_then(&map)))
}

/// Runs work on an executor owned by the composition root.
pub trait Spawner: Send + Sync + 'static {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>);
}

/// Runs work on an executor the host polls itself, with no thread behind it and
/// no `Send` bound on the future: the page's event loop reached through
/// BoltFFI's poll exports, or an Android foreground-service thread.
pub trait LocalSpawner: 'static {
    fn spawn_local(&self, future: Pin<Box<dyn Future<Output = ()> + 'static>>);
}

/// One scope's cancellation, cancelled when the last holder of the scope drops.
///
/// A child scope's token is a `child_token()` of its parent's, so cancelling a
/// workspace cancels every conversation and operation constructed under it and
/// touches nothing above it. Scopes hold this rather than a bare token so the
/// tree needs no bookkeeping: the `DropGuard` inside fires with the scope.
pub struct Cancellation {
    guard: DropGuard,
}

impl Cancellation {
    /// The root of a token tree, owned by a composition root.
    pub fn root() -> Self {
        Self::from(CancellationToken::new())
    }

    /// A token cancelled by this scope, by its own drop, or by either parent.
    pub fn child(&self) -> Self {
        Self::from(self.guard.token().child_token())
    }

    /// The token to hand to work that must stop with this scope.
    pub fn token(&self) -> CancellationToken {
        self.guard.token().clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.guard.token().is_cancelled()
    }

    /// Cancels this scope and everything under it, before it is dropped.
    pub fn cancel(&self) {
        self.guard.token().cancel();
    }
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::root()
    }
}

impl From<CancellationToken> for Cancellation {
    fn from(token: CancellationToken) -> Self {
        Self {
            guard: token.drop_guard(),
        }
    }
}

impl std::fmt::Debug for Cancellation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Cancellation")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
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
        package: "test.v1",
        version: Version { major: 1, minor: 0 },
        methods: METHODS,
    };

    impl RpcService for EmptyService {
        fn descriptor(&self) -> &'static ServiceDescriptor {
            &DESCRIPTOR
        }
    }

    struct ServiceId;
    impl Service for ServiceId {
        const DESCRIPTOR: ServiceDescriptor = DESCRIPTOR;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Reason {
        First,
        Second,
    }
    impl StatusDetail for Reason {
        const CODE: Code = Code::Unavailable;
        const ALL: &'static [Self] = &[Self::First, Self::Second];
    }

    #[test]
    fn rejects_duplicate_procedure_registration() {
        let registry = RpcRegistry::default()
            .register(Arc::new(EmptyService))
            .unwrap();
        let error = registry.register(Arc::new(EmptyService)).err().unwrap();
        assert_eq!(error.code, Code::AlreadyExists);
    }

    #[test]
    fn a_request_streaming_call_is_unimplemented_unless_a_channel_carries_it() {
        let empty: RpcStream<Vec<u8>> = Box::pin(futures_util::stream::empty());
        let error = futures_executor::block_on(
            EmptyService.client_stream("/test.Service/Call", Request::new(empty)),
        )
        .unwrap_err();
        assert_eq!(error.code, Code::Unimplemented);
    }

    #[test]
    fn a_service_set_lists_its_members_descriptors_in_source_order() {
        assert_eq!(
            <(ServiceId, ServiceId) as ServiceSet>::DESCRIPTORS,
            &[DESCRIPTOR, DESCRIPTOR]
        );
    }

    #[test]
    fn a_version_is_compatible_within_the_recorded_window() {
        let local = Version { major: 1, minor: 4 };
        assert!(compatible(local, Version { major: 1, minor: 2 }));
        assert!(!compatible(local, Version { major: 1, minor: 1 }));
        assert!(!compatible(local, Version { major: 2, minor: 4 }));
    }

    #[test]
    fn every_status_detail_variant_round_trips_through_its_tag() {
        for &reason in Reason::ALL {
            let status = reason.into_status("test");
            assert_eq!(status.code, Code::Unavailable);
            assert_eq!(Reason::from_status(&status), Some(reason));
        }
        assert_eq!(
            Reason::from_status(&Status::new(Code::Unavailable, "plain")),
            None
        );
        assert_eq!(
            Reason::from_status(&Status::new(Code::Internal, "other")),
            None
        );
    }
}
