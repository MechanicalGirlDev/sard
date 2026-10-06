//! ECS Scene - Demonstrates ECS-based rendering with hecs
//!
//! Run with: cargo run -p ecs_scene

use std::sync::Arc;

use glam::Vec3;
use sard::ecs::components::rendering::{
    CameraComponent, FrustumCullable, LightComponent, MaterialHandle, MeshHandle, MeshRenderer,
    Visible,
};
use sard::ecs::components::transform::{GlobalTransform, Transform};
use sard::ecs::systems::{culling_system, render_system, transform_system};
use sard::renderer::light::LightType;
use sard::{
    screen_target, Camera, ClearState, ColorMaterial, FrameOutput, Mesh, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("ECS Scene"))?;

    struct State {
        world: hecs::World,
        initialized: bool,
    }

    let state = State {
        world: hecs::World::new(),
        initialized: false,
    };

    window.render_loop(state, |state, frame| {
        if !state.initialized {
            // Camera entity
            let camera = Camera::new_perspective(
                Vec3::new(3.0, 3.0, 3.0),
                Vec3::ZERO,
                Vec3::Y,
                45.0,
                1.0,
                0.1,
                100.0,
            );
            state.world.spawn((
                Transform::identity(),
                GlobalTransform::default(),
                CameraComponent {
                    camera,
                    active: true,
                },
            ));

            // Light entity
            state.world.spawn((
                Transform::from_position(Vec3::new(5.0, 5.0, 5.0)),
                GlobalTransform::default(),
                LightComponent {
                    light_type: LightType::Directional,
                    color: Vec3::ONE,
                    intensity: 1.0,
                },
            ));

            // Mesh entity (cube)
            let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");
            let mesh = Mesh::cube(frame.ctx, 1.0, [0.2, 0.6, 0.9]);

            state.world.spawn((
                Transform::identity(),
                GlobalTransform::default(),
                MeshRenderer {
                    mesh: MeshHandle(Arc::new(mesh)),
                    material: MaterialHandle(Arc::new(material)),
                    visible: true,
                    cast_shadow: true,
                    receive_shadow: true,
                },
                FrustumCullable,
                Visible,
            ));

            state.initialized = true;
        }

        // Update camera viewport
        for (cam,) in state.world.query_mut::<(&mut CameraComponent,)>() {
            if cam.active {
                cam.camera.set_viewport(frame.viewport);
            }
        }

        // ECS systems
        transform_system(&mut state.world);
        culling_system(&mut state.world);

        // Render
        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("ecs scene encoder"));
        {
            let clear = ClearState::color_and_depth([0.1, 0.1, 0.1, 1.0], 1.0);
            let mut pass = target.begin_render_pass(&mut encoder, clear);
            render_system(&state.world, frame.ctx, &mut pass);
        }
        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
