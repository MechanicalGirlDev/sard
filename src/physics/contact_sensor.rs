//! Host observations of native solved contacts; no contact solver lives here.

use super::tessera::articulated_world::ArticulatedWorld;

/// An invalid selected-link contact observation.
#[derive(Debug, thiserror::Error)]
pub enum ContactObservationError {
    #[error("select one to four unique native link indices")]
    InvalidSelection,
    #[error("native link {0} does not exist")]
    MissingLink(usize),
    #[error("the sampled native substep duration must be finite and positive")]
    InvalidSubstep,
    #[error("native link {0} has invalid contact diagnostics")]
    InvalidDiagnostics(usize),
}

/// Select up to four native articulated links for contact observations.
#[derive(Debug, Clone)]
pub struct ContactImpulseSensor {
    links: Vec<usize>,
}

impl ContactImpulseSensor {
    pub fn new(links: &[usize]) -> Result<Self, ContactObservationError> {
        if links.is_empty()
            || links.len() > 4
            || links
                .iter()
                .enumerate()
                .any(|(index, link)| links[..index].contains(link))
        {
            return Err(ContactObservationError::InvalidSelection);
        }
        Ok(Self {
            links: links.to_vec(),
        })
    }

    pub fn selected_links(&self) -> &[usize] {
        &self.links
    }

    /// Sum scalar normal impulses, in newton seconds, for the last native substep.
    ///
    /// Supply the duration of the substep represented by `contact_samples`, not
    /// the enclosing frame duration. Tangential friction is projected out;
    /// opposing contacts add rather than cancelling as resultant forces do.
    /// This samples host diagnostics and does not claim GPU-resident output.
    pub fn observe(
        &self,
        world: &ArticulatedWorld,
        substep_seconds: f64,
    ) -> Result<Vec<f64>, ContactObservationError> {
        if !substep_seconds.is_finite() || substep_seconds <= 0.0 {
            return Err(ContactObservationError::InvalidSubstep);
        }
        self.links
            .iter()
            .map(|&link| {
                let contacts = world
                    .contact_samples
                    .get(link)
                    .ok_or(ContactObservationError::MissingLink(link))?;
                let mut impulse = 0.0;
                for sample in contacts {
                    if !sample
                        .force
                        .iter()
                        .chain(sample.normal.iter())
                        .all(|value| value.is_finite())
                        || (sample.normal.norm_squared() - 1.0).abs() > 1e-4
                    {
                        return Err(ContactObservationError::InvalidDiagnostics(link));
                    }
                    impulse += sample.force.dot(&sample.normal).max(0.0) * substep_seconds;
                }
                if !impulse.is_finite() {
                    return Err(ContactObservationError::InvalidDiagnostics(link));
                }
                Ok(impulse)
            })
            .collect()
    }
}
