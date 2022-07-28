// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use ble_thermometers::Scanner;
use futures::future;
use futures::stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::new().await?;

    scanner.start().await?;

    let stream = scanner.stream().await?;

    stream
        .for_each(|r| {
            println!("{:?}", r);
            future::ready(())
        })
        .await;

    Ok(())
}
