use notebooklm_api::client::NotebookLmClient;
use notebooklm_api::{AudioGenerationOptions, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let client = NotebookLmClient::from_storage(None).await?;

    let notebooks = client.notebooks().list().await?;
    println!("notebooks: {}", notebooks.len());

    if let Some(nb) = notebooks.first() {
        let status = client
            .artifacts()
            .generate_audio(&nb.id, AudioGenerationOptions::default())
            .await?;
        println!("generation task: {} ({})", status.task_id, status.status);
    }

    Ok(())
}
