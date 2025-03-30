use std::error::Error;
use std::process::Command;
use anyhow::{Result, Context};
use chrono::Utc;
use clap::Parser;
use influxdb::{Client, InfluxDbWriteable};
use std::time::{Duration};
use tokio::time;
use tracing::{info, warn, error};
use regex::Regex;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Time between runs in seconds
    #[clap(short, long, default_value = "5")]
    interval: u64,

    /// InfluxDB URL
    #[clap(long, default_value = "http://influxdb:8086")]
    influxdb_url: String,

    /// InfluxDB database
    #[clap(long, default_value = "internet_metrics")]
    influxdb_db: String,

    /// InfluxDB username (optional)
    #[clap(long)]
    influxdb_username: Option<String>,

    /// InfluxDB password (optional)
    #[clap(long)]
    influxdb_password: Option<String>,

    /// Latency test URL
    #[clap(long, default_value = "google.com")]
    latency_url: String,
}

#[derive(Debug, InfluxDbWriteable)]
struct InternetMetrics {
    time: chrono::DateTime<Utc>,
    #[influxdb(tag)]
    measurement_type: String,
    latency_ms: Option<f64>,
}

async fn measure_latency(url: &str) -> Result<f64, Box<dyn Error>> {
    let url_owned = url.to_owned();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("ping")
            .arg("-c")
            .arg("4")  // perform 4 pings
            .arg(url_owned)
            .output()
    }).await?
        .context("Failed to spawn ping command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Ping command failed: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // This regex handles both Linux ("rtt") and macOS ("round-trip") summary lines.
    let re = Regex::new(r"(?:rtt|round-trip).* = ([0-9.]+)/([0-9.]+)/([0-9.]+)/([0-9.]+) ?ms")?;
    if let Some(captures) = re.captures(&stdout) {
        // The average latency is the second captured group.
        if let Some(avg_match) = captures.get(2) {
            let avg = avg_match.as_str().parse::<f64>()
                .context("Failed to parse average latency time as float")?;
            return Ok(avg);
        }
    }
    Err("Failed to parse ping output".into())
}

async fn run_measurements(args: &Args) -> Result<InternetMetrics> {
    // Measure latency
    info!("Measuring latency to {}", args.latency_url);
    let latency = match measure_latency(&args.latency_url).await {
        Ok(latency) => {
            info!("Latency: {:.2} ms", latency);
            Some(latency)
        }
        Err(e) => {
            warn!("Failed to measure latency: {}", e);
            None
        }
    };

    Ok(InternetMetrics {
        time: Utc::now(),
        measurement_type: "internet_performance".to_string(),
        latency_ms: latency,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    info!("Starting internet-monitor with interval of {} seconds", args.interval);

    // Clone the strings we need for InfluxDB client to avoid partial move issues
    let influxdb_url = args.influxdb_url.clone();
    let influxdb_db = args.influxdb_db.clone();
    let influxdb_username = args.influxdb_username.clone();
    let influxdb_password = args.influxdb_password.clone();

    // Create InfluxDB client
    let mut influx_client = Client::new(influxdb_url, influxdb_db);
    if let (Some(username), Some(password)) = (&influxdb_username, &influxdb_password) {
        influx_client = influx_client.with_auth(username, password);
    }

    // Attempt to ping InfluxDB
    match influx_client.ping().await {
        Ok(_) => info!("Successfully connected to InfluxDB"),
        Err(e) => warn!("Could not ping InfluxDB, but will try to write anyway: {}", e),
    }

    // Main measurement loop
    let mut iteration = 0;
    loop {
        iteration += 1;
        info!("Starting measurement iteration {}", iteration);

        match run_measurements(&args).await {
            Ok(metrics) => {
                // Write to InfluxDB
                match influx_client.query(metrics.into_query("internet_metrics")).await {
                    Ok(_) => info!("Successfully wrote metrics to InfluxDB"),
                    Err(e) => {
                        error!("Failed to write metrics to InfluxDB: {}", e);
                        // Don't exit on InfluxDB errors
                    }
                }
            }
            Err(e) => {
                error!("Failed to run measurements: {}", e);
                // Don't exit on measurement errors
            }
        }

        info!("Completed measurement iteration {}. Sleeping for {} seconds...",
             iteration, args.interval);

        // Wait for the next interval
        time::sleep(Duration::from_secs(args.interval)).await;

        info!("Woke up from sleep after iteration {}", iteration);
    }
}