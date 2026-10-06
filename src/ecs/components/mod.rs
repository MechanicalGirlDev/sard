//! ECS components (transform, rendering, physics).

#[cfg(feature = "physics")]
pub mod physics;
pub mod rendering;
pub mod transform;

#[cfg(feature = "physics")]
pub use physics::*;
pub use rendering::*;
pub use transform::*;
