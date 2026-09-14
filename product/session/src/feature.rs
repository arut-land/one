//! The features a node composes, named as a type rather than as a list.
//!
//! This is the composition half of ADR 0006 lifted from one feature to a set,
//! and it copies `arut_rpc::ServiceSet` exactly (ADR 0025): a tuple is the set,
//! the tuple is the order, and the manifest is derived from the members'
//! service markers, so no package or service name is written by hand.
//!
//! Two traits, because composing needs a runtime and binding to a channel does
//! not. [`Feature`] and [`FeatureSet`] say what a session gets; [`Compose`] and
//! [`ComposeSet`] say what a local root can build, and their impls carry each
//! feature's port bundle as a bound. A runtime missing a port does not satisfy
//! `Compose<R>` for the feature that needs it, so naming it in the set is a
//! compile error rather than a startup failure.
//!
//! A feature's adapter lives here and not in the feature crate: `features/` may
//! not depend on `product/`, and this is the composition layer.
//!
//! # Examples
//!
//! ```
//! use arut_product_session::feature::{Chat, ComposeSet, FeatureSet, Services};
//! use arut_rpc::ServiceSet;
//!
//! type Features = (Chat,);
//! assert_eq!(
//!     <Services<Features> as ServiceSet>::DESCRIPTORS.len(),
//!     <arut_feature_chat::ChatServices as ServiceSet>::DESCRIPTORS.len(),
//! );
//! ```
//!
//! A runtime without the chat ports cannot be named in a set that has chat:
//!
//! ```compile_fail
//! use arut_product_session::feature::{Chat, ComposeSet};
//! use std::sync::Arc;
//!
//! struct NoPorts;
//! let _ = <(Chat,) as ComposeSet<NoPorts>>::compose(&Arc::new(NoPorts));
//! ```

use std::marker::PhantomData;
use std::sync::Arc;

use arut_feature_chat::{ChatClients, ChatRuntime, ChatServices};
use arut_rpc::{RpcChannel, RpcService, ServiceDescriptor, ServiceSet};

/// Why a feature could not be composed on a runtime.
pub type ComposeError = arut_storage::StorageError;

/// What one feature contributes to a session: the services it serves and the
/// clients a scope reaches it through.
pub trait Feature: 'static {
    /// The services it serves, as generated markers (ADR 0025).
    type Services: ServiceSet;
    /// The generated clients a session holds.
    type Clients: Clone + Send + Sync + 'static;

    /// Bind this feature's clients to the route the root chose.
    fn remote(channel: &Arc<dyn RpcChannel>) -> Self::Clients;
}

/// A feature `R` supplies the ports for.
///
/// The bound lives on the impl, so `R: Compose<F>` is the compiler's answer to
/// "can this runtime serve this feature".
pub trait Compose<R: ?Sized>: Feature {
    /// Construct this feature's services over `runtime` and hand back its
    /// clients and erased routers.
    ///
    /// # Errors
    ///
    /// Returns whatever opening the feature's storage returned.
    fn compose(runtime: &Arc<R>) -> Result<Composed<Self::Clients>, ComposeError>;
}

/// The features a node composes, named as a tuple of [`Feature`] members.
///
/// `DESCRIPTORS` is the concatenation of each member's `ServiceSet`, so a
/// manifest still comes from types alone.
pub trait FeatureSet: 'static {
    /// Each member's clients, in tuple order.
    type Clients: Clone + Send + Sync + 'static;
    /// Every service the set serves, in tuple order.
    const DESCRIPTORS: &'static [ServiceDescriptor];

    /// Bind every member's clients to the route the root chose.
    fn remote(channel: &Arc<dyn RpcChannel>) -> Self::Clients;
}

/// A set every member of which `R` supplies the ports for.
pub trait ComposeSet<R: ?Sized>: FeatureSet {
    /// Compose every member over `runtime`, flattening their routers.
    ///
    /// # Errors
    ///
    /// Returns the first member's composition failure.
    fn compose(runtime: &Arc<R>) -> Result<Composed<Self::Clients>, ComposeError>;
}

/// What composition produced: the clients a session holds and the routers a
/// node serves.
pub struct Composed<C> {
    pub clients: C,
    pub routers: Vec<Arc<dyn RpcService>>,
}

/// A feature set's services, as the [`ServiceSet`] the capability manifest
/// takes. `capability_client::<Services<F>>()` is how a session advertises
/// exactly what its features serve.
pub struct Services<F>(PhantomData<F>);

impl<F: FeatureSet> ServiceSet for Services<F> {
    const DESCRIPTORS: &'static [ServiceDescriptor] = F::DESCRIPTORS;
}

/// The most services one node advertises. Raising it costs static array slots
/// and nothing else; a set that overflows it fails to compile.
const CAPACITY: usize = 32;

const PADDING: ServiceDescriptor = ServiceDescriptor {
    name: "",
    package: "",
    version: arut_rpc::Version { major: 0, minor: 0 },
    methods: &[],
};

/// Concatenate the members' descriptors while the crate compiles.
///
/// A slice of the exact length cannot be built generically on stable, so the
/// members are copied into a fixed buffer and the used prefix is what
/// `DESCRIPTORS` names.
const fn joined(parts: &[&[ServiceDescriptor]]) -> ([ServiceDescriptor; CAPACITY], usize) {
    let mut out = [PADDING; CAPACITY];
    let mut total = 0;
    let mut part = 0;
    while part < parts.len() {
        let mut index = 0;
        while index < parts[part].len() {
            out[total] = parts[part][index];
            total += 1;
            index += 1;
        }
        part += 1;
    }
    (out, total)
}

const fn used(
    buffer: &'static ([ServiceDescriptor; CAPACITY], usize),
) -> &'static [ServiceDescriptor] {
    buffer.0.split_at(buffer.1).0
}

macro_rules! feature_set {
    ($($name:ident),+) => {
        impl<$($name: Feature),+> FeatureSet for ($($name,)+) {
            type Clients = ($($name::Clients,)+);
            const DESCRIPTORS: &'static [ServiceDescriptor] =
                used(&joined(&[$(<$name::Services as ServiceSet>::DESCRIPTORS),+]));
            fn remote(channel: &Arc<dyn RpcChannel>) -> Self::Clients {
                ($($name::remote(channel),)+)
            }
        }
        impl<R: ?Sized, $($name: Compose<R>),+> ComposeSet<R> for ($($name,)+) {
            fn compose(runtime: &Arc<R>) -> Result<Composed<Self::Clients>, ComposeError> {
                let mut routers = Vec::new();
                let clients = ($({
                    let composed = <$name as Compose<R>>::compose(runtime)?;
                    routers.extend(composed.routers);
                    composed.clients
                },)+);
                Ok(Composed { clients, routers })
            }
        }
    };
}
feature_set!(A);
feature_set!(A, B);
feature_set!(A, B, C);
feature_set!(A, B, C, D);

/// Chat as a set member.
pub struct Chat;

impl Feature for Chat {
    type Services = ChatServices;
    type Clients = ChatClients;
    fn remote(channel: &Arc<dyn RpcChannel>) -> Self::Clients {
        ChatClients::remote(channel.clone())
    }
}

impl<R: ChatRuntime> Compose<R> for Chat {
    fn compose(runtime: &Arc<R>) -> Result<Composed<ChatClients>, ComposeError> {
        let feature = arut_feature_chat::compose(runtime.clone())?;
        Ok(Composed {
            clients: feature.clients(),
            routers: feature.routers().collect(),
        })
    }
}

/// A set that serves chat, and which member answers for it.
///
/// The session's conversation list, pending chat and transcript are chat's, so
/// this is what `ProductSession<F>` needs of its set. A second feature with a
/// scope of its own adds a trait beside this one, not a field to the session.
pub trait HasChat: FeatureSet {
    fn chat(clients: &Self::Clients) -> &ChatClients;
}

impl<B: Feature> HasChat for (Chat, B) {
    fn chat(clients: &Self::Clients) -> &ChatClients {
        &clients.0
    }
}

impl HasChat for (Chat,) {
    fn chat(clients: &Self::Clients) -> &ChatClients {
        &clients.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Chat, FeatureSet, Services};
    use arut_rpc::ServiceSet;

    #[test]
    fn a_sets_descriptors_are_its_members_service_sets_in_order() {
        let chat = <arut_feature_chat::ChatServices as ServiceSet>::DESCRIPTORS;
        assert_eq!(<(Chat,) as FeatureSet>::DESCRIPTORS, chat);
        assert_eq!(
            <Services<(Chat,)> as ServiceSet>::DESCRIPTORS
                .iter()
                .map(|descriptor| descriptor.name)
                .collect::<Vec<_>>(),
            chat.iter()
                .map(|descriptor| descriptor.name)
                .collect::<Vec<_>>(),
        );
    }
}
