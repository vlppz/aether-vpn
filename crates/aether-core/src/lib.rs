//! Platform-independent core of Aether: everything that can be expressed as
//! pure data transformations lives here so it can be tested without a GUI,
//! a network or an Xray binary.

pub mod config;
pub mod country;
pub mod error;
pub mod helper_proto;
pub mod links;
pub mod model;
pub mod routing;
pub mod subscription;

pub use config::{build_xray_config, CoreSettings, DnsSettings, TunSettings, TunnelMode};
pub use error::{Error, Result};
pub use links::{parse_link, parse_many, to_link};
pub use model::{Network, Outbound, Reality, Security, Server, Stream, Subscription, Tls, UserInfo};
pub use routing::{Rule, RuleKind, RuleTarget, RoutingPreset, RoutingSettings};
