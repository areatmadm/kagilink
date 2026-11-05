//! Real Windows GATT link for the KL-V1 draft service (docs/BLE_GATT_SPEC.md section 2).
//!
//! UNVERIFIED against a real KL-V1 peripheral: it compiles and the driver logic above it is
//! covered by the fake-phone tests through the `GattLink` trait, but no phone advertises the
//! production service yet. Addresses are passed in as opaque numbers and never printed or stored.

use crate::ble::{read_buffer, KL_V1_RX, KL_V1_SERVICE, KL_V1_TX};
use crate::session::{GattLink, LinkError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::core::GUID;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattCharacteristicProperties,
    GattClientCharacteristicConfigurationDescriptorValue, GattCommunicationStatus, GattSession,
    GattValueChangedEventArgs, GattWriteOption,
};
use windows::Devices::Bluetooth::{
    BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::DataWriter;

/// Why the connect step failed (no device identifiers inside).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// A WinRT step failed or returned a bad status; the text names the step only.
    Step(&'static str),
    /// The peer does not offer the KL-V1 service or one of its characteristics.
    NotKagiLink,
    /// TX offers neither Indicate nor Notify.
    NoSubscribeMode,
}

/// A connected, subscribed KL-V1 link.
pub struct WinGattLink {
    device: BluetoothLEDevice,
    _session: GattSession,
    rx: GattCharacteristic,
    tx: GattCharacteristic,
    token: i64,
    incoming: Receiver<Vec<u8>>,
    dropped: Arc<AtomicBool>,
    chunk: usize,
    notify_only: bool,
    closed: bool,
}

fn step<T>(r: windows::core::Result<T>, name: &'static str) -> Result<T, ConnectError> {
    r.map_err(|_| ConnectError::Step(name))
}

fn find_characteristic(
    service: &windows::Devices::Bluetooth::GenericAttributeProfile::GattDeviceService,
    uuid: GUID,
) -> Result<GattCharacteristic, ConnectError> {
    let r = step(
        service
            .GetCharacteristicsForUuidWithCacheModeAsync(uuid, BluetoothCacheMode::Uncached)
            .and_then(|o| o.join()),
        "characteristic discovery",
    )?;
    if step(r.Status(), "characteristic status")? != GattCommunicationStatus::Success {
        return Err(ConnectError::NotKagiLink);
    }
    step(r.Characteristics(), "characteristics")?
        .into_iter()
        .next()
        .ok_or(ConnectError::NotKagiLink)
}

/// Connect to the advertiser the user chose, discover KL-V1 and enable Indicate (preferred) or
/// Notify on TX before anything is written.
pub fn connect(address: u64) -> Result<WinGattLink, ConnectError> {
    let device = step(
        BluetoothLEDevice::FromBluetoothAddressAsync(address).and_then(|o| o.join()),
        "connect",
    )?;
    let session = step(
        device
            .BluetoothDeviceId()
            .and_then(|id| GattSession::FromDeviceIdAsync(&id))
            .and_then(|o| o.join()),
        "gatt session",
    )?;
    let _ = session.SetMaintainConnection(true);
    let pdu = session.MaxPduSize().unwrap_or(23) as usize;
    let chunk = pdu.saturating_sub(3).clamp(20, 244);

    let services = step(
        device
            .GetGattServicesForUuidWithCacheModeAsync(KL_V1_SERVICE, BluetoothCacheMode::Uncached)
            .and_then(|o| o.join()),
        "service discovery",
    )?;
    if step(services.Status(), "service status")? != GattCommunicationStatus::Success {
        return Err(ConnectError::NotKagiLink);
    }
    let service = step(services.Services(), "services")?
        .into_iter()
        .next()
        .ok_or(ConnectError::NotKagiLink)?;
    let rx = find_characteristic(&service, KL_V1_RX)?;
    let tx = find_characteristic(&service, KL_V1_TX)?;

    let props = step(tx.CharacteristicProperties(), "tx properties")?;
    let (mode, notify_only) = if props.contains(GattCharacteristicProperties::Indicate) {
        (
            GattClientCharacteristicConfigurationDescriptorValue::Indicate,
            false,
        )
    } else if props.contains(GattCharacteristicProperties::Notify) {
        (
            GattClientCharacteristicConfigurationDescriptorValue::Notify,
            true,
        )
    } else {
        return Err(ConnectError::NoSubscribeMode);
    };

    let (sender, incoming) = channel::<Vec<u8>>();
    let handler =
        TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(move |_, args| {
            if let Some(a) = args.as_ref() {
                if let Some(bytes) = a.CharacteristicValue().ok().and_then(|b| read_buffer(&b)) {
                    let _ = sender.send(bytes);
                }
            }
            Ok(())
        });
    let token = step(tx.ValueChanged(&handler), "subscribe handler")?;

    let dropped = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&dropped);
    let status_handler =
        TypedEventHandler::<BluetoothLEDevice, windows::core::IInspectable>::new(move |d, _| {
            if let Some(d) = d.as_ref() {
                if d.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Disconnected) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
            Ok(())
        });
    let _ = device.ConnectionStatusChanged(&status_handler);

    let written = step(
        tx.WriteClientCharacteristicConfigurationDescriptorWithResultAsync(mode)
            .and_then(|o| o.join()),
        "enable indications",
    )?;
    if step(written.Status(), "cccd status")? != GattCommunicationStatus::Success {
        let _ = tx.RemoveValueChanged(token);
        return Err(ConnectError::Step("cccd write refused"));
    }

    Ok(WinGattLink {
        device,
        _session: session,
        rx,
        tx,
        token,
        incoming,
        dropped,
        chunk,
        notify_only,
        closed: false,
    })
}

impl GattLink for WinGattLink {
    fn write_rx(&mut self, chunk: &[u8]) -> Result<(), LinkError> {
        if self.closed || self.dropped.load(Ordering::SeqCst) {
            return Err(LinkError::Disconnected);
        }
        let writer = DataWriter::new().map_err(|_| LinkError::Disconnected)?;
        writer
            .WriteBytes(chunk)
            .map_err(|_| LinkError::Disconnected)?;
        let buffer = writer.DetachBuffer().map_err(|_| LinkError::Disconnected)?;
        let r = self
            .rx
            .WriteValueWithResultAndOptionAsync(&buffer, GattWriteOption::WriteWithResponse)
            .and_then(|o| o.join())
            .and_then(|r| r.Status());
        match r {
            Ok(GattCommunicationStatus::Success) => Ok(()),
            _ => Err(LinkError::Disconnected),
        }
    }

    fn next_tx(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, LinkError> {
        let end = Instant::now() + timeout;
        loop {
            if self.closed {
                return Err(LinkError::Disconnected);
            }
            let now = Instant::now();
            let slice = if now >= end {
                Duration::ZERO
            } else {
                (end - now).min(Duration::from_millis(200))
            };
            // Anything already received is delivered before a drop is reported.
            match self.incoming.recv_timeout(slice) {
                Ok(bytes) => return Ok(Some(bytes)),
                Err(RecvTimeoutError::Disconnected) => return Err(LinkError::Disconnected),
                Err(RecvTimeoutError::Timeout) => {
                    if self.dropped.load(Ordering::SeqCst) {
                        return Err(LinkError::Disconnected);
                    }
                    if now >= end {
                        return Ok(None);
                    }
                }
            }
        }
    }

    fn chunk_size(&self) -> usize {
        self.chunk
    }

    fn tx_needs_ack(&self) -> bool {
        self.notify_only
    }

    fn disconnect(&mut self) {
        if !self.closed {
            self.closed = true;
            let _ = self.tx.RemoveValueChanged(self.token);
            // Release the GATT session first so Windows can drop the link instead of keeping it alive.
            let _ = self._session.SetMaintainConnection(false);
            let _ = self._session.Close();
            let _ = self.device.Close();
        }
    }
}

impl Drop for WinGattLink {
    fn drop(&mut self) {
        self.disconnect();
    }
}
