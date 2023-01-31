// Linux only
#![cfg(target_os = "linux")]

// Catching CPU exception during the execution
pub mod exception_handler;
pub use exception_handler::*;

// Generate metadata for each objective
pub mod gen_metadata;
pub use gen_metadata::*;

// Generate initial inputs from provided UEFI images
pub mod initial_inputs;
pub use initial_inputs::*;

// Resetting the state aka. snapshotting in between fuzzing test-cases
pub mod reset_state;
pub use reset_state::*;

// Tunneling comparisons by statically/dynamically setting register values
pub mod tunneling;
pub use tunneling::*;

// Parsing the YAML config
pub mod yaml_conf;
pub use yaml_conf::*;
