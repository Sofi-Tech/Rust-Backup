mod utils;

use std::env;
use std::process::Command;
use tokio::time::Instant;
use utils::{dir_size, elapsed_time, generate_filename, get_time_str, send_webhook_message};
use webhook::client::WebhookClient;

#[tokio::main]
async fn main() {
    let instant = Instant::now();
    let filename = generate_filename();
    println!("Filename: {}", filename);

    let url = match env::var("BACKUP_WEBHOOK_URL") {
        Ok(value) => value.to_string(),
        Err(_) => panic!("BACKUP_WEBHOOK_URL must be set"),
    };

    let client: WebhookClient = WebhookClient::new(url.as_str());

    send_webhook_message(&client, "Cron job started, Removing all the files.").await;
    println!("{}: Cron job started.", get_time_str());

    let output = Command::new("sudo")
        .args(&["rm -rf", "/home/test/Sofi/*"])
        .output()
        .expect("failed to execute process");

    if let Ok(size) = dir_size("./").await {
        println!("Total directory size: {} bytes", size);
    } else {
        eprintln!("Error calculating directory size");
    }
    println!("Time elapsed: {}", elapsed_time(instant));
}
