// C++20 consumer test for core/kagilink-ffi/include/kagilink.hpp (RAII wrapper over the C ABI).
// Synthetic data only. Exit code 0 only if every check passed.
#include "kagilink.hpp"

#include <cstdio>
#include <cstring>
#include <string_view>
#include <vector>

namespace {

int g_checks = 0;
int g_failures = 0;

#define CHECK(cond)                                                              \
    do {                                                                         \
        ++g_checks;                                                              \
        if (!(cond)) {                                                           \
            ++g_failures;                                                        \
            std::fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
        }                                                                        \
    } while (0)

using Bytes = std::vector<std::uint8_t>;

Bytes b(std::string_view s) { return Bytes(s.begin(), s.end()); }

struct Established {
    kagilink::Channel tx;
    kagilink::Channel rx;
    kagilink::Receiver receiver;
    kagilink::SecretBytes code;
};

kagilink::Status establish(Established& e, const std::uint8_t* wrong_first_digit = nullptr) {
    using namespace kagilink;
    Status st = Receiver::create(b("cpp-rx"), b("claims"), e.receiver);
    if (st != Ok) return st;
    st = e.receiver.code(e.code);
    if (st != Ok) return st;
    Bytes m1, m2, m3, m4;
    st = e.receiver.begin_attempt(m1);
    if (st != Ok) return st;
    Bytes code(e.code.vec());
    if (wrong_first_digit) code[0] = *wrong_first_digit;
    Sender sender;
    st = Sender::start(code, m1, b("cpp-tx"), b("synthetic-login"), b("claims"), m2, sender);
    if (st != Ok) return st;
    st = e.receiver.handle_m2(m2, m3);
    if (st != Ok) return st;
    st = sender.finish(m3, m4, e.tx);
    if (st != Ok) return st;
    return e.receiver.finish(m4, e.rx);
}

void test_round_trip() {
    using namespace kagilink;
    Established e;
    CHECK(establish(e) == Ok);
    CHECK(e.code.vec().size() == 6);
    CHECK(static_cast<bool>(e.tx) && static_cast<bool>(e.rx));

    const Bytes secret = b("synthetic-secret-0123456789");
    std::vector<Bytes> frags;
    CHECK(e.tx.seal(KL_KIND_SECRET, secret, 8, frags) == KL_ERR_NOT_APPROVED);
    CHECK(e.tx.approve_one_transfer() == Ok);
    CHECK(e.rx.set_ready_to_receive(true) == Ok);
    CHECK(e.tx.seal(KL_KIND_SECRET, secret, 8, frags) == Ok);
    CHECK(frags.size() > 1);

    SecretBytes plain;
    std::uint8_t kind = 0;
    bool complete = false;
    for (std::size_t i = 0; i < frags.size(); ++i) {
        CHECK(e.rx.open_fragment(frags[i], plain, kind, complete) == Ok);
        CHECK(complete == (i + 1 == frags.size()));
    }
    CHECK(kind == KL_KIND_SECRET);
    CHECK(plain.vec() == secret);
    plain.wipe();
    CHECK(plain.vec().empty());

    // Approval is consumed.
    CHECK(e.tx.seal(KL_KIND_SECRET, secret, 8, frags) == KL_ERR_NOT_APPROVED);
}

void test_large_message_grows_buffers() {
    using namespace kagilink;
    Established e;
    CHECK(establish(e) == Ok);
    Bytes big(3000);
    for (std::size_t i = 0; i < big.size(); ++i) big[i] = static_cast<std::uint8_t>(i * 7 + 3);
    std::vector<Bytes> frags;
    CHECK(e.tx.seal(KL_KIND_CONTROL, big, 256, frags) == Ok);  // forces the retry path
    SecretBytes plain;
    std::uint8_t kind = 0;
    bool complete = false;
    for (const auto& f : frags) CHECK(e.rx.open_fragment(f, plain, kind, complete) == Ok);
    CHECK(complete);
    CHECK(plain.vec() == big);  // held-message path: 3000 bytes do not fit the 512 buffer
}

void test_wrong_code_and_tamper() {
    using namespace kagilink;
    {
        Established e;
        const std::uint8_t digit = '0';
        // Make sure the substituted digit differs from the real first digit.
        Receiver probe;
        SecretBytes real;
        CHECK(Receiver::create(b("p"), {}, probe) == Ok);
        CHECK(probe.code(real) == Ok);
        std::uint8_t wrong = real.vec()[0] == digit ? '1' : digit;
        CHECK(establish(e, &wrong) == KL_ERR_CONFIRMATION_FAILED);
        CHECK(!static_cast<bool>(e.tx));
    }
    {
        Established e;
        CHECK(establish(e) == Ok);
        std::vector<Bytes> frags;
        CHECK(e.tx.seal(KL_KIND_CONTROL, b("synthetic-control"), 256, frags) == Ok);
        frags[0].back() ^= 0x01;
        SecretBytes plain;
        std::uint8_t kind = 0;
        bool complete = true;
        CHECK(e.rx.open_fragment(frags[0], plain, kind, complete) != Ok);
        CHECK(!complete);
        CHECK(e.rx.is_closed());
        CHECK(plain.vec().empty());
    }
}

void test_move_semantics() {
    using namespace kagilink;
    SecretBytes a;
    a.vec() = {1, 2, 3};
    SecretBytes c(std::move(a));
    CHECK(a.vec().empty());
    CHECK(c.vec().size() == 3);
    SecretBytes d;
    d = std::move(c);
    CHECK(d.view().size() == 3);
    Receiver r1, r2;
    CHECK(Receiver::create(b("m"), {}, r1) == Ok);
    r2 = std::move(r1);  // moved-from handle must not double free
    SecretBytes code;
    CHECK(r2.code(code) == Ok);
}

Bytes pattern(std::size_t n) {
    Bytes m(n);
    for (std::size_t i = 0; i < n; ++i) m[i] = static_cast<std::uint8_t>(i * 31 + 7);
    return m;
}

void test_frame_round_trip_and_raii() {
    using namespace kagilink;
    Reassembler r;
    CHECK(!static_cast<bool>(r));
    CHECK(Reassembler::create(r) == Ok);
    CHECK(static_cast<bool>(r));
    for (const std::size_t max_chunk : {20u, 23u, 100u, 185u, 244u}) {
        for (const std::size_t len : {1u, 5u, 64u, 269u, 274u, 512u}) {
            const Bytes m = pattern(len);
            const auto want = static_cast<std::uint8_t>(len % 2 ? KL_MTYPE_CHANNEL : KL_MTYPE_HANDSHAKE);
            std::vector<Bytes> chunks;
            CHECK(frame_chunk(want, m, max_chunk, chunks) == Ok);
            SecretBytes msg;
            std::uint8_t mtype = 0;
            bool complete = false;
            for (std::size_t i = 0; i < chunks.size(); ++i) {
                CHECK(chunks[i].size() <= max_chunk);
                CHECK(r.push(chunks[i], mtype, msg, complete) == Ok);
                CHECK(complete == (i + 1 == chunks.size()));
            }
            CHECK(mtype == want);
            CHECK(msg.vec() == m);
        }
    }
    CHECK(!r.is_dead());
    Reassembler moved = std::move(r);  // moved-from handle must not double free
    CHECK(static_cast<bool>(moved));
}

void test_frame_errors_and_ack() {
    using namespace kagilink;
    std::vector<Bytes> chunks;
    CHECK(frame_chunk(KL_MTYPE_CHANNEL, pattern(200), 20, chunks) == Ok);
    CHECK(chunks.size() > 4);
    SecretBytes msg;
    std::uint8_t mtype = 0;
    bool complete = true;
    {
        Reassembler r;
        CHECK(Reassembler::create(r) == Ok);
        CHECK(r.push(chunks[0], mtype, msg, complete) == Ok && !complete);
        CHECK(r.push(chunks[0], mtype, msg, complete) == KL_ERR_FRAMING);  // duplicate
        CHECK(r.is_dead());
        CHECK(r.push(chunks[1], mtype, msg, complete) == KL_ERR_CLOSED);
        CHECK(!complete && msg.vec().empty());
    }
    {
        Reassembler r;
        CHECK(Reassembler::create(r) == Ok);
        CHECK(r.push(chunks[0], mtype, msg, complete) == Ok);
        CHECK(r.push(chunks[2], mtype, msg, complete) == KL_ERR_FRAMING);  // gap
    }
    {
        Reassembler r;
        CHECK(Reassembler::create(r) == Ok);
        Bytes bad = chunks[0];
        bad[0] |= 0x08;  // reserved bit
        CHECK(r.push(bad, mtype, msg, complete) == KL_ERR_MALFORMED);
        CHECK(r.is_dead());
    }
    {
        Reassembler r;
        CHECK(Reassembler::create(r) == Ok);
        Bytes over = chunks[0];
        over[3] = 0x02;
        over[4] = 0x01;  // declared length 513
        CHECK(r.push(over, mtype, msg, complete) == KL_ERR_LIMITS);
    }
    CHECK(frame_chunk(KL_MTYPE_CHANNEL, pattern(513), 20, chunks) == KL_ERR_LIMITS);
    CHECK(frame_chunk(KL_MTYPE_CHANNEL, pattern(10), 5, chunks) == KL_ERR_LIMITS);
    CHECK(chunks.empty());

    // ACK round trip and the "not an ACK is not an error" rule.
    Bytes ack;
    CHECK(ack_chunk(200, ack) == Ok && ack.size() == 2);
    bool is_ack = false;
    std::uint8_t seq = 0;
    CHECK(parse_ack(ack, is_ack, seq) == Ok && is_ack && seq == 200);
    const Bytes data = pattern(10);
    CHECK(parse_ack(data, is_ack, seq) == Ok && !is_ack && seq == 0);
    CHECK(parse_ack({}, is_ack, seq) == Ok && !is_ack);
}

}  // namespace

int main() {
    if (kl_abi_version() != KL_ABI_VERSION) {
        std::fprintf(stderr, "ABI mismatch\n");
        return 2;
    }
    test_round_trip();
    test_large_message_grows_buffers();
    test_wrong_code_and_tamper();
    test_move_semantics();
    test_frame_round_trip_and_raii();
    test_frame_errors_and_ack();
    std::printf("C++ consumer: %d checks, %d failures\n", g_checks, g_failures);
    return g_failures == 0 ? 0 : 1;
}
