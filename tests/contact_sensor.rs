#![cfg(feature = "physics")]

use sard::physics::nalgebra::{Isometry3, Matrix3, Vector3};
use sard::physics::tessera::articulated_world::{ArticulatedWorld, LinkContactSample};
use sard::physics::{ContactImpulseSensor, ContactObservationError, PhysicsConfig, PhysicsWorld};

fn native_world() -> Result<ArticulatedWorld, sard::physics::PhysicsError> {
    ArticulatedWorld::new(
        sard::physics::articulation::Articulation::new(
            vec![sard::physics::articulation::LinkSpec {
                mass: 0.0,
                center_of_mass: Vector3::zeros(),
                inertia: Matrix3::zeros(),
            }],
            vec![],
            0,
        )?,
        Isometry3::identity(),
        vec![],
        PhysicsConfig::default(),
    )
}

#[test]
fn selected_contact_impulses_do_not_cancel_opposing_normals_or_include_friction() {
    // Given: diagnostics whose normal forces cancel but whose contact impulses do not.
    let mut world = native_world().expect("native test operation");
    world.contact_samples[0] = vec![
        LinkContactSample {
            position: Vector3::zeros(),
            normal: Vector3::x(),
            force: Vector3::new(20.0, 100.0, 0.0),
            distance: 0.0,
        },
        LinkContactSample {
            position: Vector3::zeros(),
            normal: -Vector3::x(),
            force: Vector3::new(-20.0, -100.0, 0.0),
            distance: 0.0,
        },
    ];
    let sensor = ContactImpulseSensor::new(&[0]).expect("native test operation");

    // When: only the represented 5 ms substep is sampled.
    let impulses = sensor
        .observe(&world, 0.005)
        .expect("native test operation");

    // Then: both normal impulses count, despite zero resultant and large friction.
    assert!((impulses[0] - 0.2).abs() < 1e-12);
}

#[test]
fn contact_observation_rejects_invalid_selection_and_interval() {
    // Given: a sensor selecting the native root link.
    let native = PhysicsWorld::new(PhysicsConfig::default()).expect("native test operation");
    let sensor = ContactImpulseSensor::new(&[0]).expect("native test operation");

    // When: callers provide invalid boundary inputs.
    for selection in [vec![], vec![0, 0], vec![0, 1, 2, 3, 4]] {
        assert!(matches!(
            ContactImpulseSensor::new(&selection),
            Err(ContactObservationError::InvalidSelection)
        ));
    }
    for duration in [0.0, -0.1, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            sensor.observe(native.tessera_world(), duration),
            Err(ContactObservationError::InvalidSubstep)
        ));
    }

    // Then: stale native link indices never silently become background observations.
    assert!(matches!(
        ContactImpulseSensor::new(&[1])
            .expect("native test operation")
            .observe(native.tessera_world(), 0.005),
        Err(ContactObservationError::MissingLink(1))
    ));
}

#[test]
fn contact_observation_rejects_invalid_native_diagnostics() {
    // Given: a contact normal that cannot represent a solved unit normal.
    let mut world = native_world().expect("native test operation");
    world.contact_samples[0].push(LinkContactSample {
        position: Vector3::zeros(),
        normal: Vector3::zeros(),
        force: Vector3::x(),
        distance: 0.0,
    });
    let sensor = ContactImpulseSensor::new(&[0]).expect("native test operation");

    // When: native diagnostics are sampled.
    let result = sensor.observe(&world, 0.005);

    // Then: corrupt contact output is not advertised as a valid zero observation.
    assert!(matches!(
        result,
        Err(ContactObservationError::InvalidDiagnostics(0))
    ));
}

#[test]
fn host_sensor_observes_actual_native_contact_substep_not_enclosing_frame() {
    use sard::physics::urdf::{load_urdf_str, UrdfLoadOptions};

    // Given: a vertical slider held above the native ground by contact impulses.
    let xml = r#"<robot name="contact_slider">
      <link name="base"/>
      <link name="slider">
        <inertial><mass value="1"/><inertia ixx="1" ixy="0" ixz="0" iyy="1" iyz="0" izz="1"/></inertial>
        <collision><geometry><sphere radius="0.5"/></geometry></collision>
      </link>
      <joint name="slide" type="prismatic">
        <parent link="base"/><child link="slider"/><origin xyz="0 0 0.5"/>
        <axis xyz="0 0 1"/><limit lower="-5" upper="5" effort="100" velocity="100"/>
      </joint>
    </robot>"#;
    let loaded = load_urdf_str(
        xml,
        UrdfLoadOptions {
            world: PhysicsConfig {
                max_substep: 0.005,
                ..PhysicsConfig::default()
            },
            ..UrdfLoadOptions::default()
        },
    )
    .expect("native test operation");
    let mut physics = PhysicsWorld::from_tessera(loaded.world).expect("native test operation");
    let mut ecs = hecs::World::new();
    let sensor = ContactImpulseSensor::new(&[1, 0]).expect("native test operation");

    // When: native integration subdivides a 40 ms frame into 5 ms substeps.
    physics
        .step_with_efforts(&mut ecs, 0.04, &[0.0])
        .expect("native test operation");
    let observed = sensor
        .observe(physics.tessera_world(), 0.005)
        .expect("native test operation");

    // Then: the last support impulse is about m*g*dt, not m*g*frame_duration.
    assert!(observed[0] > 0.04 && observed[0] < 0.06);
    assert!(observed[1].abs() < 1e-12);
}
