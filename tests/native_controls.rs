#![cfg(feature = "gpu-physics")]

use sard::physics::gpu::articulated::GpuArticulatedDynamicsBatch;
use sard::physics::gpu::articulated_state::reset::GpuArticulatedResetSelection;
use sard::physics::gpu::controls::{GpuMotorTargetMapping, GpuMotorTargetMode};
use sard::physics::gpu::wgpu::{BufferDescriptor, BufferUsages, CommandEncoderDescriptor};
use sard::physics::tessera::articulated_world::JointMotor;
use sard::physics::urdf::{load_urdf_str, UrdfLoadOptions};
use sard::physics::{GpuContactDevice, PhysicsConfig};

#[test]
fn sard_resident_actions_delay_and_selective_reset_preserve_other_environment_controls() {
    // Given: two independent native sliders controlled entirely on their device.
    let xml = r#"<robot name="controlled_slider">
      <link name="base"/>
      <link name="slider">
        <inertial><mass value="1"/><inertia ixx="1" ixy="0" ixz="0" iyy="1" iyz="0" izz="1"/></inertial>
        <collision><geometry><sphere radius="0.2"/></geometry></collision>
      </link>
      <joint name="slide" type="prismatic">
        <parent link="base"/><child link="slider"/><origin xyz="0 0 3"/>
        <axis xyz="1 0 0"/><limit lower="-5" upper="5" effort="100" velocity="100"/>
      </joint>
    </robot>"#;
    let mut loaded = load_urdf_str(
        xml,
        UrdfLoadOptions {
            world: PhysicsConfig {
                gravity: [0.0; 3],
                ..PhysicsConfig::default()
            },
            ..UrdfLoadOptions::default()
        },
    )
    .expect("native test operation");
    loaded
        .world
        .set_joint_motor(
            0,
            Some(JointMotor {
                position_target: Some(0.0),
                velocity_target: 0.0,
                stiffness: 1.0,
                damping: 0.0,
                max_force: 100.0,
            }),
        )
        .expect("native test operation");
    let context = GpuContactDevice::new().expect("native test operation");
    let input = loaded
        .world
        .gpu_dynamics_input(&[0.0])
        .expect("native test operation");
    let mut batch = GpuArticulatedDynamicsBatch::new(&context, &[input.clone(), input], 0.01)
        .expect("native test operation");
    let mapping = vec![GpuMotorTargetMapping {
        coordinate: 0,
        link: 1,
        mode: GpuMotorTargetMode::Position,
    }];
    batch
        .enable_motor_target_control(&[mapping.clone(), mapping], &[0, 0])
        .expect("native test operation");
    let sensor = batch
        .normal_impulse_sensor(&[1])
        .expect("native test operation");
    let actions = context.device().create_buffer(&BufferDescriptor {
        label: Some("Sard resident policy actions"),
        size: 8,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let delays = context.device().create_buffer(&BufferDescriptor {
        label: Some("Sard resident actuator delays"),
        size: 8,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    context
        .queue()
        .write_buffer(&delays, 0, bytemuck::cast_slice(&[0_u32, 2]));
    context
        .queue()
        .write_buffer(&actions, 0, bytemuck::cast_slice(&[2_f32, -3.0]));
    let mut encoder = context
        .device()
        .create_command_encoder(&CommandEncoderDescriptor::default());
    let templates = batch
        .snapshot_reset_templates(&mut encoder)
        .expect("native test operation");
    batch
        .encode_motor_target_delays(&mut encoder, &delays)
        .expect("native test operation");
    batch
        .encode_motor_targets(&mut encoder, &actions)
        .expect("native test operation");
    batch
        .encode_step(&mut encoder)
        .expect("native test operation");
    batch
        .encode_step(&mut encoder)
        .expect("native test operation");
    let _submission = context.queue().submit(Some(encoder.finish()));
    let delayed = batch.readback().expect("native test operation");
    assert!(delayed[0].velocities[0] > 0.03);
    assert!(delayed[1].velocities[0].abs() < 1e-12);

    // When: only environment zero resets; environment one's pending action survives.
    let mut encoder = context
        .device()
        .create_command_encoder(&CommandEncoderDescriptor::default());
    batch
        .encode_reset_envs_from_templates(
            &mut encoder,
            &templates,
            &[GpuArticulatedResetSelection {
                environment: 0,
                template: 0,
                root_translation: None,
                root_velocity: None,
            }],
        )
        .expect("native test operation");
    batch
        .encode_step(&mut encoder)
        .expect("native test operation");
    sensor.encode(&mut encoder);
    let _submission = context.queue().submit(Some(encoder.finish()));
    let observed = batch.readback().expect("native test operation");

    // Then: selected state/drives restore while delayed controls remain environment-local.
    assert!(observed[0].positions[0].abs() < 1e-12);
    assert!(observed[0].velocities[0].abs() < 1e-12);
    assert!(observed[1].velocities[0] < -0.02);
    assert!(sensor
        .readback(context.queue())
        .expect("native test operation")
        .iter()
        .flatten()
        .all(|v| v.abs() < 1e-7));
}
