// Public, unauthenticated Phase 0 diagnostic only. Never a secret transport.
#pragma once
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Devices.Bluetooth.Advertisement.h>
#include <winrt/Windows.Devices.Bluetooth.GenericAttributeProfile.h>
#include <winrt/Windows.Storage.Streams.h>
#include <atomic>
#include <algorithm>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <vector>

namespace phase0_ble {
using namespace winrt::Windows::Devices::Bluetooth;
using namespace winrt::Windows::Devices::Bluetooth::Advertisement;
using namespace winrt::Windows::Devices::Bluetooth::GenericAttributeProfile;
using namespace winrt::Windows::Storage::Streams;
inline constexpr winrt::guid service_id{L"7d9a0001-6f8e-4d52-9b91-89c767b6e201"};
inline constexpr winrt::guid characteristic_id{L"7d9a0002-6f8e-4d52-9b91-89c767b6e201"};
inline constexpr std::string_view ping = "KagiLink-P0-PING";
inline constexpr std::string_view pong = "KagiLink-P0-PONG";

template<class T> struct close_on_exit {
    T value{nullptr};
    ~close_on_exit() { if (value) { try { value.Close(); } catch (...) {} } }
};

template<class T> auto bounded_result(T const& operation, ULONGLONG deadline) {
    using winrt::Windows::Foundation::AsyncStatus;
    while (operation.Status() == AsyncStatus::Started && GetTickCount64() < deadline) Sleep(20);
    if (GetTickCount64() >= deadline) {
        operation.Cancel();
        throw std::runtime_error("deadline");
    }
    return operation.GetResults();
}

struct candidate { uint64_t address; BluetoothAddressType type; };
struct scan_state {
    std::mutex mutex;
    std::vector<candidate> candidates;
    std::atomic<bool> failed{false};
    bool collecting = true;
};
struct scan_scope {
    BluetoothLEAdvertisementWatcher watcher;
    winrt::event_token received{}, stopped{};
    ~scan_scope() {
        try { watcher.Stop(); } catch (...) {}
        try { watcher.Received(received); watcher.Stopped(stopped); } catch (...) {}
    }
};

inline int run() {
    std::cout << "PHASE0_PUBLIC_UNAUTHENTICATED_BLE_TEST\n"
                 "NO_PEER_IDENTITY_OR_HARDWARE_VERIFIED\n"
                 "REAL_SECRET_TRANSFER_DISABLED\nRDP_INPUT_UNSUPPORTED\n";
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        const auto deadline = GetTickCount64() + 60000;
        auto adapter = bounded_result(BluetoothAdapter::GetDefaultAsync(), deadline);
        if (!adapter || !adapter.IsLowEnergySupported() || !adapter.IsCentralRoleSupported()) {
            std::cout << "BLE_CENTRAL_UNAVAILABLE\n"; return 2;
        }
        const auto state = std::make_shared<scan_state>();
        scan_scope scan;
        scan.watcher.ScanningMode(BluetoothLEScanningMode::Active);
        scan.watcher.AdvertisementFilter().Advertisement().ServiceUuids().Append(service_id);
        scan.received = scan.watcher.Received([state](auto const&, auto const& event) {
            // OS addresses are transient routing data only: never printed or saved.
            std::scoped_lock lock(state->mutex);
            if (!state->collecting) return;
            for (const auto& found : state->candidates)
                if (found.address == event.BluetoothAddress() && found.type == event.BluetoothAddressType()) return;
            if (state->candidates.size() < 2)
                state->candidates.push_back({event.BluetoothAddress(), event.BluetoothAddressType()});
        });
        scan.stopped = scan.watcher.Stopped([state](auto const&, auto const& event) {
            if (event.Error() != BluetoothError::Success) state->failed = true;
        });
        scan.watcher.Start();
        std::cout << "SCANNING_DIAGNOSTIC_SERVICE_10_SECONDS\n";
        const auto scan_end = (std::min)(deadline, GetTickCount64() + 10000);
        while (GetTickCount64() < scan_end && !state->failed) Sleep(20);
        scan.watcher.Stop();
        if (GetTickCount64() >= deadline) {
            std::cout << "BLE_TEST_DEADLINE_EXPIRED\n"; return 2;
        }
        candidate peer{};
        {
            std::scoped_lock lock(state->mutex);
            state->collecting = false;
            if (state->failed) { std::cout << "SCAN_FAILED\n"; return 2; }
            if (state->candidates.empty()) { std::cout << "NO_DIAGNOSTIC_SENDER\n"; return 2; }
            if (state->candidates.size() != 1) { std::cout << "AMBIGUOUS_SENDERS_STOP_OTHER_PROBES\n"; return 2; }
            peer = state->candidates.front();
        }
        close_on_exit<BluetoothLEDevice> device{bounded_result(
            BluetoothLEDevice::FromBluetoothAddressAsync(peer.address, peer.type), deadline)};
        if (!device.value) { std::cout << "DEVICE_ACCESS_UNAVAILABLE\n"; return 2; }
        auto services = bounded_result(device.value.GetGattServicesForUuidAsync(service_id, BluetoothCacheMode::Uncached), deadline);
        if (services.Status() != GattCommunicationStatus::Success || services.Services().Size() != 1) {
            // Close every returned service even on ambiguous discovery.
            for (auto const& item : services.Services()) item.Close();
            std::cout << "DIAGNOSTIC_SERVICE_UNAVAILABLE\n"; return 2;
        }
        close_on_exit<GattDeviceService> service{services.Services().GetAt(0)};
        auto characteristics = bounded_result(service.value.GetCharacteristicsForUuidAsync(
            characteristic_id, BluetoothCacheMode::Uncached), deadline);
        if (characteristics.Status() != GattCommunicationStatus::Success || characteristics.Characteristics().Size() != 1) {
            std::cout << "DIAGNOSTIC_CHARACTERISTIC_UNAVAILABLE\n"; return 2;
        }
        auto characteristic = characteristics.Characteristics().GetAt(0);
        const auto properties = characteristic.CharacteristicProperties();
        if ((properties & GattCharacteristicProperties::Read) == GattCharacteristicProperties::None ||
            (properties & GattCharacteristicProperties::Write) == GattCharacteristicProperties::None) {
            std::cout << "READ_WRITE_WITH_RESPONSE_REQUIRED\n"; return 2;
        }
        DataWriter writer;
        writer.WriteBytes(winrt::array_view<const uint8_t>(
            reinterpret_cast<const uint8_t*>(ping.data()), static_cast<uint32_t>(ping.size())));
        const auto written = bounded_result(characteristic.WriteValueAsync(writer.DetachBuffer(), GattWriteOption::WriteWithResponse), deadline);
        if (written != GattCommunicationStatus::Success) { std::cout << "FIXED_WRITE_FAILED\n"; return 2; }
        const auto read = bounded_result(characteristic.ReadValueAsync(BluetoothCacheMode::Uncached), deadline);
        if (read.Status() != GattCommunicationStatus::Success || read.Value().Length() != pong.size()) {
            std::cout << "FIXED_READ_FAILED\n"; return 2;
        }
        uint8_t bytes[pong.size()]{};
        DataReader::FromBuffer(read.Value()).ReadBytes(bytes);
        for (size_t i = 0; i < pong.size(); ++i) {
            if (bytes[i] != static_cast<uint8_t>(pong[i])) { std::cout << "FIXED_REPLY_MISMATCH\n"; return 2; }
        }
        std::cout << "SYNTHETIC_GATT_WRITE_READ_PASS\n"
                     "NOT_AUTHENTICATION_NOT_PRODUCTION_PROTOCOL\n"
                     "NOTIFICATION_MTU_FRAGMENTATION_NOT_TESTED\n";
        return 0;
    } catch (const winrt::hresult_error& error) {
        std::cout << "BLE_TEST_API_FAILED HRESULT=0x" << std::hex
                  << static_cast<unsigned long>(error.code().value) << std::dec << '\n';
    } catch (const std::runtime_error&) { std::cout << "BLE_TEST_DEADLINE_EXPIRED\n"; }
    return 2;
}
} // namespace phase0_ble
