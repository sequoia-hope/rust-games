//! The new generator (PLAN.md Phase 3): genre profiles from
//! research/RULES.md, an arrangement planner that gives every instrument
//! its own timeline, and candidates scored for surprise. It writes a
//! [`crate::score::Song`], which [`crate::song::SongPlayer`] plays.

pub mod surprise;
pub mod theory;
