// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use btleplug::api::Peripheral;
use btleplug::api::{Central, CentralEvent, Manager as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use futures::stream::StreamExt;
use std::convert::TryInto;
use std::error::Error;
use std::str::FromStr;
use uuid::Uuid;

async fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().await.unwrap();
    adapters.into_iter().nth(0).unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;

    // get the first bluetooth adapter
    // connect to the adapter
    let central = get_central(&manager).await;

    // Each adapter has an event stream, we fetch via events(),
    // simplifying the type, this will return what is essentially a
    // Future<Result<Stream<Item=CentralEvent>>>.
    let mut events = central.events().await?;

    // start scanning for devices
    central.start_scan(ScanFilter::default()).await?;

    // Print based on whatever the event receiver outputs. Note that the event
    // receiver blocks, so in a real program, this should be run in its own
    // thread (not task, as this library does not yet use async channels).
    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(_id) => {
                /*
                let per = central.peripheral(&id).await?;

                if let Some(local_name) = per.properties().await?.unwrap().local_name {
                    println!("discovered {:?}", local_name);
                }
                */
            }
            CentralEvent::DeviceConnected(_id) => {
                // println!("DeviceConnected: {:?}", id);
            }
            CentralEvent::DeviceDisconnected(_id) => {
                // println!("DeviceDisconnected: {:?}", id);
            }
            CentralEvent::ManufacturerDataAdvertisement {
                id,
                manufacturer_data,
            } => {
                let per = central.peripheral(&id).await?;
                let id = per.properties().await?.unwrap().local_name;
                if let Some(local_name) = id.filter(|v| v.starts_with("GVH")) {
                    let idx = 60552;
                    if let Some(data) = manufacturer_data.get(&idx) {
                        let len = data.len();
                        let pat = b"INTELLI_ROCKS";
                        let data = if len > 25 {
                            let len = data
                                .windows(pat.len())
                                .position(|w| w == pat)
                                .unwrap_or(len);
                            &data[..len]
                        } else {
                            &data[..]
                        };

                        if data.len() == 10 {
                            let (temp_humi, bat) = data.split_at(4);
                            let temp_humi = i32::from_be_bytes(temp_humi.try_into().unwrap());

                            let temp = if temp_humi & 0x800000 != 0 {
                                (temp_humi ^ 0x800000) / 1000
                            } else {
                                temp_humi / 1000
                            } as f32
                                / 10.0;

                            let humi = (temp_humi % 1000) as f32 / 10.0;
                            let bat = bat[0];

                            println!(
                                "{:?}, temp {:.2}C humid {:.2}% {}%",
                                local_name, temp, humi, bat
                            );
                        }
                    }
                }
            }
            CentralEvent::ServiceDataAdvertisement { id, service_data } => {
                let per = central.peripheral(&id).await?;
                let id = per.properties().await?.unwrap().local_name;

                if let Some(local_name) = id.as_ref().filter(|id| id.starts_with("ATC_")) {
                    let uuid = Uuid::from_str("0000181a-0000-1000-8000-00805f9b34fb")?;
                    if let Some(data) = service_data.get(&uuid).filter(|v| v.len() >= 15) {
                        let (_mac, data) = data.split_at(6);
                        let (temp, data) = data.split_at(2);
                        let (humi, data) = data.split_at(2);
                        let (vbat, data) = data.split_at(2);
                        let (bat, data) = data.split_at(1);
                        let (cnt, data) = data.split_at(1);
                        let (flag, _data) = data.split_at(1);

                        let temp = i16::from_le_bytes(temp.try_into()?) as f32 / 100.0;
                        let humi = i16::from_le_bytes(humi.try_into()?) as f32 / 100.0;
                        let vbat = u16::from_le_bytes(vbat.try_into()?);

                        println!(
                            "{:?}, temp {:.2}C humid {:.2}% V{} {}% cnt {} flag {}",
                            local_name, temp, humi, vbat, bat[0], cnt[0], flag[0]
                        );
                    }
                } /*  else {
                      println!("{:?}, {:?}", id, service_data);
                  } */
            }
            CentralEvent::ServicesAdvertisement { id: _, services: _ } => {
                /* let per = central.peripheral(&id).await?;

                let id = per.properties().await?.unwrap().local_name;

                                if let Some(local_name) = id.filter(|v| v.starts_with("GVH")) {
                                    let services: Vec<String> =
                                        services.into_iter().map(|s| s.to_short_string()).collect();
                                    println!("ServicesAdvertisement: {:?}, {:?}", local_name, services);
                                }
                */
            }
            _ => {}
        }
    }
    Ok(())
}
