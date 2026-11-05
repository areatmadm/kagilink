// C++20 convenience wrapper over the plain C ABI in kagilink.h (D016/D021).
// Header-only, no exceptions, no allocation of secrets outside std::vector buffers that are
// wiped with kl_secure_wipe on destruction. All unsafe pointer handling stays in kagilink.h's
// library; this file only adds RAII ownership and "grow the buffer and retry" convenience.
// Synthetic data only until the independent review of the FFI crate exists.
#pragma once

#include "kagilink.h"

#include <cstddef>
#include <cstdint>
#include <memory>
#include <span>
#include <utility>
#include <vector>

namespace kagilink {

using Status = kl_status_t;
inline constexpr Status Ok = KL_OK;

// Byte buffer for secrets (codes, plaintext). Wiped on destruction, move-only.
class SecretBytes {
public:
    SecretBytes() = default;
    SecretBytes(const SecretBytes&) = delete;
    SecretBytes& operator=(const SecretBytes&) = delete;
    SecretBytes(SecretBytes&& o) noexcept : v_(std::move(o.v_)) { o.v_.clear(); }
    SecretBytes& operator=(SecretBytes&& o) noexcept {
        if (this != &o) {
            wipe();
            v_ = std::move(o.v_);
            o.v_.clear();
        }
        return *this;
    }
    ~SecretBytes() { wipe(); }

    std::vector<std::uint8_t>& vec() { return v_; }
    const std::vector<std::uint8_t>& vec() const { return v_; }
    std::span<const std::uint8_t> view() const { return {v_.data(), v_.size()}; }
    void wipe() {
        if (!v_.empty()) kl_secure_wipe(v_.data(), v_.size());
        v_.clear();
    }

private:
    std::vector<std::uint8_t> v_;
};

namespace detail {

// Call `f(out_ptr, &len)`; on KL_ERR_BUFFER the library reports the required size without
// consuming protocol state, so grow and retry once.
template <class F>
Status with_buffer(std::vector<std::uint8_t>& out, F&& f) {
    out.assign(512, 0);
    for (int attempt = 0; attempt < 2; ++attempt) {
        std::size_t len = out.size();
        const Status st = f(out.data(), &len);
        if (st == KL_ERR_BUFFER) {
            out.assign(len, 0);
            continue;
        }
        if (st == KL_OK) {
            out.resize(len);
        } else if (!out.empty()) {
            kl_secure_wipe(out.data(), out.size());
            out.clear();
        }
        return st;
    }
    return KL_ERR_BUFFER;
}

struct ReceiverDel { void operator()(kl_receiver_t* p) const noexcept { kl_receiver_free(p); } };
struct SenderDel { void operator()(kl_sender_t* p) const noexcept { kl_sender_free(p); } };
struct ChannelDel { void operator()(kl_channel_t* p) const noexcept { kl_channel_free(p); } };

inline const std::uint8_t* ptr(std::span<const std::uint8_t> s) {
    return s.empty() ? nullptr : s.data();
}

}  // namespace detail

// Authenticated encrypted channel (owns the handle; keys are erased by the library).
class Channel {
public:
    Channel() = default;
    explicit Channel(kl_channel_t* raw) : h_(raw) {}
    explicit operator bool() const { return static_cast<bool>(h_); }

    Status approve_one_transfer() { return kl_channel_approve_one_transfer(h_.get()); }
    Status set_ready_to_receive(bool ready) {
        return kl_channel_set_ready_to_receive(h_.get(), ready ? 1 : 0);
    }
    Status close() { return kl_channel_close(h_.get()); }
    bool is_closed() const { return kl_channel_is_closed(h_.get()) != 0; }

    // Encrypt and fragment one message; each fragment is one element of `fragments`.
    Status seal(std::uint8_t kind, std::span<const std::uint8_t> plaintext,
                std::size_t max_fragment_payload,
                std::vector<std::vector<std::uint8_t>>& fragments) {
        fragments.clear();
        std::vector<std::uint8_t> packed;
        std::size_t count = 0;
        const Status st = detail::with_buffer(packed, [&](std::uint8_t* out, std::size_t* len) {
            return kl_channel_seal(h_.get(), kind, detail::ptr(plaintext), plaintext.size(),
                                   max_fragment_payload, out, len, &count);
        });
        if (st != KL_OK) return st;
        std::size_t off = 0;
        for (std::size_t i = 0; i < count; ++i) {
            if (off + 4 > packed.size()) return KL_ERR_MALFORMED;
            const std::size_t n = (std::size_t{packed[off]} << 24) | (std::size_t{packed[off + 1]} << 16) |
                                  (std::size_t{packed[off + 2]} << 8) | std::size_t{packed[off + 3]};
            off += 4;
            if (off + n > packed.size()) return KL_ERR_MALFORMED;
            fragments.emplace_back(packed.begin() + static_cast<std::ptrdiff_t>(off),
                                   packed.begin() + static_cast<std::ptrdiff_t>(off + n));
            off += n;
        }
        if (!packed.empty()) kl_secure_wipe(packed.data(), packed.size());
        return KL_OK;
    }

    // Feed one fragment. `complete` is true only when a full authenticated message was
    // written to `plain`. If the message did not fit, the library holds it and a second call
    // with an empty fragment collects it (handled here).
    Status open_fragment(std::span<const std::uint8_t> fragment, SecretBytes& plain,
                         std::uint8_t& kind, bool& complete) {
        plain.wipe();
        complete = false;
        std::uint8_t done = 0;
        std::vector<std::uint8_t>& out = plain.vec();
        out.assign(512, 0);
        std::size_t len = out.size();
        Status st = kl_channel_open_fragment(h_.get(), detail::ptr(fragment), fragment.size(),
                                             out.data(), &len, &kind, &done);
        if (st == KL_ERR_BUFFER) {
            out.assign(len, 0);
            st = kl_channel_open_fragment(h_.get(), nullptr, 0, out.data(), &len, &kind, &done);
        }
        if (st != KL_OK) {
            plain.wipe();
            return st;
        }
        complete = done != 0;
        out.resize(complete ? len : 0);
        return KL_OK;
    }

private:
    std::unique_ptr<kl_channel_t, detail::ChannelDel> h_;
};

class Receiver {
public:
    static Status create(std::span<const std::uint8_t> device_id,
                         std::span<const std::uint8_t> claims, Receiver& out) {
        kl_receiver_t* raw = nullptr;
        const Status st = kl_receiver_new(detail::ptr(device_id), device_id.size(),
                                          detail::ptr(claims), claims.size(), &raw);
        if (st == KL_OK) out.h_.reset(raw);
        return st;
    }
    // The six ASCII digits to display; wiped when `code` goes out of scope.
    Status code(SecretBytes& code) {
        code.wipe();
        return detail::with_buffer(code.vec(), [&](std::uint8_t* out, std::size_t* len) {
            return kl_receiver_code(h_.get(), out, len);
        });
    }
    Status begin_attempt(std::vector<std::uint8_t>& m1) {
        return detail::with_buffer(m1, [&](std::uint8_t* out, std::size_t* len) {
            return kl_receiver_begin_attempt(h_.get(), out, len);
        });
    }
    Status handle_m2(std::span<const std::uint8_t> m2, std::vector<std::uint8_t>& m3) {
        return detail::with_buffer(m3, [&](std::uint8_t* out, std::size_t* len) {
            return kl_receiver_handle_m2(h_.get(), detail::ptr(m2), m2.size(), out, len);
        });
    }
    Status finish(std::span<const std::uint8_t> m4, Channel& channel) {
        kl_channel_t* raw = nullptr;
        const Status st = kl_receiver_finish(h_.get(), detail::ptr(m4), m4.size(), &raw);
        if (st == KL_OK) channel = Channel(raw);
        return st;
    }

private:
    std::unique_ptr<kl_receiver_t, detail::ReceiverDel> h_;
};

class Sender {
public:
    static Status start(std::span<const std::uint8_t> code, std::span<const std::uint8_t> m1,
                        std::span<const std::uint8_t> device_id,
                        std::span<const std::uint8_t> purpose,
                        std::span<const std::uint8_t> claims, std::vector<std::uint8_t>& m2,
                        Sender& out) {
        kl_sender_t* raw = nullptr;
        const Status st = detail::with_buffer(m2, [&](std::uint8_t* o, std::size_t* len) {
            return kl_sender_start(detail::ptr(code), code.size(), detail::ptr(m1), m1.size(),
                                   detail::ptr(device_id), device_id.size(), detail::ptr(purpose),
                                   purpose.size(), detail::ptr(claims), claims.size(), o, len, &raw);
        });
        if (st == KL_OK) out.h_.reset(raw);
        return st;
    }
    // Verify M3, produce M4 and the channel. The pending state is consumed on every outcome
    // except NULL, BUFFER and BUSY (see kagilink.h); the handle shell is freed on destruction.
    Status finish(std::span<const std::uint8_t> m3, std::vector<std::uint8_t>& m4,
                  Channel& channel) {
        kl_channel_t* raw = nullptr;
        const Status st = detail::with_buffer(m4, [&](std::uint8_t* o, std::size_t* len) {
            return kl_sender_finish(h_.get(), detail::ptr(m3), m3.size(), o, len, &raw);
        });
        if (st == KL_OK) channel = Channel(raw);
        return st;
    }

private:
    std::unique_ptr<kl_sender_t, detail::SenderDel> h_;
};

}  // namespace kagilink
