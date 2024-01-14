// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use ble_thermometers::Scanner;
use futures::future;
use futures::stream::{StreamExt, TryStreamExt};
use gluesql::core::ast_builder::{self, num, text, timestamp, Execute};
use gluesql::json_storage::JsonStorage;
use gluesql::prelude::Glue;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::new().await?;

    scanner.start().await?;

    let stream = scanner.stream().await?;

    let storage = JsonStorage::new("logs/")?;

    let mut glue = Glue::new(storage.clone());

    ast_builder::table("temps")
        .create_table_if_not_exists()
        .add_column("name TEXT")
        .add_column("ts TIMESTAMP")
        .add_column("temp FLOAT")
        .add_column("humi FLOAT")
        .execute(&mut glue)
        .await?;

    stream
        .then(|r| {
            println!("{:?}", r);
            let v = vec![
                text(r.name.clone()),
                timestamp(r.date.to_string()),
                num(r.temp),
                num(r.humi),
            ];
            let mut glue = Glue::new(storage.clone());
            async move {
                ast_builder::table("temps")
                    .insert()
                    .values(vec![v])
                    .execute(&mut glue)
                    .await
            }
        })
        .try_for_each(|_v| future::ready(Ok(())))
        .await?;

    Ok(())
}
