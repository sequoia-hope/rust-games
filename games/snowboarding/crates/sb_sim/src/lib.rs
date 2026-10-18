//! The Snowboarding simulation: no engine, no clock, no threads. The client
//! steps a [`race::Session`] one fixed tick at a time and draws what it
//! holds.
//!
//! - [`course`]: the mountain, its kickers, trees and flags.
//! - [`rider`]: a rider's physics: carving, air, landing, crashing.
//! - [`tricks`]: naming and scoring jumps.
//! - [`ai`]: the CPU riders.
//! - [`race`]: a run: countdown, clock, checkpoints, results.

#![forbid(unsafe_code)]

pub mod ai;
pub mod course;
pub mod noise;
pub mod race;
pub mod rider;
pub mod tricks;

pub use glam::Vec3;
