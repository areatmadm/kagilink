// Phase 0 only. No arbitrary target, payload, secret or RDP input API.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <ncrypt.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Devices.Bluetooth.h>
#include <iostream>
#include <string_view>

constexpr bool real_secret_transfer_enabled = false;
constexpr wchar_t synthetic[] = L"KagiLink-SYNTHETIC-ONLY-42";

bool ordinary_integrity() {
    HANDLE token{};
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) {
        std::cout << "TOKEN_QUERY_DENIED WIN32=" << GetLastError() << '\n';
        return false;
    }
    alignas(TOKEN_MANDATORY_LABEL) BYTE buffer[256]{};
    DWORD size{};
    bool permitted = false;
    if (GetTokenInformation(token, TokenIntegrityLevel, buffer, sizeof(buffer), &size)) {
        const auto sid = reinterpret_cast<TOKEN_MANDATORY_LABEL*>(buffer)->Label.Sid;
        const auto count = *GetSidSubAuthorityCount(sid);
        const DWORD level = *GetSidSubAuthority(sid, count - 1);
        std::cout << "PROCESS_INTEGRITY_RID=" << level << '\n';
        permitted = level == SECURITY_MANDATORY_MEDIUM_RID;
    } else std::cout << "INTEGRITY_QUERY_FAILED WIN32=" << GetLastError() << '\n';
    CloseHandle(token);
    return permitted;
}

int inventory() {
    // Never report the adapter address, device names or account identity.
    bool ble = false;
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        const auto operation = winrt::Windows::Devices::Bluetooth::BluetoothAdapter::GetDefaultAsync();
        const auto start = GetTickCount64();
        while (operation.Status() == winrt::Windows::Foundation::AsyncStatus::Started) {
            if (GetTickCount64() - start >= 10000) {
                operation.Cancel();
                std::cout << "BLE_ADAPTER_QUERY_TIMEOUT\n";
                return 2;
            }
            Sleep(20);
        }
        const auto adapter = operation.GetResults();
        if (!adapter) std::cout << "BLE_ADAPTER_ABSENT\n";
        else {
            ble = adapter.IsLowEnergySupported();
            std::cout << (ble ? "BLE_LE_CAPABILITY_REPORTED\n" : "BLE_LE_UNSUPPORTED\n");
            std::cout << (adapter.IsCentralRoleSupported() ? "BLE_CENTRAL_REPORTED\n" : "BLE_CENTRAL_UNSUPPORTED\n");
            ble = ble && adapter.IsCentralRoleSupported();
        }
    } catch (const winrt::hresult_error& error) {
        std::cout << "BLE_API_FAILED HRESULT=0x" << std::hex
                  << static_cast<unsigned long>(error.code().value) << std::dec << '\n';
    }
    NCRYPT_PROV_HANDLE provider{};
    const auto status = NCryptOpenStorageProvider(&provider, MS_PLATFORM_CRYPTO_PROVIDER, 0);
    if (status == ERROR_SUCCESS) {
        std::cout << "TPM_PROVIDER_OPENED_NOT_KEY_VERIFIED\n";
        NCryptFreeObject(provider);
    } else std::cout << "TPM_PROVIDER_UNAVAILABLE_RAM_MODE_ONLY\n";
    std::cout << "GATT_EXCHANGE_NOT_TESTED\nTPM2_AND_KEY_USE_NOT_TESTED\n"
                 "REAL_SECRET_TRANSFER_DISABLED\nRDP_INPUT_UNSUPPORTED\n";
    return ble ? 0 : 2;
}

LRESULT CALLBACK window_proc(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    if (message == WM_DESTROY) { PostQuitMessage(0); return 0; }
    return DefWindowProcW(window, message, wp, lp);
}

void pump() {
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
}

int self_input() {
    // This is an opt-in test against this process's own ordinary edit only.
    // Pre/post checks do NOT eliminate the foreground race in SendInput.
    const auto instance = GetModuleHandleW(nullptr);
    WNDCLASSW wc{};
    wc.lpfnWndProc = window_proc;
    wc.hInstance = instance;
    wc.lpszClassName = L"KagiLink_Phase0_Synthetic";
    if (!RegisterClassW(&wc)) return 2;
    const auto window = CreateWindowW(wc.lpszClassName,
        L"KagiLink synthetic input test — no real passwords", WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT, CW_USEDEFAULT, 600, 180, nullptr, nullptr, instance, nullptr);
    if (!window) return 2;
    const auto edit = CreateWindowW(L"EDIT", L"", WS_CHILD | WS_VISIBLE | WS_BORDER,
        20, 30, 540, 30, window, nullptr, instance, nullptr);
    if (!edit) { DestroyWindow(window); return 2; }
    ShowWindow(window, SW_SHOW);
    SetForegroundWindow(window);
    SetFocus(edit);
    pump();
    bool passed = GetForegroundWindow() == window && GetFocus() == edit;
    if (passed) {
        // Fixed non-secret literal only; Unicode key-down/key-up pairs.
        for (const wchar_t ch : std::wstring_view(synthetic)) {
            if (GetForegroundWindow() != window || GetFocus() != edit) { passed = false; break; }
            INPUT events[2]{};
            events[0].type = events[1].type = INPUT_KEYBOARD;
            events[0].ki.wScan = events[1].ki.wScan = ch;
            events[0].ki.dwFlags = KEYEVENTF_UNICODE;
            events[1].ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
            if (SendInput(2, events, sizeof(INPUT)) != 2) { passed = false; break; }
            // SendInput inserts events asynchronously: pump until processed or timeout.
            Sleep(10);
            pump();
        }
        const auto deadline = GetTickCount64() + 1000;
        wchar_t received[128]{};
        do {
            pump();
            GetWindowTextW(edit, received, 128);
            if (std::wstring_view(received) == synthetic) break;
            Sleep(10);
        } while (GetTickCount64() < deadline);
        passed = passed && GetForegroundWindow() == window && GetFocus() == edit
            && std::wstring_view(received) == synthetic;
        SecureZeroMemory(received, sizeof(received));
    }
    SetWindowTextW(edit, L"");
    DestroyWindow(window);
    std::cout << (passed ? "OWNED_EDIT_SYNTHETIC_READBACK_PASS\n" : "OWNED_EDIT_SYNTHETIC_FAILED_OR_FOCUS_CHANGED\n");
    std::cout << "NO_CREDENTIAL_UI_OR_RDP_SUPPORT_PROVEN\nFOCUS_RACE_NOT_ELIMINATED\n";
    return passed ? 0 : 2;
}

int wmain(int argc, wchar_t** argv) {
    if (argc == 2 && std::wstring_view(argv[1]) == L"--version") {
        std::cout << "KagiLink 0.0.0-phase0 (AreaTM)\nSynthetic-only research; not a production release.\n";
        return 0;
    }
    if (argc == 2 && (std::wstring_view(argv[1]) == L"--help" || std::wstring_view(argv[1]) == L"-h")) {
        std::cout << "KagiLink Phase 0 probe by AreaTM\n"
                     "Usage: kagilink-phase0-probe.exe --inventory | --policy | --self-input | --version | --help\n"
                     "Synthetic-only; no arbitrary password or target arguments supported.\n";
        return 0;
    }
    if (argc == 2 && std::wstring_view(argv[1]) == L"--policy") {
        static_assert(!real_secret_transfer_enabled);
        std::cout << "REAL_SECRET_TRANSFER_DISABLED\nRDP_INPUT_UNSUPPORTED\n";
        return 0;
    }
    const bool normal_user = ordinary_integrity();
    if (argc == 2 && std::wstring_view(argv[1]) == L"--inventory") {
        if (!normal_user) std::cout << "DIAGNOSTIC_ONLY_NOT_NORMAL_USER_FEASIBILITY\n";
        const int result = inventory(); // Read-only API observation even in a restricted sandbox.
        return normal_user ? result : 2;
    }
    if (!normal_user) {
        std::cout << "REFUSED_NOT_NORMAL_MEDIUM_INTEGRITY\n";
        return 2;
    }
    if (argc == 2 && std::wstring_view(argv[1]) == L"--self-input") return self_input();
    std::cout << "Usage: kagilink-phase0-probe.exe --inventory | --policy | --self-input | --version | --help\n"
                 "Synthetic-only; no arbitrary password or target arguments supported.\n";
    return 2;
}
