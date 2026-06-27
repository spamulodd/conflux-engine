//! JSON line protocol over Windows named pipes (Unix socket fallback on other OS).

pub mod client;
pub mod protocol;
pub mod server;

pub use client::IpcClient;
pub use protocol::{
    default_endpoint, Request, Response, ResponseData, ResponseStatus, DEFAULT_PIPE_NAME,
    PROTOCOL_VERSION,
};
pub use server::{EngineState, IpcServer};
