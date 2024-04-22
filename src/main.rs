mod utils;

use std::env;
use std::process::Command;
use tokio::time::Instant;
use utils::{
    command_success, dir_size, elapsed_time, find_oldest_file, generate_filename, get_time_str,
    remove_id_index, send_webhook_message,
};
use webhook::client::WebhookClient;

#[tokio::main]
async fn main() {
    let instant = Instant::now();
    let database_name = "Sofi";
    let destination_dir = "rustBackup/";
    let filename = generate_filename();

    let url = match env::var("BACKUP_WEBHOOK_URL") {
        Ok(value) => value.to_string(),
        Err(_) => panic!("BACKUP_WEBHOOK_URL must be set"),
    };

    let mongodb_uri = match env::var("MONGODB_URI") {
        Ok(value) => value.to_string(),
        Err(_) => panic!("MONGODB_URI must be set"),
    };

    let ssh_origin = match env::var("SSH_ORIGIN") {
        Ok(value) => value.to_string(),
        Err(_) => panic!("SSH_ORIGIN must be set"),
    };

    let ssh_password = match env::var("SSH_PASSWORD") {
        Ok(value) => value.to_string(),
        Err(_) => panic!("SSH_PASSWORD must be set"),
    };

    let client: WebhookClient = WebhookClient::new(url.as_str());

    /****************
     * Remove all the old mongo dump files
     ****************/
    send_webhook_message(
        &client,
        "Cron job started, Removing all the old dump files.",
    )
    .await;
    println!("{}: Cron job started.", get_time_str());
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("rm -rf /home/backup/{}", database_name))
        .output()
        .expect("failed to execute process");
    let files_cmd = command_success(
        &output,
        format!("{}: Removing all the old dump files.", get_time_str()).as_str(),
    );
    if !files_cmd {
        send_webhook_message(&client, "Error removing all the old dump files.").await;
        println!("{}: Error removing all the old dump files.", get_time_str());
    } else {
        send_webhook_message(&client, "All the old dump files removed.").await;
        println!("{}: All the old dump files removed.", get_time_str());
    }

    /****************
     * Dump the mongo database
     ****************/
    send_webhook_message(&client, "Dumping the mongo database.").await;
    let output = Command::new("/usr/bin/mongodump")
        .arg(format!("--uri={}", mongodb_uri))
        .arg(format!("-d={}", database_name))
        .arg("-o=/home/backup/")
        .arg("--gzip")
        .arg("--numParallelCollections=10")
        .output()
        .expect("failed to execute process");

    let dump_cmd = command_success(
        &output,
        format!("{}: Dumping the mongo database.", get_time_str()).as_str(),
    );

    if !dump_cmd {
        send_webhook_message(&client, "Error dumping the mongo database.").await;
        println!("{}: Error dumping the mongo database.", get_time_str());
        panic!("Error dumping the mongo database.");
    } else {
        send_webhook_message(&client, "Mongo database dumped.").await;
        println!("{}: Mongo database dumped.", get_time_str());
    }

    /****************
     * Remove the _id_ index from the dump files
     ****************/
    send_webhook_message(&client, "Removing the _id_ index from the dump files.").await;
    println!(
        "{}: Removing the _id_ index from the dump files.",
        get_time_str()
    );
    remove_id_index(database_name).await.unwrap();

    /****************
     * Calculate the size of the directory
     ****************/
    let dir_size = dir_size(format!("/home/backup/{}", database_name).as_str())
        .await
        .unwrap();

    send_webhook_message(
        &client,
        format!("Native MONGODB compressed (gz) size: {}", dir_size).as_str(),
    )
    .await;

    /****************
     * delete the old file from the storage box
     ****************/
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "sshpass -p '{}' ssh '-p23' {} 'ls ./{}'",
            ssh_password, ssh_origin, destination_dir,
        ))
        .output()
        .expect("failed to execute process");

    // create a dir in the storage box
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "sshpass -p '{}' ssh '-p23' {} 'mkdir ./{}{}'",
            ssh_password, ssh_origin, destination_dir, filename
        ))
        .output()
        .expect("failed to execute process");

    let ls_cmd = command_success(
        &output,
        format!(
            "{}: Finding the old file from the storage box.",
            get_time_str()
        )
        .as_str(),
    );

    if !ls_cmd {
        send_webhook_message(&client, "Error finding the old file from the storage box.").await;
        println!(
            "{}: Error finding the old file from the storage box.",
            get_time_str()
        );
        panic!("Error finding the old file from the storage box.");
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let old_file = find_oldest_file(stdout.as_ref());
        if !old_file.is_empty() {
            println!("Old file: {}", old_file);
            send_webhook_message(
                &client,
                format!("Deleting oldest file `{}`", old_file).as_str(),
            )
            .await;

            let output = Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "sshpass -p '{}' ssh -p 23 {} 'rm -rf ./{}{}'",
                    ssh_password, ssh_origin, destination_dir, old_file
                ))
                .output()
                .expect("failed to execute process");

            let rm_cmd = command_success(
                &output,
                format!(
                    "{}: Deleting the old file from the storage box.",
                    get_time_str()
                )
                .as_str(),
            );
            if !rm_cmd {
                send_webhook_message(&client, "Error deleting the old file from the storage box.")
                    .await;
                println!(
                    "{}: Error deleting the old file from the storage box.",
                    get_time_str()
                );
                panic!("Error deleting the old file from the storage box.");
            } else {
                send_webhook_message(&client, "Old file deleted.").await;
                println!("{}: Old file deleted.", get_time_str());
            }
        }
    }

    /****************
     * Delete all the zips from the server
     ****************/
    send_webhook_message(&client, "Deleting the all the zips from the server.").await;
    println!(
        "{}: Deleting the all the zips from the server.",
        get_time_str()
    );

    let output = Command::new("sh")
        .arg("-c")
        .arg("rm -r /home/backup/zips/*")
        .output()
        .expect("failed to execute process");

    let cp_cmd = command_success(
        &output,
        format!("{}: Deleted all the old zips.", get_time_str()).as_str(),
    );

    if !cp_cmd {
        send_webhook_message(&client, "Error deleting all the old zips.").await;
        println!("{}: Error deleting all the old zips.", get_time_str());
        panic!("Error deleting all the old zips.");
    } else {
        send_webhook_message(&client, "All the are deleted").await;
        println!("{}: All the are deleted", get_time_str());
    }

    /****************
     * Copy all the files and create a dir inside of the zips folder
     ****************/
    send_webhook_message(
        &client,
        "Moving all the files and create a dir inside of the zips folder.",
    )
    .await;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "mv -r /home/backup/{} /home/backup/zips/{}",
            database_name, filename
        ))
        .output()
        .expect("failed to execute process");

    let cp_cmd = command_success(
        &output,
        format!(
            "{}: Moving all the files and create a dir inside of the zips folder.",
            get_time_str()
        )
        .as_str(),
    );

    if !cp_cmd {
        send_webhook_message(
            &client,
            "Error moving all the files and create a dir inside of the zips folder.",
        )
        .await;
        println!(
            "{}: Error moving all the files and create a dir inside of the zips folder.",
            get_time_str()
        );
        panic!("Error moving all the files and create a dir inside of the zips folder.");
    } else {
        send_webhook_message(
            &client,
            "All the files moved and a dir created inside of the zips folder.",
        )
        .await;
        println!(
            "{}: All the files moved and a dir created inside of the zips folder.",
            get_time_str()
        );
    }

    /****************
     * Copy the zip file to the storage box
     ****************/
    send_webhook_message(&client, "Copying the zip file to the storage box.").await;
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "sshpass -p '{}' scp '-p23' /home/backup/zips/{}/* {}:{}{}",
            ssh_password, filename, ssh_origin, destination_dir, filename
        ))
        .output()
        .expect("failed to execute process");

    let scp_cmd = command_success(
        &output,
        format!(
            "{}: Copying the zip file to the storage box.",
            get_time_str()
        )
        .as_str(),
    );

    if !scp_cmd {
        send_webhook_message(&client, "Error copying the zip file to the storage box.").await;
        println!(
            "{}: Error copying the zip file to the storage box.",
            get_time_str()
        );
        panic!("Error copying the zip file to the storage box.");
    } else {
        send_webhook_message(&client, "Zip file copied.").await;
        println!("{}: Zip file copied.", get_time_str());
    }

    /****************
     * Cron job finished
     ****************/

    send_webhook_message(
        &client,
        format!("Cron job finished. Time Taken: {}", elapsed_time(instant)).as_str(),
    )
    .await;
    println!("{}: Cron job finished.", get_time_str());

    println!("Time elapsed: {}", elapsed_time(instant));
    std::process::exit(0);
}
