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
