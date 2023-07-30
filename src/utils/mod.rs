use chrono::Local;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::time::Instant;
use webhook::client::WebhookClient;

pub fn get_time_str() -> String {
    let now = Local::now();
    now.format("%m/%d/%Y, %I:%M:%S %p").to_string()
}

pub async fn remove_id_index() -> Result<(), Box<dyn std::error::Error>> {
    let dirname = "/home/backup/Sofi/";
    let mut reader = fs::read_dir(dirname).await.unwrap();
    loop {
        if let Some(entry) = reader.next_entry().await? {
            let filename = entry.file_name();
            let filename = filename.to_str().unwrap();
            let pat = format!("{}/{}", dirname, filename);
            if filename.ends_with(".json") {
                let content = fs::read_to_string(&pat).await?;
                on_file_content(content, &pat).await?;
            }
        } else {
            break;
        }
    }
    Ok(())
}

pub async fn dir_size(directory: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let mut total_size = 0;
    let mut entries = fs::read_dir(directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        println!("{}", path.to_str().unwrap());
        if path.is_file() {
            let metadata = entry.metadata().await?;
            total_size += metadata.len();
        }
    }
    Ok(total_size)
}

pub async fn on_file_content(content: String, pat: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Debug, Deserialize, Serialize)]
    struct Index {
        name: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct Object {
        indexes: Vec<Index>,
    }

    let mut object: Object = serde_json::from_str(&content)?;
    object.indexes.retain(|index| index.name != "_id_");
    let updated_content = serde_json::to_string(&object)?;
    fs::write(pat, updated_content).await?;
    Ok(())
}

pub fn generate_filename() -> String {
    let now = Local::now();
    let date_part = now.format("%Y-%m-%d").to_string();
    let time_part = now.format("%I-%M-%S_%p").to_string();

    format!("{}_{}.zip", date_part, time_part)
}

pub fn elapsed_time(start: Instant) -> String {
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs();
    let mins = secs / 60;
    let secs = secs % 60;
    let millis = elapsed.subsec_millis();
    format!("{}m {}s {}ms", mins, secs, millis)
}

pub async fn send_webhook_message(client: &WebhookClient, msg: &str) {
    let fut = client
        .send(|message| {
            message
                .username("Sofi-Backup")
                .content(format!("`{}`: {}", get_time_str(), msg).as_str())
        })
        .await;

    if let Err(why) = fut {
        eprintln!("Error sending message: {:?}", why);
    }
}
