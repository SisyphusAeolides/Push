//! Bounded service-priority policy with an optional Fortran implementation.

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServiceEvidence {
    pub criticality: f64,
    pub health: f64,
    pub dependency_readiness: f64,
    pub failure_pressure: f64,
}

impl ServiceEvidence {
    const fn as_array(self) -> [f64; 4] {
        [
            self.criticality,
            self.health,
            self.dependency_readiness,
            self.failure_pressure,
        ]
    }
}

pub fn supervision_score(evidence: ServiceEvidence) -> f64 {
    let values = evidence.as_array();
    score_impl(&values)
}

#[cfg(feature = "fortran-policy")]
fn score_impl(values: &[f64; 4]) -> f64 {
    unsafe extern "C" {
        fn arach_push_supervision_score(features: *const f64, count: i32) -> f64;
    }
    // SAFETY: the Fortran function reads the four contiguous values and does
    // not retain their address.
    unsafe { arach_push_supervision_score(values.as_ptr(), values.len() as i32) }
}

#[cfg(not(feature = "fortran-policy"))]
fn score_impl(values: &[f64; 4]) -> f64 {
    let criticality = values[0].clamp(0.0, 1.0);
    let health = values[1].clamp(0.0, 1.0);
    let readiness = values[2].clamp(0.0, 1.0);
    let pressure = values[3].clamp(0.0, 1.0);
    (criticality * 0.40 + health * 0.20 + readiness * 0.40 - pressure * 0.25).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_critical_service_outranks_failed_dependency_chain() {
        let ready = supervision_score(ServiceEvidence {
            criticality: 1.0,
            health: 1.0,
            dependency_readiness: 1.0,
            failure_pressure: 0.0,
        });
        let blocked = supervision_score(ServiceEvidence {
            dependency_readiness: 0.0,
            failure_pressure: 1.0,
            ..ServiceEvidence {
                criticality: 1.0,
                health: 1.0,
                dependency_readiness: 1.0,
                failure_pressure: 0.0,
            }
        });
        assert!(ready > blocked);
        assert!((0.0..=1.0).contains(&ready));
        assert!((0.0..=1.0).contains(&blocked));
    }
}
