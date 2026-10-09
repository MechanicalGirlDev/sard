//! Rendering data from an authoritative MPM snapshot, without extrapolation.

use glam::{DVec3, Vec3};
use tessera_mpm::MpmParticle;

use crate::renderer::ParticleData;

/// A particle snapshot cannot be represented by the rendering device.
#[derive(Debug, thiserror::Error)]
pub enum ParticleRenderError {
    #[error("particle color must contain finite nonnegative values")]
    InvalidColor,
    #[error("particle {0} has a position outside the finite rendering range")]
    InvalidPosition(usize),
}

/// Convert enabled native particles to frozen rendering data.
///
/// Use the latest synchronized CPU/GPU snapshot. Velocities are zero; pass
/// `Vec3::ZERO` as the acceleration to `ParticleSystem::new` to display the
/// snapshot without another particle simulation. Fixed particles remain visible.
pub fn mpm_particle_data(
    particles: &[MpmParticle],
    color: [f32; 3],
) -> Result<ParticleData, ParticleRenderError> {
    if color.iter().any(|value| !value.is_finite() || *value < 0.0) {
        return Err(ParticleRenderError::InvalidColor);
    }
    let mut positions = Vec::with_capacity(particles.len());
    for (index, particle) in particles.iter().enumerate() {
        if !particle.enabled {
            continue;
        }
        let position = DVec3::new(
            particle.position.x,
            particle.position.y,
            particle.position.z,
        )
        .as_vec3();
        if !position.is_finite() {
            return Err(ParticleRenderError::InvalidPosition(index));
        }
        positions.push(position);
    }
    let count = positions.len();
    Ok(ParticleData {
        start_positions: positions,
        start_velocities: vec![Vec3::ZERO; count],
        colors: vec![color; count],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;
    use tessera_mpm::MaterialModel;

    #[test]
    fn authoritative_particles_do_not_acquire_rendering_motion() {
        // Given: moving, fixed, and disabled material points.
        let mut moving = MpmParticle::new(
            Vector3::new(1.0, 2.0, 3.0),
            0.02,
            1_000.0,
            MaterialModel::default(),
        );
        moving.velocity = Vector3::new(10.0, 20.0, 30.0);
        let mut fixed = moving.clone();
        fixed.position.x = 4.0;
        fixed.fixed = true;
        let mut disabled = moving.clone();
        disabled.enabled = false;

        // When: the latest snapshot is prepared for rendering.
        let data = mpm_particle_data(&[moving, fixed, disabled], [0.2, 0.4, 0.6]).unwrap();

        // Then: only visible points are copied, with no second simulation.
        assert_eq!(
            data.start_positions,
            [Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 2.0, 3.0)]
        );
        assert_eq!(data.start_velocities, [Vec3::ZERO; 2]);
        assert_eq!(data.colors, [[0.2, 0.4, 0.6]; 2]);
    }

    #[cfg(feature = "gpu-mpm")]
    #[test]
    fn resident_mpm_readback_publishes_native_motion_without_extrapolation() {
        use crate::physics::mpm::{
            GpuMpmResidentSession, GpuMpmTransfers, MpmParams, MpmWorld, WorldBounds,
        };
        use crate::physics::GpuContactDevice;

        // Given: one native material point and a required compute adapter.
        let device = GpuContactDevice::new().unwrap();
        let mut particle = MpmParticle::new(
            Vector3::repeat(0.5),
            0.04,
            1_000.0,
            MaterialModel::neo_hookean(2_000.0, 0.2),
        );
        particle.velocity.x = 1.0;
        let params = MpmParams {
            bounds: Some(WorldBounds {
                min: Vector3::zeros(),
                max: Vector3::repeat(1.0),
            }),
            ..MpmParams::default()
        };
        let native = MpmWorld::new(vec![particle], params).unwrap();
        let mut cpu = native.clone();
        let transfers = GpuMpmTransfers::new(device.device());
        let mut session =
            GpuMpmResidentSession::new(&transfers, device.device(), device.queue(), native, 0.0001)
                .unwrap();

        // When: steps stay resident until the owner explicitly synchronizes.
        session.submit_steps(4).unwrap();
        assert_eq!(session.world().substeps, 0);
        session.synchronize().unwrap();
        for _ in 0..4 {
            cpu.step(0.0001).unwrap();
        }
        let data = mpm_particle_data(&session.world().particles, [1.0, 0.5, 0.0]).unwrap();

        // Then: the renderer receives the solved snapshot, not a second simulation.
        let expected = &cpu.particles[0].position;
        let actual = data.start_positions[0];
        assert_eq!(session.world().substeps, 4);
        assert!(actual.x > 0.5003);
        assert!(actual.z < 0.5);
        assert!((f64::from(actual.x) - expected.x).abs() < 2e-5);
        assert!((f64::from(actual.z) - expected.z).abs() < 2e-5);
        assert_eq!(data.start_velocities, [Vec3::ZERO]);
    }
}
