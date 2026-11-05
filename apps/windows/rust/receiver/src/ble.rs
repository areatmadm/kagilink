//! BLE central, Windows only (WinRT through the official `windows` crate, no `unsafe`).
//!
//! This implements the PHASE 0 DIAGNOSTIC contract from docs/PHASE0_BLE_PROBE.md in Rust: scan up
//! to N seconds for the fixed diagnostic service, require exactly one sender, connect, write the
//! fixed 16-byte request with response, read the fixed 16-byte reply and compare it. It carries no
//! secret and no production protocol; mapping the core's M1..M4 and fragments onto GATT is Phase 2
//! work that is not designed here. Device addresses and names are never printed or stored.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::core::GUID;
use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
    BluetoothLEScanningMode,
};
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCommunicationStatus, GattWriteOption,
};
use windows::Devices::Bluetooth::{BluetoothAdapter, BluetoothCacheMode, BluetoothLEDevice};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::{DataReader, DataWriter};

/// Fixed diagnostic service and characteristic (Phase 0 only; not the production service).
const SERVICE: GUID = GUID::from_u128(0x7d9a0001_6f8e_4d52_9b91_89c767b6e201);
const CHARACTERISTIC: GUID = GUID::from_u128(0x7d9a0002_6f8e_4d52_9b91_89c767b6e201);
const PING: &[u8; 16] = b"KagiLink-P0-PING";
const PONG: &[u8; 16] = b"KagiLink-P0-PONG";

/// Production draft service (docs/BLE_GATT_SPEC.md, D025). Advertised by the sender.
pub(crate) const KL_V1_SERVICE: GUID = GUID::from_u128(0x3685355e_2903_4e85_bdd6_474ada0838c1);
/// Production draft RX characteristic (receiver writes).
pub(crate) const KL_V1_RX: GUID = GUID::from_u128(0x316da2a3_cfc7_4bbf_a736_58c4e7d8d5cf);
/// Production draft TX characteristic (sender notifies/indicates).
pub(crate) const KL_V1_TX: GUID = GUID::from_u128(0x7c69e9b3_c395_4fc0_87b2_b564d7b8b29e);

/// BLE AD type "Service Data - 128-bit UUID".
const AD_SERVICE_DATA_128: u8 = 0x21;

/// Why an advertisement scan could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanError {
    /// No Bluetooth adapter is available to this process.
    NoAdapter,
    /// The adapter does not support the LE central role.
    CentralUnsupported,
    /// A WinRT step failed.
    Failed(String),
}

/// Parse the payload of an AD type 0x21 section (the bytes after the type): a 16-byte UUID in
/// little-endian order followed by the data. Returns the data only if the UUID equals
/// `KL_V1_SERVICE` AND at least one data byte is present (an empty payload gives `None`).
fn parse_service_data_128(section_bytes: &[u8]) -> Option<Vec<u8>> {
    if section_bytes.len() <= 16 {
        return None;
    }
    let (uuid, data) = section_bytes.split_at(16);
    if uuid == KL_V1_SERVICE.to_u128().to_le_bytes() {
        Some(data.to_vec())
    } else {
        None
    }
}

pub(crate) fn read_buffer(buf: &windows::Storage::Streams::IBuffer) -> Option<Vec<u8>> {
    let reader = DataReader::FromBuffer(buf).ok()?;
    let len = reader.UnconsumedBufferLength().ok()? as usize;
    let mut bytes = vec![0u8; len];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(bytes)
}

/// Actively scan for `scan_seconds` and return one observation per received packet that lists or
/// carries data for the KL-V1 service. Addresses are used only as an opaque grouping key and are
/// never printed or stored; device names are only passed on in memory.
pub fn scan_advertisements(
    scan_seconds: u64,
) -> Result<Vec<crate::selection::RawAdvertisement>, ScanError> {
    use crate::selection::RawAdvertisement;
    let adapter = BluetoothAdapter::GetDefaultAsync()
        .and_then(|op| op.join())
        .map_err(|_| ScanError::NoAdapter)?;
    if !adapter.IsCentralRoleSupported().unwrap_or(false) {
        return Err(ScanError::CentralUnsupported);
    }
    let fail = |step: &str, e: windows::core::Error| ScanError::Failed(format!("{step}: {e}"));
    let watcher = BluetoothLEAdvertisementWatcher::new().map_err(|e| fail("watcher", e))?;
    // Active scanning is required so scan responses (which may carry the tag) arrive.
    watcher
        .SetScanningMode(BluetoothLEScanningMode::Active)
        .map_err(|e| fail("scanning mode", e))?;
    let found: Arc<Mutex<Vec<RawAdvertisement>>> = Arc::new(Mutex::new(Vec::new()));
    let found_cb = Arc::clone(&found);
    let handler = TypedEventHandler::<
        BluetoothLEAdvertisementWatcher,
        BluetoothLEAdvertisementReceivedEventArgs,
    >::new(move |_, args| {
        let Some(args) = args.as_ref() else {
            return Ok(());
        };
        let Ok(adv) = args.Advertisement() else {
            return Ok(());
        };
        let listed = adv
            .ServiceUuids()
            .map(|u| u.into_iter().any(|g| g == KL_V1_SERVICE))
            .unwrap_or(false);
        let mut service_data = None;
        if let Ok(sections) = adv.DataSections() {
            for section in sections {
                if section.DataType().ok() != Some(AD_SERVICE_DATA_128) {
                    continue;
                }
                if let Some(bytes) = section.Data().ok().and_then(|b| read_buffer(&b)) {
                    if let Some(d) = parse_service_data_128(&bytes) {
                        service_data = Some(d);
                        break;
                    }
                }
            }
        }
        if !listed && service_data.is_none() {
            return Ok(());
        }
        let local_name = adv
            .LocalName()
            .ok()
            .map(|h| h.to_string())
            .filter(|n| !n.is_empty());
        let raw = RawAdvertisement {
            local_name,
            service_data,
            rssi_dbm: args.RawSignalStrengthInDBm().ok(),
            source: args.BluetoothAddress().unwrap_or(0),
        };
        if let Ok(mut v) = found_cb.lock() {
            v.push(raw);
        }
        Ok(())
    });
    let token = watcher
        .Received(&handler)
        .map_err(|e| fail("watcher handler", e))?;
    if let Err(e) = watcher.Start() {
        let _ = watcher.RemoveReceived(token);
        return Err(fail("scan start", e));
    }
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(scan_seconds) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = watcher.Stop();
    let _ = watcher.RemoveReceived(token);
    let out = found.lock().map(|v| v.clone()).unwrap_or_default();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(uuid: GUID, data: &[u8]) -> Vec<u8> {
        let mut v = uuid.to_u128().to_le_bytes().to_vec();
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn matching_uuid_returns_data() {
        let p = payload(KL_V1_SERVICE, &[0x4B, 0x4C, 0x32]);
        assert_eq!(parse_service_data_128(&p), Some(vec![0x4B, 0x4C, 0x32]));
    }

    #[test]
    fn wrong_uuid_is_rejected() {
        let p = payload(GUID::from_u128(0x1234), &[1, 2, 3]);
        assert_eq!(parse_service_data_128(&p), None);
    }

    #[test]
    fn big_endian_uuid_order_is_rejected() {
        let mut p = KL_V1_SERVICE.to_u128().to_be_bytes().to_vec();
        p.push(9);
        assert_eq!(parse_service_data_128(&p), None);
    }

    #[test]
    fn too_short_is_rejected() {
        assert_eq!(parse_service_data_128(&[]), None);
        assert_eq!(parse_service_data_128(&[0u8; 15]), None);
    }

    #[test]
    fn exactly_uuid_without_data_is_none() {
        let p = payload(KL_V1_SERVICE, &[]);
        assert_eq!(p.len(), 16);
        assert_eq!(parse_service_data_128(&p), None);
    }
}

/// What the diagnostic found. Nothing here identifies a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BleOutcome {
    /// No Bluetooth adapter is available to this process.
    NoAdapter,
    /// The adapter exists but does not support the LE central role.
    CentralUnsupported,
    /// No sender advertising the diagnostic service was seen.
    NoSender,
    /// More than one sender was seen; the diagnostic refuses to pick one.
    MultipleSenders(usize),
    /// Fixed request written and the exact fixed reply read back.
    ExchangeOk,
    /// A step failed.
    ExchangeFailed(String),
}

fn fail<T: std::fmt::Display>(step: &str, e: T) -> BleOutcome {
    BleOutcome::ExchangeFailed(format!("{step}: {e}"))
}

/// Scan for `scan_seconds`, then run the fixed exchange with the single sender found.
pub fn ping_diagnostic(scan_seconds: u64) -> BleOutcome {
    let adapter = match BluetoothAdapter::GetDefaultAsync().and_then(|op| op.join()) {
        Ok(a) => a,
        Err(_) => return BleOutcome::NoAdapter,
    };
    if !adapter.IsCentralRoleSupported().unwrap_or(false) {
        return BleOutcome::CentralUnsupported;
    }

    let watcher = match BluetoothLEAdvertisementWatcher::new() {
        Ok(w) => w,
        Err(e) => return fail("watcher", e),
    };
    let _ = watcher.SetScanningMode(BluetoothLEScanningMode::Active);
    let seen: Arc<Mutex<HashSet<u64>>> = Arc::new(Mutex::new(HashSet::new()));
    let seen_cb = Arc::clone(&seen);
    let handler = TypedEventHandler::<
        BluetoothLEAdvertisementWatcher,
        BluetoothLEAdvertisementReceivedEventArgs,
    >::new(move |_, args| {
        if let Some(args) = args.as_ref() {
            if let Ok(uuids) = args.Advertisement().and_then(|a| a.ServiceUuids()) {
                if uuids.into_iter().any(|u| u == SERVICE) {
                    if let Ok(addr) = args.BluetoothAddress() {
                        if let Ok(mut set) = seen_cb.lock() {
                            set.insert(addr);
                        }
                    }
                }
            }
        }
        Ok(())
    });
    let token = match watcher.Received(&handler) {
        Ok(t) => t,
        Err(e) => return fail("watcher handler", e),
    };
    if let Err(e) = watcher.Start() {
        return fail("scan start", e);
    }
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(scan_seconds) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = watcher.Stop();
    let _ = watcher.RemoveReceived(token);

    let addresses: Vec<u64> = seen
        .lock()
        .map(|s| s.iter().copied().collect())
        .unwrap_or_default();
    match addresses.len() {
        0 => return BleOutcome::NoSender,
        1 => {}
        n => return BleOutcome::MultipleSenders(n),
    }
    exchange(addresses[0])
}

fn exchange(address: u64) -> BleOutcome {
    let device = match BluetoothLEDevice::FromBluetoothAddressAsync(address).and_then(|o| o.join())
    {
        Ok(d) => d,
        Err(e) => return fail("connect", e),
    };
    let services = match device
        .GetGattServicesForUuidAsync(SERVICE)
        .and_then(|o| o.join())
    {
        Ok(s) => s,
        Err(e) => return fail("service discovery", e),
    };
    if services.Status().ok() != Some(GattCommunicationStatus::Success) {
        return BleOutcome::ExchangeFailed("service discovery status".to_string());
    }
    let service = match services.Services().ok().and_then(|v| v.into_iter().next()) {
        Some(s) => s,
        None => return BleOutcome::ExchangeFailed("service missing".to_string()),
    };
    let chars = match service
        .GetCharacteristicsForUuidAsync(CHARACTERISTIC)
        .and_then(|o| o.join())
    {
        Ok(c) => c,
        Err(e) => return fail("characteristic discovery", e),
    };
    let characteristic = match chars
        .Characteristics()
        .ok()
        .and_then(|v| v.into_iter().next())
    {
        Some(c) => c,
        None => return BleOutcome::ExchangeFailed("characteristic missing".to_string()),
    };

    let writer = match DataWriter::new() {
        Ok(w) => w,
        Err(e) => return fail("buffer", e),
    };
    if writer.WriteBytes(PING).is_err() {
        return BleOutcome::ExchangeFailed("buffer write".to_string());
    }
    let buffer = match writer.DetachBuffer() {
        Ok(b) => b,
        Err(e) => return fail("buffer", e),
    };
    let wrote = characteristic
        .WriteValueWithResultAndOptionAsync(&buffer, GattWriteOption::WriteWithResponse)
        .and_then(|o| o.join());
    match wrote.and_then(|r| r.Status()) {
        Ok(GattCommunicationStatus::Success) => {}
        Ok(_) => return BleOutcome::ExchangeFailed("write was not acknowledged".to_string()),
        Err(e) => return fail("write", e),
    }

    let read = match characteristic
        .ReadValueWithCacheModeAsync(BluetoothCacheMode::Uncached)
        .and_then(|o| o.join())
    {
        Ok(r) => r,
        Err(e) => return fail("read", e),
    };
    if read.Status().ok() != Some(GattCommunicationStatus::Success) {
        return BleOutcome::ExchangeFailed("read status".to_string());
    }
    let value = match read.Value().and_then(|b| DataReader::FromBuffer(&b)) {
        Ok(r) => r,
        Err(e) => return fail("read buffer", e),
    };
    let mut bytes = [0u8; 16];
    if value.UnconsumedBufferLength().unwrap_or(0) != 16 || value.ReadBytes(&mut bytes).is_err() {
        return BleOutcome::ExchangeFailed("reply length".to_string());
    }
    if &bytes == PONG {
        BleOutcome::ExchangeOk
    } else {
        BleOutcome::ExchangeFailed("reply bytes differ".to_string())
    }
}
