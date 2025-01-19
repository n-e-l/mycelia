use dotenv::dotenv;
use crate::database::Database;

mod database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize .env environment variables
    dotenv().ok();

    let db = Database::new();
    let a = db.get_concepts().await;
    println!("{:?}", a);

    Ok(())
}
