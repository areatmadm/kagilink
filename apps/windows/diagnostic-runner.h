// Fixed-command Phase 0 child process runner for the native diagnostic GUI.
// No shell, arbitrary arguments, secret input, files or interactive input probe.
#pragma once
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <algorithm>
#include <string>
#include <vector>

namespace diagnostic_gui {
enum class Action { Inventory, Ble };

class ProbeRunner final {
    class Handle final {
        HANDLE value_{};
    public:
        Handle() noexcept = default;
        explicit Handle(HANDLE value) noexcept : value_(value == INVALID_HANDLE_VALUE ? nullptr : value) {}
        ~Handle() { Reset(); }
        Handle(const Handle&) = delete;
        Handle& operator=(const Handle&) = delete;
        HANDLE Get() const noexcept { return value_; }
        void Reset(HANDLE value = nullptr) noexcept {
            if (value_) CloseHandle(value_);
            value_ = value == INVALID_HANDLE_VALUE ? nullptr : value;
        }
        HANDLE Release() noexcept { HANDLE result = value_; value_ = nullptr; return result; }
    };
    class Attributes final {
        std::vector<unsigned char> storage_;
        LPPROC_THREAD_ATTRIBUTE_LIST list_{};
    public:
        Attributes() = default;
        ~Attributes() { if (list_) DeleteProcThreadAttributeList(list_); }
        Attributes(const Attributes&) = delete;
        Attributes& operator=(const Attributes&) = delete;
        bool Initialize(HANDLE* handles, SIZE_T count) {
            SIZE_T bytes{};
            InitializeProcThreadAttributeList(nullptr, 1, 0, &bytes);
            if (bytes == 0) return false;
            storage_.resize(bytes);
            auto candidate = reinterpret_cast<LPPROC_THREAD_ATTRIBUTE_LIST>(storage_.data());
            if (!InitializeProcThreadAttributeList(candidate, 1, 0, &bytes)) return false;
            list_ = candidate;
            return UpdateProcThreadAttribute(list_, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                handles, sizeof(HANDLE) * count, nullptr, nullptr) != FALSE;
        }
        LPPROC_THREAD_ATTRIBUTE_LIST Get() const noexcept { return list_; }
    };

    Handle process_, job_, read_pipe_;
    std::string output_;
    std::wstring error_;
    DWORD exit_code_ = 2;
    ULONGLONG started_{};
    bool running_ = false;
    bool timed_out_ = false;
    bool cancelled_ = false;
    static constexpr std::size_t max_output = 16 * 1024;
    static constexpr ULONGLONG max_runtime_ms = 70000;

    void CloseHandles() noexcept {
        // KILL_ON_JOB_CLOSE covers any remaining child even during GUI destruction.
        job_.Reset();
        process_.Reset();
        read_pipe_.Reset();
        running_ = false;
    }
    void SetError(const wchar_t* message, DWORD code) {
        error_ = message;
        error_ += L" (Win32 " + std::to_wstring(code) + L")";
    }
    void StopWithError(const wchar_t* message, DWORD code) {
        SetError(message, code);
        exit_code_ = code;
        if (job_.Get()) TerminateJobObject(job_.Get(), code);
        CloseHandles();
    }
    bool Drain() {
        // PeekNamedPipe limits every ReadFile to bytes already available. No pipe
        // wait or unbounded reader loop is allowed on the GUI message thread.
        std::size_t budget = max_output + 1;
        while (budget > 0 && read_pipe_.Get()) {
            DWORD available{};
            if (!PeekNamedPipe(read_pipe_.Get(), nullptr, 0, nullptr, &available, nullptr)) {
                const DWORD code = GetLastError();
                if (code == ERROR_BROKEN_PIPE) return true;
                StopWithError(L"Diagnostic output query failed", code);
                return false;
            }
            if (available == 0) return true;
            if (output_.size() >= max_output) {
                StopWithError(L"Diagnostic output exceeded its limit", ERROR_BUFFER_OVERFLOW);
                return false;
            }
            char bytes[1024]{};
            const auto count = static_cast<DWORD>((std::min)({
                static_cast<std::size_t>(available), sizeof(bytes), max_output - output_.size(), budget}));
            DWORD received{};
            if (!ReadFile(read_pipe_.Get(), bytes, count, &received, nullptr)) {
                const DWORD code = GetLastError();
                if (code == ERROR_BROKEN_PIPE) return true;
                StopWithError(L"Diagnostic output read failed", code);
                return false;
            }
            if (received == 0) return true;
            output_.append(bytes, received);
            budget -= received;
        }
        return true;
    }

public:
    ProbeRunner() = default;
    ~ProbeRunner() { CloseHandles(); }
    ProbeRunner(const ProbeRunner&) = delete;
    ProbeRunner& operator=(const ProbeRunner&) = delete;

    bool Start(Action action) {
        if (running_) return false;
        CloseHandles();
        output_.clear(); error_.clear();
        exit_code_ = 2; timed_out_ = false; cancelled_ = false;
        const wchar_t* argument{};
        switch (action) {
        case Action::Inventory: argument = L"--inventory"; break;
        case Action::Ble: argument = L"--ble-test"; break;
        default: SetError(L"Unsupported diagnostic action", ERROR_INVALID_PARAMETER); return false;
        }

        std::wstring image(32768, L'\0');
        const DWORD length = GetModuleFileNameW(nullptr, image.data(), static_cast<DWORD>(image.size()));
        if (length == 0 || length >= image.size()) {
            SetError(L"Cannot resolve the application directory", length == 0 ? GetLastError() : ERROR_INSUFFICIENT_BUFFER);
            return false;
        }
        image.resize(length);
        const auto separator = image.find_last_of(L"\\/");
        if (separator == std::wstring::npos) { SetError(L"Application directory unavailable", ERROR_BAD_PATHNAME); return false; }
        const std::wstring directory = image.substr(0, separator + 1);
        const std::wstring executable = directory + L"kagilink-phase0-probe.exe";
        std::wstring command = L"\"" + executable + L"\" " + argument;

        SECURITY_ATTRIBUTES security{sizeof(SECURITY_ATTRIBUTES), nullptr, TRUE};
        HANDLE read{}, write{};
        if (!CreatePipe(&read, &write, &security, 4096)) { SetError(L"Cannot create diagnostic pipe", GetLastError()); return false; }
        Handle pending_read(read), pending_write(write);
        if (!SetHandleInformation(read, HANDLE_FLAG_INHERIT, 0)) { SetError(L"Cannot isolate diagnostic pipe", GetLastError()); return false; }
        Handle input(CreateFileW(L"NUL", GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE,
            &security, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr));
        if (!input.Get()) { SetError(L"Cannot open null diagnostic input", GetLastError()); return false; }
        HANDLE inherited[] = {write, input.Get()};
        Attributes attributes;
        if (!attributes.Initialize(inherited, 2)) { SetError(L"Cannot isolate child handles", GetLastError()); return false; }

        Handle pending_job(CreateJobObjectW(nullptr, nullptr));
        if (!pending_job.Get()) { SetError(L"Cannot create diagnostic job", GetLastError()); return false; }
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits{};
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if (!SetInformationJobObject(pending_job.Get(), JobObjectExtendedLimitInformation, &limits, sizeof(limits))) {
            SetError(L"Cannot configure diagnostic cleanup", GetLastError()); return false;
        }
        STARTUPINFOEXW startup{};
        startup.StartupInfo.cb = sizeof(startup);
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = input.Get();
        startup.StartupInfo.hStdOutput = write;
        startup.StartupInfo.hStdError = write;
        startup.lpAttributeList = attributes.Get();
        PROCESS_INFORMATION info{};
        if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, TRUE,
            CREATE_NO_WINDOW | CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT,
            nullptr, directory.c_str(), &startup.StartupInfo, &info)) {
            SetError(L"Cannot start the sibling diagnostic executable", GetLastError()); return false;
        }
        Handle pending_process(info.hProcess), thread(info.hThread);
        if (!AssignProcessToJobObject(pending_job.Get(), pending_process.Get())) {
            const DWORD code = GetLastError();
            TerminateProcess(pending_process.Get(), code); // Still suspended; never ran uncontained.
            SetError(L"Cannot contain diagnostic process", code); return false;
        }
        if (ResumeThread(thread.Get()) == static_cast<DWORD>(-1)) {
            SetError(L"Cannot resume diagnostic process", GetLastError()); return false;
        }
        process_.Reset(pending_process.Release());
        job_.Reset(pending_job.Release());
        read_pipe_.Reset(pending_read.Release());
        started_ = GetTickCount64();
        exit_code_ = STILL_ACTIVE;
        running_ = true;
        return true;
    }

    void Poll() {
        if (!running_) return;
        if (!Drain() || !running_) return;
        const DWORD state = WaitForSingleObject(process_.Get(), 0);
        if (state == WAIT_OBJECT_0) {
            DWORD code{};
            if (!GetExitCodeProcess(process_.Get(), &code)) { StopWithError(L"Cannot query diagnostic completion", GetLastError()); return; }
            // Child stdout is flushed by the CRT before exit. Drain once more to
            // include any final bytes that arrived after the first availability query.
            if (!Drain() || !running_) return;
            exit_code_ = code;
            CloseHandles();
            return;
        }
        if (state == WAIT_FAILED) { StopWithError(L"Cannot monitor diagnostic process", GetLastError()); return; }
        if (GetTickCount64() - started_ >= max_runtime_ms) {
            timed_out_ = true;
            StopWithError(L"Diagnostic exceeded its deadline", ERROR_TIMEOUT);
        }
    }
    void Cancel() {
        if (!running_) return;
        cancelled_ = true;
        exit_code_ = ERROR_CANCELLED;
        if (job_.Get()) TerminateJobObject(job_.Get(), ERROR_CANCELLED);
        CloseHandles();
    }
    bool Running() const noexcept { return running_; }
    DWORD ExitCode() const noexcept { return exit_code_; }
    bool TimedOut() const noexcept { return timed_out_; }
    bool Cancelled() const noexcept { return cancelled_; }
    const std::string& Output() const noexcept { return output_; }
    const std::wstring& Error() const noexcept { return error_; }
};
} // namespace diagnostic_gui
