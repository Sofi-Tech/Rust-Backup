use chrono::prelude::*;
use chrono::Local;
use serde::{Deserialize, Serialize};
use tokio::fs::{read_dir, read_to_string, remove_file, write};
use tokio::time::Instant;
use webhook::client::WebhookClient;

pub fn get_time_str() -> String {
    let now = Local::now();
    now.format("%m/%d/%Y, %I:%M:%S %p").to_string()
}

pub async fn remove_id_index(database_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dirname = format!("/home/backup/{}/", database_name).to_string();
    let mut reader = read_dir(dirname.clone()).await?; // Clone the dirname here
    while let Some(entry) = reader.next_entry().await? {
        let filename = entry.file_name();
        let filename = filename.to_str().unwrap();
        let pat = format!("{}/{}", dirname, filename);
        if filename.ends_with(".json") {
            let content = read_to_string(&pat).await?;
            on_file_content(content, &pat).await?;
        }
    }
    Ok(())
}

pub async fn dir_size(directory: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut total_size = 0;
    let mut entries = read_dir(directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            let metadata = entry.metadata().await?;
            total_size += metadata.len();
        }
    }

    let size_in_mb = total_size as f64 / 1024.0 / 1024.0;
    let size_in_gb = total_size as f64 / 1024.0 / 1024.0 / 1024.0;

    let size_formatted = if size_in_gb >= 1.0 {
        format!("{:.2} GB", size_in_gb)
    } else {
        format!("{:.2} MB", size_in_mb)
    };

    Ok(size_formatted)
}

pub async fn on_file_content(content: String, pat: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Debug, Deserialize, Serialize)]
    struct Index {
        name: String,
        key: serde_json::Value,
        v: serde_json::Value,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct Object {
        indexes: Vec<Index>,
    }

    let mut object: Object = serde_json::from_str(&content)?;
    let mut new_indexes: Vec<Index> = Vec::new();

    for index in object.indexes.iter() {
        if index.name != "_id_" {
            new_indexes.push(Index {
                name: index.name.clone(),
                key: index.key.clone(),
                v: index.v.clone(),
            })
        }
    }
    object.indexes = new_indexes;
    let new_content = serde_json::to_string(&object)?;
    write(pat, new_content).await?;
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

pub fn command_success(output: &std::process::Output, msg: &str) -> bool {
    if output.status.success() {
        println!("{}", msg);
        true
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error executing command: {}", stderr);
        false
    }
}

pub fn find_oldest_file(files_list: &str) -> &str {
    let filenames: Vec<&str> = files_list
        .split('\n')
        .filter(|filename| !filename.is_empty())
        .collect();

    if filenames.len() < 6 {
        return "";
    }

    let timestamps: Vec<i64> = filenames
        .iter()
        .map(|filename| {
            let date_string = filename.split('_').next().unwrap();
            let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d").unwrap();
            date.and_hms_opt(0, 0, 0).unwrap().timestamp()
        })
        .collect();

    let oldest_index = timestamps
        .iter()
        .position(|&ts| ts == *timestamps.iter().min().unwrap())
        .unwrap();

    filenames[oldest_index]
}

pub async fn delete_files_if_more_than_3(
    directory: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries = read_dir(directory).await?;
    let mut files: Vec<_> = Vec::new();

    // Collect the files along with their last modified timestamp
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            let metadata = entry.metadata().await?;
            let last_modified = metadata.modified()?;
            files.push((path, last_modified));
        }
    }

    // Sort files by last modified timestamp in descending order
    files.sort_by(|(_, t1), (_, t2)| t2.cmp(t1));

    // Keep only the three latest files, delete the rest
    for (i, (path, _)) in files.iter().enumerate() {
        if i >= 3 {
            remove_file(path).await?;
            println!("Deleted file: {:?}", path);
        }
    }

    Ok(())
}
