//! Native Tessera components for ECS physics entities.
//!
//! A rigid body owns its pose, mass, inertia, velocities, force, and colliders.
//! Add an optional [`ColliderMaterial`] to override its colliders' material.
//! [`crate::physics::PhysicsWorld`] publishes solved world-space poses to rendering
//! transforms; modify the body's native pose to teleport it.

pub use tessera_physics::articulated_world::SceneBody as RigidBody;
pub use tessera_physics::material::ColliderMaterial;
