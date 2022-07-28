use std::str::FromStr;
use std::sync::Arc;
use std::time::SystemTime;

use btleplug::api::{Central, CentralEvent, Manager, Peripheral, ScanFilter};
use btleplug::platform::{self, Adapter, PeripheralId};
use futures::{Stream, StreamExt};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Record {
    pub name: String,
    pub temp: f32,
    pub humi: f32,
    pub batt: f32,
    pub date: SystemTime,
}

pub struct Scanner {
    central: Adapter,
}

fn parse_atc_data(name: String, data: &[u8]) -> Option<Record> {
    let (_mac, data) = data.split_at(6);
    let (temp, data) = data.split_at(2);
    let (humi, data) = data.split_at(2);
    let (_vbat, data) = data.split_at(2);
    let (bat, data) = data.split_at(1);
    let (_cnt, data) = data.split_at(1);
    let (_flag, _data) = data.split_at(1);
    // SAFETY: cannot fail
    let temp = i16::from_le_bytes(temp.try_into().unwrap()) as f32 / 100.0;
    // SAFETY: cannot fail
    let humi = i16::from_le_bytes(humi.try_into().unwrap()) as f32 / 100.0;
    // let vbat = u16::from_le_bytes(vbat.try_into()?);
    let batt = bat[0] as f32;
    let date = SystemTime::now();
    Some(Record {
        name,
        temp,
        humi,
        batt,
        date,
    })
}

fn parse_govee_data(name: String, data: &[u8]) -> Option<Record> {
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
        let batt = bat[0] as f32;

        let date = SystemTime::now();

        Some(Record {
            name,
            temp,
            humi,
            batt,
            date,
        })
    } else {
        None
    }
}

async fn get_name(central: &Adapter, id: &PeripheralId) -> Option<String> {
    if let Ok(per) = central.peripheral(&id).await {
        if let Ok(Some(props)) = per.properties().await {
            props.local_name
        } else {
            None
        }
    } else {
        None
    }
}

impl Scanner {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = platform::Manager::new().await?;
        let adapters = manager.adapters().await.unwrap();
        let central = adapters.into_iter().nth(0).unwrap();

        Ok(Scanner { central })
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.central.start_scan(ScanFilter::default()).await?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.central.stop_scan().await?;

        Ok(())
    }

    pub async fn stream(&self) -> Result<impl Stream<Item = Record>, Box<dyn std::error::Error>> {
        let central = Arc::new(self.central.clone());
        let events = self.central.events().await?;

        let records = events.filter_map(move |ev| {
            let central = central.clone();
            async move {
                match ev {
                    CentralEvent::ManufacturerDataAdvertisement {
                        id,
                        manufacturer_data,
                    } => {
                        let name = get_name(&central, &id).await;
                        if let Some(local_name) = name.filter(|v| v.starts_with("GVH")) {
                            let idx = 60552;
                            if let Some(data) = manufacturer_data.get(&idx) {
                                parse_govee_data(local_name, &data)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    CentralEvent::ServiceDataAdvertisement { id, service_data } => {
                        let name = get_name(&central, &id).await;
                        if let Some(local_name) = name.filter(|v| v.starts_with("ATC_")) {
                            // SAFETY: cannot fail.
                            let uuid =
                                Uuid::from_str("0000181a-0000-1000-8000-00805f9b34fb").unwrap();
                            if let Some(data) = service_data.get(&uuid).filter(|v| v.len() >= 15) {
                                parse_atc_data(local_name, &data)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
        });

        Ok(records)
    }
}
