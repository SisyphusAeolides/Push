//! Ordered COSMIC logout and system shutdown coordination.

use crate::service::ServiceId;

pub const TERMINATE_GRACE_TICKS: u64 = 5_000;
pub const KILL_GRACE_TICKS: u64 = 1_000;

pub const LOGOUT_ORDER: [ServiceId; 2] = [ServiceId::XdgPortal, ServiceId::CosmicSession];

pub const SHUTDOWN_ORDER: [ServiceId; 8] = [
    ServiceId::XdgPortal,
    ServiceId::CosmicSession,
    ServiceId::CosmicGreeter,
    ServiceId::CosmicCompositor,
    ServiceId::Wireplumber,
    ServiceId::Pipewire,
    ServiceId::DbusBroker,
    ServiceId::Seatd,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleRequest {
    Logout,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleAction {
    Terminate(ServiceId),
    Kill(ServiceId),
    Complete(LifecycleRequest),
    Failed(ServiceId),
    Idle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    AlreadyActive,
    NotActive,
    UnexpectedService,
    InvalidProcessIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AwaitingExit {
    service: ServiceId,
    pid: u32,
    deadline: u64,
    killed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleCoordinator {
    request: Option<LifecycleRequest>,
    active_mask: u16,
    cursor: usize,
    awaiting: Option<AwaitingExit>,
    terminal: Option<LifecycleAction>,
}

impl LifecycleCoordinator {
    pub const fn new() -> Self {
        Self {
            request: None,
            active_mask: 0,
            cursor: 0,
            awaiting: None,
            terminal: None,
        }
    }

    pub fn begin(
        &mut self,
        request: LifecycleRequest,
        active_mask: u16,
    ) -> Result<(), LifecycleError> {
        if self.request.is_some() || self.terminal.is_some() {
            return Err(LifecycleError::AlreadyActive);
        }
        self.request = Some(request);
        self.active_mask = active_mask;
        self.cursor = 0;
        self.awaiting = None;
        Ok(())
    }

    pub const fn request(&self) -> Option<LifecycleRequest> {
        self.request
    }

    pub const fn awaiting_service(&self) -> Option<ServiceId> {
        match self.awaiting {
            Some(awaiting) => Some(awaiting.service),
            None => None,
        }
    }

    pub const fn remaining_mask(&self) -> u16 {
        self.active_mask
    }

    pub fn tick(
        &mut self,
        now: u64,
        pid_for: impl FnOnce(ServiceId) -> Option<u32> + Copy,
    ) -> LifecycleAction {
        if let Some(terminal) = self.terminal {
            return terminal;
        }
        let Some(request) = self.request else {
            return LifecycleAction::Idle;
        };

        if let Some(mut awaiting) = self.awaiting {
            if now < awaiting.deadline {
                return LifecycleAction::Idle;
            }
            if !awaiting.killed {
                awaiting.killed = true;
                awaiting.deadline = now.saturating_add(KILL_GRACE_TICKS);
                self.awaiting = Some(awaiting);
                return LifecycleAction::Kill(awaiting.service);
            }
            let failed = LifecycleAction::Failed(awaiting.service);
            self.terminal = Some(failed);
            self.request = None;
            return failed;
        }

        let order = order_for(request);
        while self.cursor < order.len() {
            let service = order[self.cursor];
            if self.active_mask & service_bit(service) == 0 {
                self.cursor += 1;
                continue;
            }
            let Some(pid) = pid_for(service) else {
                let failed = LifecycleAction::Failed(service);
                self.terminal = Some(failed);
                self.request = None;
                return failed;
            };
            if pid == 0 {
                let failed = LifecycleAction::Failed(service);
                self.terminal = Some(failed);
                self.request = None;
                return failed;
            }
            self.awaiting = Some(AwaitingExit {
                service,
                pid,
                deadline: now.saturating_add(TERMINATE_GRACE_TICKS),
                killed: false,
            });
            return LifecycleAction::Terminate(service);
        }

        let complete = LifecycleAction::Complete(request);
        self.terminal = Some(complete);
        self.request = None;
        complete
    }

    pub fn record_exit(&mut self, service: ServiceId, pid: u32) -> Result<(), LifecycleError> {
        let Some(awaiting) = self.awaiting else {
            return Err(LifecycleError::NotActive);
        };
        if service != awaiting.service {
            return Err(LifecycleError::UnexpectedService);
        }
        if pid == 0 || pid != awaiting.pid {
            return Err(LifecycleError::InvalidProcessIdentity);
        }
        self.active_mask &= !service_bit(service);
        self.cursor += 1;
        self.awaiting = None;
        Ok(())
    }
}

impl Default for LifecycleCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

const fn order_for(request: LifecycleRequest) -> &'static [ServiceId] {
    match request {
        LifecycleRequest::Logout => &LOGOUT_ORDER,
        LifecycleRequest::Shutdown => &SHUTDOWN_ORDER,
    }
}

const fn service_bit(service: ServiceId) -> u16 {
    1_u16 << service as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(services: &[ServiceId]) -> u16 {
        services
            .iter()
            .fold(0, |value, service| value | service_bit(*service))
    }

    fn pid_for(service: ServiceId) -> Option<u32> {
        Some(service as u32 + 100)
    }

    #[test]
    fn shutdown_uses_reverse_dependency_order() {
        let mut coordinator = LifecycleCoordinator::new();
        coordinator
            .begin(LifecycleRequest::Shutdown, mask(&SHUTDOWN_ORDER))
            .unwrap();

        for (index, service) in SHUTDOWN_ORDER.iter().copied().enumerate() {
            assert_eq!(
                coordinator.tick(index as u64, pid_for),
                LifecycleAction::Terminate(service)
            );
            coordinator
                .record_exit(service, service as u32 + 100)
                .unwrap();
        }
        assert_eq!(
            coordinator.tick(100, pid_for),
            LifecycleAction::Complete(LifecycleRequest::Shutdown)
        );
        assert_eq!(coordinator.remaining_mask(), 0);
    }

    #[test]
    fn logout_stops_only_session_owned_services() {
        let mut coordinator = LifecycleCoordinator::new();
        coordinator
            .begin(LifecycleRequest::Logout, mask(&SHUTDOWN_ORDER))
            .unwrap();
        for service in LOGOUT_ORDER {
            assert_eq!(
                coordinator.tick(0, pid_for),
                LifecycleAction::Terminate(service)
            );
            coordinator
                .record_exit(service, service as u32 + 100)
                .unwrap();
        }
        assert_eq!(
            coordinator.tick(0, pid_for),
            LifecycleAction::Complete(LifecycleRequest::Logout)
        );
        assert_ne!(coordinator.remaining_mask(), 0);
        assert_eq!(
            coordinator.remaining_mask() & service_bit(ServiceId::CosmicGreeter),
            service_bit(ServiceId::CosmicGreeter)
        );
    }

    #[test]
    fn inactive_services_are_skipped() {
        let mut coordinator = LifecycleCoordinator::new();
        coordinator
            .begin(
                LifecycleRequest::Shutdown,
                mask(&[ServiceId::CosmicSession, ServiceId::DbusBroker]),
            )
            .unwrap();
        assert_eq!(
            coordinator.tick(0, pid_for),
            LifecycleAction::Terminate(ServiceId::CosmicSession)
        );
        coordinator
            .record_exit(
                ServiceId::CosmicSession,
                ServiceId::CosmicSession as u32 + 100,
            )
            .unwrap();
        assert_eq!(
            coordinator.tick(1, pid_for),
            LifecycleAction::Terminate(ServiceId::DbusBroker)
        );
    }

    #[test]
    fn exit_must_match_exact_service_and_pid() {
        let mut coordinator = LifecycleCoordinator::new();
        coordinator
            .begin(
                LifecycleRequest::Shutdown,
                mask(&[ServiceId::CosmicSession]),
            )
            .unwrap();
        assert_eq!(
            coordinator.tick(0, pid_for),
            LifecycleAction::Terminate(ServiceId::CosmicSession)
        );
        assert_eq!(
            coordinator.record_exit(ServiceId::DbusBroker, 107),
            Err(LifecycleError::UnexpectedService)
        );
        assert_eq!(
            coordinator.record_exit(ServiceId::CosmicSession, 999),
            Err(LifecycleError::InvalidProcessIdentity)
        );
    }

    #[test]
    fn timeout_escalates_once_then_fails_closed() {
        let mut coordinator = LifecycleCoordinator::new();
        coordinator
            .begin(LifecycleRequest::Shutdown, mask(&[ServiceId::XdgPortal]))
            .unwrap();
        assert_eq!(
            coordinator.tick(0, pid_for),
            LifecycleAction::Terminate(ServiceId::XdgPortal)
        );
        assert_eq!(
            coordinator.tick(TERMINATE_GRACE_TICKS, pid_for),
            LifecycleAction::Kill(ServiceId::XdgPortal)
        );
        assert_eq!(
            coordinator.tick(TERMINATE_GRACE_TICKS + KILL_GRACE_TICKS, pid_for),
            LifecycleAction::Failed(ServiceId::XdgPortal)
        );
    }

    #[test]
    fn missing_process_identity_fails_closed() {
        let mut coordinator = LifecycleCoordinator::new();
        coordinator
            .begin(LifecycleRequest::Shutdown, mask(&[ServiceId::Seatd]))
            .unwrap();
        assert_eq!(
            coordinator.tick(0, |_| None),
            LifecycleAction::Failed(ServiceId::Seatd)
        );
    }
}
