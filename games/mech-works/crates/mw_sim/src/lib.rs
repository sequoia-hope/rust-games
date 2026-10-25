//! The Mech Works simulation: the one source of physics truth for training
//! (through `mw_sim_py`) and for the game. The mech is a Rapier multibody
//! whose joints are driven by PD controllers through a motor, driver and
//! thermal model; nothing about how it walks is animated.

pub mod actuator;
pub mod env;
pub mod mech;
pub mod rng;
pub mod rubble;
pub mod sim;
pub mod terrain;

pub use rapier3d;
