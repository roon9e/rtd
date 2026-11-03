//! Edge configuration read from the command line and environment.
//!
//! The edge takes its settings from `--flag value` (or `--flag=value`)
//! arguments, each with an `RTD_EDGE_*` environment fallback. The RSA private
//! key is read and parsed lazily by [`EdgeConfig::load_rsa_key`], so an invalid
//! path is reported when the edge starts rather than when arguments are parsed.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rsa::RsaPrivateKey;
use rtd_server::session::{
    DEFAULT_AUTH_KEY_CACHE_MAX_ENTRIES, DEFAULT_AUTH_KEY_CACHE_TTL,
    DEFAULT_TEMP_KEY_CACHE_MAX_ENTRIES, DEFAULT_TEMP_KEY_CACHE_TTL,
};
use rtd_tlprofile::CANONICAL_LAYER;
use rtd_transport::Secret;

use crate::error::EdgeError;

/// The default bind address.
pub const DEFAULT_BIND: &str = "0.0.0.0:443";

/// The default serving datacenter identifier.
pub const DEFAULT_DC: i32 = 2;

/// The default public port advertised in `help.getConfig`.
pub const ADVERTISED_PORT: i32 = 443;

/// The default layer profile the edge admits bare requests under.
///
/// This must match the layer the backing core can re-admit. A gramsrv v2 core
/// pins the layer-229 schema, so a 232 default makes every bare request (for
/// example `auth.bindTempAuthKey` or a ping) fail admission at the core.
pub const DEFAULT_LAYER: i32 = CANONICAL_LAYER;

/// The `--help` text.
pub const USAGE: &str = "\
rtd-edge - MTProto edge sidecar

USAGE:
    rtd-edge [OPTIONS]

OPTIONS:
    --bind <ADDR>      Address to listen on [env: RTD_EDGE_BIND] [default: 0.0.0.0:443]
    --dc <ID>          Serving datacenter id [env: RTD_EDGE_DC] [default: 2]
    --layer <N>        Layer profile for bare requests [env: RTD_EDGE_LAYER] [default: 232]
    --rsa-key <PATH>   PEM RSA private key file [env: RTD_EDGE_RSA_KEY]
    --secret <HEX>     MTProxy secret: plain, `dd`, or `ee` [env: RTD_EDGE_SECRET]
    --websocket        Enable WebSocket ingress [env: RTD_EDGE_WEBSOCKET]
    --websocket-origin <ORIGIN>  Allow a WebSocket origin (repeatable)
                       [env: RTD_EDGE_WEBSOCKET_ORIGINS]
    --core <ENDPOINT>  Core CoreExecService endpoint, e.g. http://127.0.0.1:50051
                       [env: RTD_EDGE_CORE]
    --core-token <T>   Bearer token for the core CoreExecService [env: RTD_EDGE_CORE_TOKEN]
    --advertise-ip <IP>   Public IP advertised in help.getConfig [env: RTD_EDGE_ADVERTISE_IP]
    --advertise-port <P>  Public port advertised in help.getConfig [env: RTD_EDGE_ADVERTISE_PORT]
    --strict-dc        Require clients to match this DC label [env: RTD_EDGE_STRICT_DC]
    --instance-id <ID> Instance identity for logs [env: RTD_EDGE_INSTANCE_ID]
    --exchange-test-params  Use fixed test DH parameters [env: RTD_EDGE_EXCHANGE_TEST_PARAMS]
    --auth-key-cache-max <N>   Max cached auth keys [env: RTD_EDGE_AUTH_KEY_CACHE_MAX]
    --auth-key-cache-ttl <S>   Auth-key cache TTL seconds [env: RTD_EDGE_AUTH_KEY_CACHE_TTL]
    --temp-key-cache-max <N>   Max cached temp keys [env: RTD_EDGE_TEMP_KEY_CACHE_MAX]
    --temp-key-cache-ttl <S>   Temp-key cache TTL seconds [env: RTD_EDGE_TEMP_KEY_CACHE_TTL]
    -h, --help         Print this help
    -V, --version      Print the version";

/// The edge's runtime configuration.
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// The address the edge listens on.
    pub bind: String,
    /// The datacenter this edge serves.
    pub dc: i32,
    /// The layer profile the edge admits bare requests under.
    pub layer: i32,
    /// The path to the PEM RSA private key used for the key exchange.
    pub rsa_key_path: PathBuf,
    /// The optional MTProxy secret, parsed from its human string.
    pub secret: Option<Secret>,
    /// Whether WebSocket ingress is enabled.
    pub websocket: bool,
    /// WebSocket origins allowed to upgrade; empty accepts any origin.
    pub websocket_origins: Vec<String>,
    /// The optional core gRPC endpoint; the in-process core is used when unset.
    pub core_endpoint: Option<String>,
    /// The optional bearer token presented to the core's `CoreExecService`.
    pub core_token: Option<String>,
    /// Whether the key exchange uses the fixed test parameters.
    pub exchange_test_params: bool,
    /// Maximum number of cached authorization keys.
    pub auth_key_cache_max: usize,
    /// Time-to-live of a cached authorization key.
    pub auth_key_cache_ttl: Duration,
    /// Maximum number of cached temporary keys.
    pub temp_key_cache_max: usize,
    /// Time-to-live of a cached temporary key.
    pub temp_key_cache_ttl: Duration,
    /// The public IP advertised in `help.getConfig`.
    pub advertise_ip: String,
    /// The public port advertised in `help.getConfig`.
    pub advertise_port: i32,
    /// Whether the client's datacenter label must match this edge's.
    pub strict_dc_check: bool,
    /// The instance identity used in logs; generated when unset.
    pub instance_id: Option<String>,
}

impl EdgeConfig {
    /// Parses `args` (excluding the program name) with `env` as the environment
    /// fallback.
    ///
    /// # Errors
    ///
    /// Returns [`EdgeError::Usage`] for an unknown flag, a missing or malformed
    /// value, an invalid datacenter or secret, or a missing required
    /// `--rsa-key`.
    pub fn parse(
        args: impl IntoIterator<Item = String>,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, EdgeError> {
        let args: Vec<String> = args.into_iter().collect();
        let mut bind = None;
        let mut dc = None;
        let mut layer = None;
        let mut rsa_key = None;
        let mut secret = None;
        let mut websocket = None;
        let mut websocket_origins: Vec<String> = Vec::new();
        let mut core = None;
        let mut core_token = None;
        let mut exchange_test_params = None;
        let mut auth_key_cache_max = None;
        let mut auth_key_cache_ttl = None;
        let mut temp_key_cache_max = None;
        let mut temp_key_cache_ttl = None;
        let mut advertise_ip = None;
        let mut advertise_port = None;
        let mut strict_dc = None;
        let mut instance_id = None;

        let mut index = 0;
        while index < args.len() {
            let (name, inline) = match args[index].split_once('=') {
                Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
                None => (args[index].clone(), None),
            };
            match name.as_str() {
                "--bind" => bind = Some(take(&args, &mut index, inline, &name)?),
                "--dc" => dc = Some(take(&args, &mut index, inline, &name)?),
                "--layer" => layer = Some(take(&args, &mut index, inline, &name)?),
                "--rsa-key" => rsa_key = Some(take(&args, &mut index, inline, &name)?),
                "--secret" => secret = Some(take(&args, &mut index, inline, &name)?),
                "--core" => core = Some(take(&args, &mut index, inline, &name)?),
                "--core-token" => core_token = Some(take(&args, &mut index, inline, &name)?),
                "--websocket" => {
                    websocket = Some(match inline {
                        Some(value) => parse_bool(&value)?,
                        None => true,
                    });
                }
                "--websocket-origin" => {
                    websocket_origins.push(take(&args, &mut index, inline, &name)?);
                }
                "--exchange-test-params" => {
                    exchange_test_params = Some(match inline {
                        Some(value) => parse_bool(&value)?,
                        None => true,
                    });
                }
                "--auth-key-cache-max" => {
                    auth_key_cache_max = Some(take(&args, &mut index, inline, &name)?);
                }
                "--auth-key-cache-ttl" => {
                    auth_key_cache_ttl = Some(take(&args, &mut index, inline, &name)?);
                }
                "--temp-key-cache-max" => {
                    temp_key_cache_max = Some(take(&args, &mut index, inline, &name)?);
                }
                "--temp-key-cache-ttl" => {
                    temp_key_cache_ttl = Some(take(&args, &mut index, inline, &name)?);
                }
                "--advertise-ip" => {
                    advertise_ip = Some(take(&args, &mut index, inline, &name)?);
                }
                "--advertise-port" => {
                    advertise_port = Some(take(&args, &mut index, inline, &name)?);
                }
                "--strict-dc" => {
                    strict_dc = Some(match inline {
                        Some(value) => parse_bool(&value)?,
                        None => true,
                    });
                }
                "--instance-id" => {
                    instance_id = Some(take(&args, &mut index, inline, &name)?);
                }
                other => {
                    return Err(EdgeError::Usage(format!("unknown argument {other:?}")));
                }
            }
            index += 1;
        }

        let bind = bind
            .or_else(|| env("RTD_EDGE_BIND"))
            .unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let dc = match dc.or_else(|| env("RTD_EDGE_DC")) {
            Some(value) => value
                .parse()
                .map_err(|_| EdgeError::Usage(format!("invalid --dc value {value:?}")))?,
            None => DEFAULT_DC,
        };
        let layer = match layer.or_else(|| env("RTD_EDGE_LAYER")) {
            Some(value) => value
                .parse()
                .map_err(|_| EdgeError::Usage(format!("invalid --layer value {value:?}")))?,
            None => DEFAULT_LAYER,
        };
        let rsa_key = rsa_key.or_else(|| env("RTD_EDGE_RSA_KEY")).ok_or_else(|| {
            EdgeError::Usage("the --rsa-key option (or RTD_EDGE_RSA_KEY) is required".to_owned())
        })?;
        let secret = secret
            .or_else(|| env("RTD_EDGE_SECRET"))
            .map(|value| {
                Secret::parse(&value)
                    .map_err(|error| EdgeError::Usage(format!("invalid MTProxy secret: {error}")))
            })
            .transpose()?;
        let websocket = match websocket {
            Some(value) => value,
            None => env("RTD_EDGE_WEBSOCKET")
                .map(|value| parse_bool(&value))
                .transpose()?
                .unwrap_or(false),
        };
        if websocket_origins.is_empty()
            && let Some(value) = env("RTD_EDGE_WEBSOCKET_ORIGINS")
        {
            websocket_origins = value
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect();
        }
        let core_endpoint = core.or_else(|| env("RTD_EDGE_CORE"));
        let core_token = core_token.or_else(|| env("RTD_EDGE_CORE_TOKEN"));
        let exchange_test_params = match exchange_test_params {
            Some(value) => value,
            None => env("RTD_EDGE_EXCHANGE_TEST_PARAMS")
                .map(|value| parse_bool(&value))
                .transpose()?
                .unwrap_or(false),
        };
        let auth_key_cache_max = parse_usize(
            auth_key_cache_max.or_else(|| env("RTD_EDGE_AUTH_KEY_CACHE_MAX")),
            DEFAULT_AUTH_KEY_CACHE_MAX_ENTRIES,
            "--auth-key-cache-max",
        )?;
        let auth_key_cache_ttl = parse_secs(
            auth_key_cache_ttl.or_else(|| env("RTD_EDGE_AUTH_KEY_CACHE_TTL")),
            DEFAULT_AUTH_KEY_CACHE_TTL,
            "--auth-key-cache-ttl",
        )?;
        let temp_key_cache_max = parse_usize(
            temp_key_cache_max.or_else(|| env("RTD_EDGE_TEMP_KEY_CACHE_MAX")),
            DEFAULT_TEMP_KEY_CACHE_MAX_ENTRIES,
            "--temp-key-cache-max",
        )?;
        let temp_key_cache_ttl = parse_secs(
            temp_key_cache_ttl.or_else(|| env("RTD_EDGE_TEMP_KEY_CACHE_TTL")),
            DEFAULT_TEMP_KEY_CACHE_TTL,
            "--temp-key-cache-ttl",
        )?;
        let advertise_ip = advertise_ip
            .or_else(|| env("RTD_EDGE_ADVERTISE_IP"))
            .unwrap_or_else(|| "127.0.0.1".to_owned());
        let advertise_port = match advertise_port.or_else(|| env("RTD_EDGE_ADVERTISE_PORT")) {
            Some(value) => value.parse().map_err(|_| {
                EdgeError::Usage(format!("invalid --advertise-port value {value:?}"))
            })?,
            None => ADVERTISED_PORT,
        };
        let strict_dc_check = match strict_dc {
            Some(value) => value,
            None => env("RTD_EDGE_STRICT_DC")
                .map(|value| parse_bool(&value))
                .transpose()?
                .unwrap_or(false),
        };

        Ok(Self {
            bind,
            dc,
            layer,
            rsa_key_path: PathBuf::from(rsa_key),
            secret,
            websocket,
            websocket_origins,
            core_endpoint,
            core_token,
            exchange_test_params,
            auth_key_cache_max,
            auth_key_cache_ttl,
            temp_key_cache_max,
            temp_key_cache_ttl,
            advertise_ip,
            advertise_port,
            strict_dc_check,
            instance_id: instance_id.or_else(|| env("RTD_EDGE_INSTANCE_ID")),
        })
    }

    /// Reads and parses the configured PEM RSA private key.
    ///
    /// # Errors
    ///
    /// Returns [`EdgeError::KeyFile`] when the file cannot be read and
    /// [`EdgeError::KeyParse`] when it is not a PEM RSA private key.
    pub fn load_rsa_key(&self) -> Result<Arc<RsaPrivateKey>, EdgeError> {
        let pem =
            std::fs::read_to_string(&self.rsa_key_path).map_err(|source| EdgeError::KeyFile {
                path: self.rsa_key_path.clone(),
                source,
            })?;
        let key = rtd_crypto::parse_rsa_private_key(&pem).map_err(EdgeError::KeyParse)?;
        Ok(Arc::new(key))
    }
}

/// Parses an optional numeric option, falling back to `default`.
fn parse_usize(value: Option<String>, default: usize, flag: &str) -> Result<usize, EdgeError> {
    match value {
        Some(value) => value
            .parse()
            .map_err(|_| EdgeError::Usage(format!("invalid {flag} value {value:?}"))),
        None => Ok(default),
    }
}

/// Parses an optional seconds option into a [`Duration`], falling back to `default`.
fn parse_secs(value: Option<String>, default: Duration, flag: &str) -> Result<Duration, EdgeError> {
    match value {
        Some(value) => value
            .parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|_| EdgeError::Usage(format!("invalid {flag} value {value:?}"))),
        None => Ok(default),
    }
}

/// Returns the next option value, preferring an inline `--flag=value`.
fn take(
    args: &[String],
    index: &mut usize,
    inline: Option<String>,
    name: &str,
) -> Result<String, EdgeError> {
    if let Some(value) = inline {
        return Ok(value);
    }
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| EdgeError::Usage(format!("missing value for {name}")))
}

/// Parses a boolean flag value.
fn parse_bool(value: &str) -> Result<bool, EdgeError> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(EdgeError::Usage(format!("invalid boolean {value:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_BIND, EdgeConfig, EdgeError};
    use std::path::PathBuf;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn parse(args: &[&str], env: impl Fn(&str) -> Option<String>) -> Result<EdgeConfig, EdgeError> {
        EdgeConfig::parse(args.iter().map(|arg| (*arg).to_owned()), env)
    }

    #[test]
    fn parses_explicit_options() {
        let config = parse(
            &[
                "--bind",
                "127.0.0.1:9000",
                "--dc",
                "5",
                "--rsa-key",
                "key.pem",
                "--websocket",
                "--core",
                "http://127.0.0.1:50051",
                "--core-token",
                "s3cret",
            ],
            no_env,
        )
        .expect("config");
        assert_eq!(config.bind, "127.0.0.1:9000");
        assert_eq!(config.dc, 5);
        assert_eq!(config.rsa_key_path, PathBuf::from("key.pem"));
        assert!(config.websocket);
        assert_eq!(
            config.core_endpoint.as_deref(),
            Some("http://127.0.0.1:50051")
        );
        assert_eq!(config.core_token.as_deref(), Some("s3cret"));
        assert!(config.secret.is_none());
    }

    #[test]
    fn falls_back_to_environment() {
        let env = |name: &str| match name {
            "RTD_EDGE_RSA_KEY" => Some("env-key.pem".to_owned()),
            "RTD_EDGE_DC" => Some("7".to_owned()),
            "RTD_EDGE_WEBSOCKET" => Some("true".to_owned()),
            "RTD_EDGE_CORE_TOKEN" => Some("env-token".to_owned()),
            _ => None,
        };
        let config = parse(&[], env).expect("config");
        assert_eq!(config.bind, DEFAULT_BIND);
        assert_eq!(config.dc, 7);
        assert!(config.websocket);
        assert_eq!(config.rsa_key_path, PathBuf::from("env-key.pem"));
        assert_eq!(config.core_token.as_deref(), Some("env-token"));
    }

    #[test]
    fn requires_a_key() {
        let error = parse(&[], no_env).expect_err("missing key");
        assert!(matches!(error, EdgeError::Usage(_)));
    }

    #[test]
    fn parses_inline_values_and_secret() {
        let config = parse(
            &[
                "--rsa-key=key.pem",
                "--dc=3",
                "--secret=dd00112233445566778899aabbccddeeff",
            ],
            no_env,
        )
        .expect("config");
        assert_eq!(config.dc, 3);
        assert!(config.secret.is_some());
    }

    #[test]
    fn rejects_unknown_argument() {
        let error = parse(&["--nope"], no_env).expect_err("unknown");
        assert!(matches!(error, EdgeError::Usage(_)));
    }

    #[test]
    fn rejects_bad_secret_and_datacenter() {
        assert!(matches!(
            parse(&["--rsa-key", "k", "--secret", "zz"], no_env).expect_err("secret"),
            EdgeError::Usage(_)
        ));
        assert!(matches!(
            parse(&["--rsa-key", "k", "--dc", "not-a-number"], no_env).expect_err("dc"),
            EdgeError::Usage(_)
        ));
    }

    #[test]
    fn missing_key_file_is_reported() {
        let config = parse(&["--rsa-key", "does-not-exist.pem"], no_env).expect("config");
        assert!(matches!(
            config.load_rsa_key().expect_err("missing file"),
            EdgeError::KeyFile { .. }
        ));
    }
}
