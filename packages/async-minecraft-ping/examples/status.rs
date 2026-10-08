use anyhow::Result;
use clap::Parser;

use async_minecraft_ping::ConnectionConfig;

#[derive(Debug, Parser)]
#[command(name = "example")]
struct Args {
    /// Server to connect to
    address: String,

    /// Port to connect to
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,

    /// Enable SRV record lookup (requires 'srv' feature)
    #[cfg(feature = "srv")]
    #[arg(long = "srv")]
    srv_lookup: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut config = ConnectionConfig::build(args.address);
    if let Some(port) = args.port {
        config = config.with_port(port);
    }
    #[cfg(feature = "srv")]
    if args.srv_lookup {
        config = config.with_srv_lookup();
    }

    let connection = config.connect().await?;

    let connection = connection.status().await?;

    println!(
        "{} of {} player(s) online",
        connection.status.players.online, connection.status.players.max
    );

    connection.ping(42).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn address_and_both_port_flags_preserve_the_cli() {
        let args = Args::try_parse_from(["example", "localhost"]).unwrap();
        assert_eq!(args.address, "localhost");
        assert_eq!(args.port, None);
        for flag in ["-p", "--port"] {
            let args = Args::try_parse_from(["example", "localhost", flag, "25566"]).unwrap();
            assert_eq!(args.address, "localhost");
            assert_eq!(args.port, Some(25566));
        }
        assert!(Args::try_parse_from(["example"]).is_err());
        assert!(Args::try_parse_from(["example", "localhost", "-p", "65536"]).is_err());
    }

    #[test]
    fn help_is_available_without_a_network_connection() {
        for flag in ["-h", "--help"] {
            let error = Args::try_parse_from(["example", flag]).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            let help = error.to_string();
            assert!(help.contains("<ADDRESS>"));
            assert!(help.contains("--port"));
            assert_eq!(help.contains("--srv"), cfg!(feature = "srv"));
        }
    }

    #[cfg(feature = "srv")]
    #[test]
    fn srv_flag_is_available_when_enabled() {
        let args = Args::try_parse_from(["example", "localhost", "--srv"]).unwrap();
        assert!(args.srv_lookup);
    }

    #[cfg(not(feature = "srv"))]
    #[test]
    fn srv_flag_is_rejected_without_the_feature() {
        let error = Args::try_parse_from(["example", "localhost", "--srv"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }
}
