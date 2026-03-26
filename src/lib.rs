//! GFD -- Generalized Fluid Dynamics multi-physics solver library.
//!
//! This library exposes a high-level API for running simulations programmatically.
//! AI agents and other consumers can call solver functions directly without
//! the CLI or external files.
//!
//! # Quick Start
//! ```rust,no_run
//! use gfd::api;
//!
//! // 1D heat conduction
//! let result = api::solve_conduction_1d(20, 1.0, 100.0, 200.0, 0.0);
//! assert_eq!(result.status, "converged");
//!
//! // Lid-driven cavity flow
//! let result = api::solve_cavity(20, 20, 100.0, 200);
//! println!("{}", serde_json::to_string_pretty(&result).unwrap());
//! ```

pub mod api;
