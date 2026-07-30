pub mod builtin;
pub mod external;
pub mod protocol;
pub mod registry;

pub use builtin::{python, rust, typescript, BuiltinPlugin};
pub use external::ExternalPlugin;
pub use protocol::{GenerateRequest, GenerateResponse, GeneratedFile, PluginDiagnostic, PROTOCOL_VERSION};
pub use registry::PluginRegistry;
