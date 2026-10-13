#![cfg(feature = "physics")]

use sard::ecs::components::{GlobalTransform, Transform};
use sard::physics::mjcf::{load_mjcf_str, MjcfLoadOptions};
use sard::physics::{publish_poses, PhysicsWorld};

#[test]
fn sard_mjcf_actuator_order_drives_native_coordinates_and_publishes_visual_pose() {
    // Given: actuator declaration order differs from native joint coordinate order.
    let xml = r#"<mujoco><option gravity="0 0 0"/>
      <worldbody><body name="root"><inertial mass="1" diaginertia="1 1 1"/>
        <body name="first"><joint name="a" type="slide" axis="1 0 0"/>
          <inertial mass="1" diaginertia="1 1 1"/></body>
        <body name="second"><joint name="b" type="hinge" axis="0 0 1"/>
          <inertial mass="1" diaginertia="1 1 1"/></body>
      </body></worldbody><actuator>
        <motor name="b_motor" joint="b" gear="-2"/>
        <position name="a_servo" joint="a" kp="25" kv="8"/>
      </actuator></mujoco>"#;
    let mut loaded = load_mjcf_str(xml, MjcfLoadOptions::default()).expect("native test operation");
    assert_eq!(
        loaded
            .actuators
            .entries()
            .iter()
            .map(|a| a.coordinate)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    let link_index = loaded
        .joints
        .iter()
        .find(|joint| joint.name == "a")
        .ok_or("missing imported slider joint")
        .expect("native test operation")
        .child_link;
    loaded
        .set_controls(&[1.0, 0.5])
        .expect("native test operation");
    let mut ecs = hecs::World::new();
    let visual = ecs.spawn((Transform::default(), GlobalTransform::default()));

    // When: the native owner evaluates actuators and publishes solved link poses.
    loaded.step(0.04).expect("native test operation");
    let link = loaded.world.link_poses().expect("native test operation")[link_index];
    publish_poses(&mut ecs, &[(visual, link)]).expect("native test operation");

    // Then: actuator controls affect the correct coordinates and reach Sard visuals.
    assert!(loaded.world.velocities[0] > 0.0);
    assert!(loaded.world.velocities[1] < 0.0);
    assert!(
        ecs.get::<&Transform>(visual)
            .expect("native test operation")
            .position
            .x
            > 0.0
    );
    assert!(
        (f64::from(
            ecs.get::<&Transform>(visual)
                .expect("native test operation")
                .position
                .x
        ) - link.translation.x)
            .abs()
            < 1e-6
    );
    // Moving the imported world into the bridge retains its solved coordinate state.
    let state = loaded.world.positions.clone();
    let bridge = PhysicsWorld::from_tessera(loaded.world).expect("native test operation");
    assert_eq!(bridge.tessera_world().positions, state);
}
