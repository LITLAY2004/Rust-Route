//! RustRoute: Simple RIP Implementation in Rust
//! 
//! This library implements a simple and practical RIP routing protocol
//! focused on core functionality and ease of use.

pub mod router;
pub mod network;
pub mod protocol;
pub mod routing_table;
pub mod metrics;
pub mod cli;
pub mod testing;

use std::error::Error;
use std::fmt;

/// RustRoute specific error types
#[derive(Debug)]
pub enum RustRouteError {
    NetworkError(String),
    RoutingError(String),
    ConfigError(String),
    ProtocolError(String),
    InvalidInput(String),
}

impl fmt::Display for RustRouteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RustRouteError::NetworkError(msg) => write!(f, "Network Error: {}", msg),
            RustRouteError::RoutingError(msg) => write!(f, "Routing Error: {}", msg),
            RustRouteError::ConfigError(msg) => write!(f, "Config Error: {}", msg),
            RustRouteError::ProtocolError(msg) => write!(f, "Protocol Error: {}", msg),
            RustRouteError::InvalidInput(msg) => write!(f, "Invalid Input: {}", msg),
        }
    }
}

impl Error for RustRouteError {}

/// Result type for RustRoute operations
pub type RustRouteResult<T> = Result<T, RustRouteError>;
