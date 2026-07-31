use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};

pub const SERVICE_COUNT: usize = 9;
pub const MAXIMUM_SERVICES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ServiceId {
    SlopeNet = 0,
    Corinth = 1,
    Crest = 2,
    /// Argus is catalogued separately so browser transport authority cannot
    /// be confused with Crest's display authority. It is not booted until a
    /// measured Argus service image and broker mapping exist.
    Argus = 3,
    DbusBroker = 4,
    CosmicCompositor = 5,
    CosmicGreeter = 6,
    CosmicSession = 7,
    XdgPortal = 8,
}

impl ServiceId {
    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Backoff,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorMode {
    Normal,
    Recovery { failed_service: ServiceId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureReason {
    LaunchUnavailable,
    LaunchRejected,
    Exited(i32),
    Unresponsive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceSpec {
    pub id: ServiceId,
    pub name: &'static str,
    pub executable: &'static str,
    pub critical: bool,
    pub maximum_restarts: u8,
    pub backoff_ticks: u64,
}

pub const CORINTH: ServiceSpec = ServiceSpec {
    id: ServiceId::Corinth,
    name: "corinth",
    executable: "/system/corinth",
    critical: true,
    maximum_restarts: 3,
    backoff_ticks: 4,
};

pub const SLOPE_NET: ServiceSpec = ServiceSpec {
    id: ServiceId::SlopeNet,
    name: "slope-net",
    executable: "/system/slope-net",
    critical: true,
    maximum_restarts: 5,
    backoff_ticks: 20,
};

pub const CREST: ServiceSpec = ServiceSpec {
    id: ServiceId::Crest,
    name: "crest",
    executable: "/system/crest",
    critical: false,
    // Arach reclaims a stopped Crest image only after execution leaves its
    // page-table root. Restart remains terminal until the kernel can build a
    // fresh verified image rather than reusing mutable user memory.
    maximum_restarts: 0,
    backoff_ticks: 2,
};

pub const ARGUS: ServiceSpec = ServiceSpec {
    id: ServiceId::Argus,
    name: "argus",
    executable: "/system/argus",
    critical: false,
    maximum_restarts: 2,
    backoff_ticks: 8,
};

pub const DBUS_BROKER: ServiceSpec = ServiceSpec {
    id: ServiceId::DbusBroker,
    name: "dbus-broker",
    executable: "/system/dbus-broker-launch",
    critical: true,
    maximum_restarts: 5,
    backoff_ticks: 4,
};

pub const COSMIC_COMPOSITOR: ServiceSpec = ServiceSpec {
    id: ServiceId::CosmicCompositor,
    name: "cosmic-comp",
    executable: "/system/cosmic-comp",
    critical: true,
    maximum_restarts: 3,
    backoff_ticks: 8,
};

pub const COSMIC_GREETER: ServiceSpec = ServiceSpec {
    id: ServiceId::CosmicGreeter,
    // COSMIC Greeter is a greetd session. The live image provides both the
    // canonical cosmic-greeter.toml and the conventional config.toml alias,
    // so the current kernel spawn ABI can start greetd without argv support.
    name: "greetd (cosmic-greeter)",
    executable: "/system/greetd",
    critical: true,
    maximum_restarts: 3,
    backoff_ticks: 8,
};

pub const COSMIC_SESSION: ServiceSpec = ServiceSpec {
    id: ServiceId::CosmicSession,
    name: "cosmic-session",
    executable: "/system/cosmic-session",
    critical: false,
    maximum_restarts: 2,
    backoff_ticks: 8,
};

pub const XDG_PORTAL: ServiceSpec = ServiceSpec {
    id: ServiceId::XdgPortal,
    name: "xdg-desktop-portal-cosmic",
    executable: "/system/xdg-desktop-portal-cosmic",
    critical: false,
    maximum_restarts: 3,
    backoff_ticks: 8,
};

pub const INITIAL_SERVICES: [ServiceSpec; SERVICE_COUNT] = [
    SLOPE_NET,
    CORINTH,
    CREST,
    ARGUS,
    DBUS_BROKER,
    COSMIC_COMPOSITOR,
    COSMIC_GREETER,
    COSMIC_SESSION,
    XDG_PORTAL,
];

/// The measured COSMIC service set. Arach OS promotes this list into the boot
/// set only after every executable and dependency is present in the signed
/// system image.
pub const COSMIC_SERVICES: [ServiceSpec; 5] = [
    DBUS_BROKER,
    COSMIC_COMPOSITOR,
    COSMIC_GREETER,
    COSMIC_SESSION,
    XDG_PORTAL,
];
/// Services whose exact measured images are present in the current boot image.
///
/// The minimal C0 image contains only the bounded probe. A production image
/// is compiled with `cosmic-boot` after the signed COSMIC bundle has been
/// assembled; that profile promotes the complete session chain atomically so
/// Push never launches a partially measured desktop.
#[cfg(feature = "cosmic-boot")]
pub const BOOT_SERVICES: &[ServiceSpec] = &COSMIC_SERVICES;

#[cfg(not(feature = "cosmic-boot"))]
pub const BOOT_SERVICES: &[ServiceSpec] = &[CREST];

/// Fixed-capacity dependency graph. Bit N represents service N.
#[derive(Clone, Copy)]
pub struct ArachneMatrix {
    dependencies: [u16; MAXIMUM_SERVICES],
    active_state_mask: u16,
}

impl ArachneMatrix {
    pub const fn new() -> Self {
        Self {
            dependencies: [0; MAXIMUM_SERVICES],
            active_state_mask: 0,
        }
    }

    pub const fn require(&mut self, service: ServiceId, dependency: ServiceId) {
        self.dependencies[service.index()] |= 1_u16 << dependency.index();
    }

    #[inline(always)]
    pub fn can_start(&self, service: ServiceId) -> bool {
        let required = self.dependencies[service.index()];
        self.active_state_mask & required == required
    }

    pub fn mark_running(&mut self, service: ServiceId) {
        self.active_state_mask |= 1_u16 << service.index();
    }

    pub fn mark_failed(&mut self, service: ServiceId) {
        self.active_state_mask &= !(1_u16 << service.index());
    }

    pub const fn active_state_mask(&self) -> u16 {
        self.active_state_mask
    }
}

impl Default for ArachneMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C, align(4096))]
pub struct PanopticonTelemetry {
    pub heartbeat: AtomicU64,
    pub active_service_mask: AtomicU16,
    pub critical_failure_flag: AtomicU16,
    pub total_crashes: AtomicU64,
    pub compressed_failure_mass: AtomicU64,
}

pub static PANOPTICON: PanopticonTelemetry = PanopticonTelemetry {
    heartbeat: AtomicU64::new(0),
    active_service_mask: AtomicU16::new(0),
    critical_failure_flag: AtomicU16::new(0),
    total_crashes: AtomicU64::new(0),
    compressed_failure_mass: AtomicU64::new(0),
};

/// Four bits of saturating failure mass for each of sixteen service slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventHorizon {
    compressed_mass: u64,
}

impl EventHorizon {
    pub const fn new() -> Self {
        Self { compressed_mass: 0 }
    }

    pub fn accrete_mass(&mut self, service: ServiceId) {
        let shift = service.index() * 4;
        let current = (self.compressed_mass >> shift) & 0xf;
        if current < 0xf {
            self.compressed_mass =
                (self.compressed_mass & !(0xf_u64 << shift)) | ((current + 1) << shift);
        }
    }

    pub const fn measure_mass(&self, service: ServiceId) -> u8 {
        ((self.compressed_mass >> (service as usize * 4)) & 0xf) as u8
    }

    pub fn evaporate(&mut self, service: ServiceId) {
        self.compressed_mass &= !(0xf_u64 << (service.index() * 4));
    }

    pub const fn compressed_mass(&self) -> u64 {
        self.compressed_mass
    }
}

impl Default for EventHorizon {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Thermodynamics {
    stalled_ticks: u64,
    last_progress_generation: u64,
    pub event_horizon: EventHorizon,
}

impl Thermodynamics {
    pub const DEADLOCK_TICKS: u64 = 10_000;

    pub const fn new() -> Self {
        Self {
            stalled_ticks: 0,
            last_progress_generation: 0,
            event_horizon: EventHorizon::new(),
        }
    }

    fn observe(&mut self, progress_generation: u64, starting_mask: u16) -> Option<ServiceId> {
        if starting_mask == 0 {
            self.stalled_ticks = 0;
            self.last_progress_generation = progress_generation;
            return None;
        }
        if progress_generation != self.last_progress_generation {
            self.stalled_ticks = 1;
            self.last_progress_generation = progress_generation;
            return None;
        }
        self.stalled_ticks = self.stalled_ticks.saturating_add(1);
        if self.stalled_ticks < Self::DEADLOCK_TICKS {
            return None;
        }
        self.stalled_ticks = 0;

        let mut selected = None;
        let mut maximum_mass = 0;
        for spec in BOOT_SERVICES {
            if starting_mask & (1_u16 << spec.id.index()) == 0 {
                continue;
            }
            let mass = self.event_horizon.measure_mass(spec.id);
            if selected.is_none() || mass > maximum_mass {
                selected = Some(spec.id);
                maximum_mass = mass;
            }
        }
        selected
    }
}

impl Default for Thermodynamics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceStatus {
    pub state: ServiceState,
    pub pid: Option<u32>,
    pub restart_count: u8,
    pub next_start_tick: u64,
    pub last_failure: Option<FailureReason>,
}

impl ServiceStatus {
    const STOPPED: Self = Self {
        state: ServiceState::Stopped,
        pid: None,
        restart_count: 0,
        next_start_tick: 0,
        last_failure: None,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorAction {
    Start(ServiceSpec),
    Deadlock { service: ServiceId },
    EnterRecovery { failed_service: ServiceId },
    Idle,
}

pub struct Supervisor {
    tick: u64,
    mode: SupervisorMode,
    status: [ServiceStatus; SERVICE_COUNT],
    matrix: ArachneMatrix,
    thermodynamics: Thermodynamics,
    progress_generation: u64,
}

impl Supervisor {
    pub const fn new() -> Self {
        let mut matrix = ArachneMatrix::new();
        matrix.require(ServiceId::Corinth, ServiceId::SlopeNet);
        matrix.require(ServiceId::CosmicCompositor, ServiceId::DbusBroker);
        matrix.require(ServiceId::CosmicGreeter, ServiceId::DbusBroker);
        matrix.require(ServiceId::CosmicGreeter, ServiceId::CosmicCompositor);
        matrix.require(ServiceId::CosmicSession, ServiceId::DbusBroker);
        matrix.require(ServiceId::CosmicSession, ServiceId::CosmicCompositor);
        matrix.require(ServiceId::CosmicSession, ServiceId::CosmicGreeter);
        matrix.require(ServiceId::XdgPortal, ServiceId::DbusBroker);
        matrix.require(ServiceId::XdgPortal, ServiceId::CosmicSession);
        Self {
            tick: 0,
            mode: SupervisorMode::Normal,
            status: [ServiceStatus::STOPPED; SERVICE_COUNT],
            matrix,
            thermodynamics: Thermodynamics::new(),
            progress_generation: 1,
        }
    }

    pub const fn tick_count(&self) -> u64 {
        self.tick
    }

    pub const fn mode(&self) -> SupervisorMode {
        self.mode
    }

    pub fn status(&self, service: ServiceId) -> ServiceStatus {
        self.status[service.index()]
    }

    pub const fn active_service_mask(&self) -> u16 {
        self.matrix.active_state_mask()
    }

    pub const fn failure_mass(&self, service: ServiceId) -> u8 {
        self.thermodynamics.event_horizon.measure_mass(service)
    }

    /// Advances one bounded policy step and emits at most one external action.
    pub fn tick(&mut self) -> SupervisorAction {
        self.tick = self.tick.saturating_add(1);
        PANOPTICON.heartbeat.store(self.tick, Ordering::Relaxed);
        if !matches!(self.mode, SupervisorMode::Normal) {
            PANOPTICON.critical_failure_flag.store(1, Ordering::Release);
            return SupervisorAction::Idle;
        }

        if let Some(service) = self
            .thermodynamics
            .observe(self.progress_generation, self.starting_mask())
        {
            return SupervisorAction::Deadlock { service };
        }

        for spec in BOOT_SERVICES {
            let status = &mut self.status[spec.id.index()];
            if status.state == ServiceState::Failed && spec.critical {
                self.mode = SupervisorMode::Recovery {
                    failed_service: spec.id,
                };
                self.progress_generation = self.progress_generation.wrapping_add(1);
                return SupervisorAction::EnterRecovery {
                    failed_service: spec.id,
                };
            }
            if matches!(status.state, ServiceState::Stopped | ServiceState::Backoff)
                && self.tick >= status.next_start_tick
                && self.matrix.can_start(spec.id)
            {
                status.state = ServiceState::Starting;
                self.progress_generation = self.progress_generation.wrapping_add(1);
                return SupervisorAction::Start(*spec);
            }
            // Preserve causal ordering: later services wait until each prior
            // dependency has an acknowledged Running state.
            if status.state != ServiceState::Running {
                return SupervisorAction::Idle;
            }
        }
        SupervisorAction::Idle
    }

    /// Binds a successfully launched, nonzero process identity to one
    /// Starting service. The PID is retained so exit notifications cannot be
    /// confused across service restarts.
    pub fn record_started(&mut self, service: ServiceId, pid: u32) -> Result<(), SupervisorError> {
        if pid == 0 || self.pid_is_active(pid) {
            return Err(SupervisorError::InvalidProcessIdentity);
        }
        let status = &mut self.status[service.index()];
        if status.state != ServiceState::Starting {
            return Err(SupervisorError::InvalidTransition);
        }
        status.state = ServiceState::Running;
        status.pid = Some(pid);
        status.last_failure = None;
        self.matrix.mark_running(service);
        self.progress_generation = self.progress_generation.wrapping_add(1);
        PANOPTICON
            .active_service_mask
            .store(self.matrix.active_state_mask(), Ordering::Release);
        Ok(())
    }

    /// Attributes a reaped child to its exact currently-running service.
    /// An exited service is always a failed service from the supervisor's
    /// perspective, including exit status zero: availability is declared by
    /// a still-live measured process, not by a past successful exit.
    pub fn record_child_exit(
        &mut self,
        pid: u32,
        status: i32,
    ) -> Result<ServiceId, SupervisorError> {
        if pid == 0 {
            return Err(SupervisorError::UnknownProcess);
        }
        let service = INITIAL_SERVICES
            .iter()
            .find(|spec| self.status[spec.id.index()].pid == Some(pid))
            .map(|spec| spec.id)
            .ok_or(SupervisorError::UnknownProcess)?;
        self.record_failure(service, FailureReason::Exited(status))?;
        Ok(service)
    }

    pub fn record_failure(
        &mut self,
        service: ServiceId,
        reason: FailureReason,
    ) -> Result<(), SupervisorError> {
        let spec = INITIAL_SERVICES[service.index()];
        let status = &mut self.status[service.index()];
        if !matches!(status.state, ServiceState::Starting | ServiceState::Running) {
            return Err(SupervisorError::InvalidTransition);
        }
        status.last_failure = Some(reason);
        self.matrix.mark_failed(service);
        self.thermodynamics.event_horizon.accrete_mass(service);
        PANOPTICON.compressed_failure_mass.store(
            self.thermodynamics.event_horizon.compressed_mass(),
            Ordering::Release,
        );
        PANOPTICON.total_crashes.fetch_add(1, Ordering::Relaxed);

        for dependent in BOOT_SERVICES {
            if self.status[dependent.id.index()].state == ServiceState::Running
                && !self.matrix.can_start(dependent.id)
            {
                self.status[dependent.id.index()].state = ServiceState::Stopped;
                self.matrix.mark_failed(dependent.id);
            }
        }
        PANOPTICON
            .active_service_mask
            .store(self.matrix.active_state_mask(), Ordering::Release);

        let status = &mut self.status[service.index()];
        self.progress_generation = self.progress_generation.wrapping_add(1);
        status.pid = None;
        if status.restart_count >= spec.maximum_restarts {
            status.state = ServiceState::Failed;
            return Ok(());
        }

        status.restart_count += 1;
        status.state = ServiceState::Backoff;
        let delay = spec
            .backoff_ticks
            .saturating_mul(u64::from(status.restart_count));
        status.next_start_tick = self.tick.saturating_add(delay);
        Ok(())
    }

    fn starting_mask(&self) -> u16 {
        let mut mask = 0_u16;
        for spec in BOOT_SERVICES {
            if self.status[spec.id.index()].state == ServiceState::Starting {
                mask |= 1_u16 << spec.id.index();
            }
        }
        mask
    }

    fn pid_is_active(&self, pid: u32) -> bool {
        INITIAL_SERVICES
            .iter()
            .any(|spec| self.status[spec.id.index()].pid == Some(pid))
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorError {
    InvalidTransition,
    InvalidProcessIdentity,
    UnknownProcess,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "cosmic-boot"))]
    #[test]
    fn starts_only_the_measured_boot_service() {
        let mut supervisor = Supervisor::new();
        let first = BOOT_SERVICES[0];
        assert_eq!(supervisor.tick(), SupervisorAction::Start(first));
        supervisor.record_started(first.id, 2).unwrap();
        assert_eq!(supervisor.tick(), SupervisorAction::Idle);
    }

    #[cfg(feature = "cosmic-boot")]
    #[test]
    fn cosmic_boot_profile_contains_the_complete_ordered_session_chain() {
        assert_eq!(BOOT_SERVICES, &COSMIC_SERVICES);
        assert_eq!(BOOT_SERVICES.len(), COSMIC_SERVICES.len());
        assert_eq!(BOOT_SERVICES[0].id, ServiceId::DbusBroker);
        assert_eq!(BOOT_SERVICES[1].id, ServiceId::CosmicCompositor);
        assert_eq!(BOOT_SERVICES[2].id, ServiceId::CosmicGreeter);
        assert_eq!(BOOT_SERVICES[3].id, ServiceId::CosmicSession);
        assert_eq!(BOOT_SERVICES[4].id, ServiceId::XdgPortal);
    }

    #[cfg(feature = "cosmic-boot")]
    #[test]
    fn cosmic_boot_profile_advances_only_after_each_dependency_is_running() {
        let mut supervisor = Supervisor::new();
        assert_eq!(supervisor.tick(), SupervisorAction::Start(DBUS_BROKER));
        supervisor.record_started(ServiceId::DbusBroker, 2).unwrap();
        assert_eq!(
            supervisor.tick(),
            SupervisorAction::Start(COSMIC_COMPOSITOR)
        );
    }

    #[test]
    fn cosmic_catalog_covers_the_complete_session_chain() {
        assert_eq!(COSMIC_SERVICES[0], DBUS_BROKER);
        assert_eq!(COSMIC_SERVICES[1], COSMIC_COMPOSITOR);
        assert_eq!(COSMIC_SERVICES[2], COSMIC_GREETER);
        assert_eq!(COSMIC_SERVICES[3], COSMIC_SESSION);
        assert_eq!(COSMIC_SERVICES[4], XDG_PORTAL);
    }

    #[test]
    fn cosmic_session_dependencies_fail_closed() {
        let mut matrix = ArachneMatrix::new();
        matrix.require(ServiceId::CosmicSession, ServiceId::DbusBroker);
        matrix.require(ServiceId::CosmicSession, ServiceId::CosmicCompositor);
        matrix.require(ServiceId::CosmicSession, ServiceId::CosmicGreeter);
        assert!(!matrix.can_start(ServiceId::CosmicSession));
        matrix.mark_running(ServiceId::DbusBroker);
        matrix.mark_running(ServiceId::CosmicCompositor);
        assert!(!matrix.can_start(ServiceId::CosmicSession));
        matrix.mark_running(ServiceId::CosmicGreeter);
        assert!(matrix.can_start(ServiceId::CosmicSession));
    }

    #[cfg(not(feature = "cosmic-boot"))]
    #[test]
    fn unscheduled_catalog_entries_do_not_block_crest() {
        let mut supervisor = Supervisor::new();
        assert!(supervisor.matrix.can_start(ServiceId::Crest));
        assert_eq!(supervisor.tick(), SupervisorAction::Start(CREST));
    }

    #[cfg(not(feature = "cosmic-boot"))]
    #[test]
    fn single_use_crest_fails_without_advertising_an_impossible_restart() {
        let mut supervisor = Supervisor::new();
        assert_eq!(supervisor.tick(), SupervisorAction::Start(CREST));
        supervisor
            .record_failure(ServiceId::Crest, FailureReason::Unresponsive)
            .unwrap();
        assert_eq!(
            supervisor.status(ServiceId::Crest).state,
            ServiceState::Failed
        );
        assert_eq!(supervisor.mode(), SupervisorMode::Normal);
        assert_eq!(supervisor.tick(), SupervisorAction::Idle);
    }

    #[cfg(not(feature = "cosmic-boot"))]
    #[test]
    fn crest_failure_does_not_leave_a_false_active_bit() {
        let mut supervisor = Supervisor::new();
        assert_eq!(supervisor.tick(), SupervisorAction::Start(CREST));
        supervisor.record_started(ServiceId::Crest, 2).unwrap();
        assert_eq!(supervisor.active_service_mask(), 0b100);

        supervisor
            .record_failure(ServiceId::Crest, FailureReason::Unresponsive)
            .unwrap();
        assert_eq!(supervisor.active_service_mask(), 0);
    }

    #[cfg(not(feature = "cosmic-boot"))]
    #[test]
    fn child_exit_is_bound_to_the_exact_live_service_pid() {
        let mut supervisor = Supervisor::new();
        assert_eq!(supervisor.tick(), SupervisorAction::Start(CREST));
        supervisor.record_started(ServiceId::Crest, 41).unwrap();
        assert_eq!(supervisor.status(ServiceId::Crest).pid, Some(41));

        assert_eq!(
            supervisor.record_child_exit(42, 1),
            Err(SupervisorError::UnknownProcess)
        );
        assert_eq!(
            supervisor.status(ServiceId::Crest).state,
            ServiceState::Running
        );

        assert_eq!(supervisor.record_child_exit(41, 0), Ok(ServiceId::Crest));
        let service = supervisor.status(ServiceId::Crest);
        assert_eq!(service.pid, None);
        assert_eq!(service.state, ServiceState::Failed);
        assert_eq!(service.last_failure, Some(FailureReason::Exited(0)));
    }

    #[cfg(not(feature = "cosmic-boot"))]
    #[test]
    fn a_pid_cannot_be_reused_while_its_service_is_live() {
        let mut supervisor = Supervisor::new();
        assert_eq!(supervisor.tick(), SupervisorAction::Start(CREST));
        assert_eq!(
            supervisor.record_started(ServiceId::Crest, 0),
            Err(SupervisorError::InvalidProcessIdentity)
        );
        supervisor.record_started(ServiceId::Crest, 7).unwrap();
        assert_eq!(
            supervisor.record_started(ServiceId::Crest, 7),
            Err(SupervisorError::InvalidProcessIdentity)
        );
    }

    #[test]
    fn event_horizon_saturates_each_service_nibble() {
        let mut horizon = EventHorizon::new();
        for _ in 0..32 {
            horizon.accrete_mass(ServiceId::Corinth);
        }
        assert_eq!(horizon.measure_mass(ServiceId::Corinth), 15);
        assert_eq!(horizon.measure_mass(ServiceId::Crest), 0);
        horizon.evaporate(ServiceId::Corinth);
        assert_eq!(horizon.measure_mass(ServiceId::Corinth), 0);
    }

    #[cfg(not(feature = "cosmic-boot"))]
    #[test]
    fn thermodynamics_targets_only_a_stalled_start() {
        let mut supervisor = Supervisor::new();
        assert_eq!(supervisor.tick(), SupervisorAction::Start(CREST));
        for _ in 0..Thermodynamics::DEADLOCK_TICKS - 1 {
            assert_eq!(supervisor.tick(), SupervisorAction::Idle);
        }
        assert_eq!(
            supervisor.tick(),
            SupervisorAction::Deadlock {
                service: ServiceId::Crest
            }
        );
    }
}
