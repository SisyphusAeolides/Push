//! Push-side admission for Argus HTTPS leases.
//!
//! This module performs policy admission only. It cannot manufacture TCP,
//! TLS, or an IPC mapping: Arach must first supply a live network capability
//! and a shared Hermes endpoint. Keeping that absence explicit prevents the UI
//! from treating a URL parser result as network authority.

use slope::hypermedia::{ArgusEndpointLease, HttpLease, HttpsRequest};

use crate::gordian::CapabilityHandle;
use crate::service::{ServiceId, ServiceState, Supervisor};

/// A lease is deliberately short-lived in supervisor epochs, preventing a
/// staged document request from becoming long-lived ambient network access.
pub const MAX_HTTP_LEASE_EPOCHS: u64 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpLeaseError {
    WrongRequester,
    ServiceNotRunning,
    InvalidLifetime,
    InvalidBrokerReply,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointLeaseError {
    WrongRequester,
    ServiceNotRunning,
    InvalidLifetime,
    InvalidBrokerReply,
}

/// Converts one authenticated Arach network capability into an Argus-only
/// HTTPS lease. The raw broker handle stays inside the resulting opaque lease;
/// callers receive no raw socket or generalized `NetworkCapability`.
pub fn issue_http_lease(
    requester: ServiceId,
    supervisor: &Supervisor,
    network_handle: CapabilityHandle,
    generation: u32,
    request: HttpsRequest,
    current_epoch: u64,
    expiry_epoch: u64,
) -> Result<HttpLease, HttpLeaseError> {
    if requester != ServiceId::Argus {
        return Err(HttpLeaseError::WrongRequester);
    }
    if supervisor.status(ServiceId::Argus).state != ServiceState::Running {
        return Err(HttpLeaseError::ServiceNotRunning);
    }
    if expiry_epoch <= current_epoch || expiry_epoch - current_epoch > MAX_HTTP_LEASE_EPOCHS {
        return Err(HttpLeaseError::InvalidLifetime);
    }
    // SAFETY: `network_handle` is opaque and may only have been constructed by
    // a Arach-backed Push capability broker. The checked service identity,
    // bounded request, and lifetime are retained in the derived lease.
    unsafe {
        HttpLease::from_broker(
            network_handle.raw(),
            generation,
            request.origin(),
            request.budget(),
            expiry_epoch,
        )
        .map_err(|_| HttpLeaseError::InvalidBrokerReply)
    }
}

/// Derives the Argus Hermes endpoint authority from two broker handles.  The
/// endpoint and mapping remain distinct so Arach can revoke the shared
/// mapping before recycling its physical pages; Push never receives a
/// physical address or a raw NIC capability.
pub fn issue_argus_endpoint(
    requester: ServiceId,
    supervisor: &Supervisor,
    endpoint_handle: CapabilityHandle,
    mapping_handle: CapabilityHandle,
    generation: u32,
    mapping_generation: u32,
    current_epoch: u64,
    expiry_epoch: u64,
) -> Result<ArgusEndpointLease, EndpointLeaseError> {
    if requester != ServiceId::Argus {
        return Err(EndpointLeaseError::WrongRequester);
    }
    if supervisor.status(ServiceId::Argus).state != ServiceState::Running {
        return Err(EndpointLeaseError::ServiceNotRunning);
    }
    if expiry_epoch <= current_epoch || expiry_epoch - current_epoch > MAX_HTTP_LEASE_EPOCHS {
        return Err(EndpointLeaseError::InvalidLifetime);
    }
    // SAFETY: both handles are opaque values returned by the authenticated
    // broker, and the caller supplies the generation pairing it received.
    unsafe {
        ArgusEndpointLease::from_broker(
            endpoint_handle.raw(),
            mapping_handle.raw(),
            generation,
            mapping_generation,
            expiry_epoch,
        )
        .map_err(|_| EndpointLeaseError::InvalidBrokerReply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slope::hypermedia::HttpBudget;

    #[test]
    fn rejects_unstarted_or_wrong_requesters_before_a_lease_can_exist() {
        let supervisor = Supervisor::new();
        let request = HttpsRequest::parse_location(b"https://example.com/", HttpBudget::DEFAULT)
            .expect("HTTPS request");
        // SAFETY: this synthetic token models a nonzero broker reply.
        let handle = unsafe { CapabilityHandle::from_kernel(1).expect("handle") };
        assert_eq!(
            issue_http_lease(ServiceId::Crest, &supervisor, handle, 1, request, 1, 2),
            Err(HttpLeaseError::WrongRequester)
        );
        assert_eq!(
            issue_http_lease(ServiceId::Argus, &supervisor, handle, 1, request, 1, 2),
            Err(HttpLeaseError::ServiceNotRunning)
        );
        assert_eq!(
            issue_argus_endpoint(ServiceId::Crest, &supervisor, handle, handle, 1, 1, 1, 2),
            Err(EndpointLeaseError::WrongRequester)
        );
        assert_eq!(
            issue_argus_endpoint(ServiceId::Argus, &supervisor, handle, handle, 1, 1, 1, 2),
            Err(EndpointLeaseError::ServiceNotRunning)
        );
    }
}
