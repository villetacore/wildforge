//! Networking: the background WebSocket connection and the wire protocol.

pub mod connection;
pub mod protocol;

pub use connection::{Connection, NetEvent};
pub use protocol::{Catalog, ServerMsg};
