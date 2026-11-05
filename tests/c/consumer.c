/* Real C consumer of the KagiLink C ABI (core/kagilink-ffi/include/kagilink.h).
 * Synthetic data only. Exercises the public header exactly as a Swift/JNI/C++ host would:
 * handshake, a fragmented SECRET round trip, wrong code, tampering, NULL and small buffers.
 * Exit code 0 only if every check passed. */
#include "kagilink.h"

#include <stdio.h>
#include <string.h>

static int g_failures = 0;
static int g_checks = 0;

#define CHECK(cond)                                                         \
    do {                                                                    \
        g_checks++;                                                         \
        if (!(cond)) {                                                      \
            g_failures++;                                                   \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
        }                                                                   \
    } while (0)

#define BUF 16384

typedef struct {
    kl_receiver_t* rx;
    kl_sender_t* tx;
    kl_channel_t* rx_ch;
    kl_channel_t* tx_ch;
    uint8_t code[6];
    uint8_t m1[BUF], m2[BUF], m3[BUF], m4[BUF];
    size_t m1_len, m2_len, m3_len, m4_len;
} Flow;

static const uint8_t RX_ID[] = "c-rx-device";
static const uint8_t TX_ID[] = "c-tx-device";
static const uint8_t PURPOSE[] = "synthetic-login";
static const uint8_t CLAIMS[] = "claims";

/* Run M1..M3 with the real code, or with `code_override` if given. */
static kl_status_t flow_to_m3(Flow* f, const uint8_t* code_override) {
    kl_status_t st;
    size_t n;
    memset(f, 0, sizeof *f);
    st = kl_receiver_new(RX_ID, sizeof RX_ID - 1, CLAIMS, sizeof CLAIMS - 1, &f->rx);
    if (st != KL_OK) return st;
    n = sizeof f->code;
    st = kl_receiver_code(f->rx, f->code, &n);
    if (st != KL_OK) return st;
    if (n != 6) return KL_ERR_MALFORMED;
    f->m1_len = sizeof f->m1;
    st = kl_receiver_begin_attempt(f->rx, f->m1, &f->m1_len);
    if (st != KL_OK) return st;
    f->m2_len = sizeof f->m2;
    st = kl_sender_start(code_override ? code_override : f->code, 6, f->m1, f->m1_len, TX_ID,
                         sizeof TX_ID - 1, PURPOSE, sizeof PURPOSE - 1, CLAIMS,
                         sizeof CLAIMS - 1, f->m2, &f->m2_len, &f->tx);
    if (st != KL_OK) return st;
    f->m3_len = sizeof f->m3;
    return kl_receiver_handle_m2(f->rx, f->m2, f->m2_len, f->m3, &f->m3_len);
}

static void flow_free(Flow* f) {
    kl_channel_free(f->tx_ch);
    kl_channel_free(f->rx_ch);
    kl_sender_free(f->tx);
    kl_receiver_free(f->rx);
    memset(f, 0, sizeof *f);
}

static uint32_t be32(const uint8_t* p) {
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | p[3];
}

static void test_abi_and_wipe(void) {
    uint8_t z[4] = {1, 2, 3, 4};
    CHECK(kl_abi_version() == KL_ABI_VERSION);
    kl_secure_wipe(z, sizeof z);
    CHECK(z[0] == 0 && z[1] == 0 && z[2] == 0 && z[3] == 0);
    kl_secure_wipe(NULL, 0); /* must not crash */
}

static void test_round_trip(void) {
    Flow f;
    static const uint8_t secret[] = "synthetic-secret-0123456789";
    uint8_t sealed[BUF], plain[BUF];
    size_t sealed_len = sizeof sealed, count = 0, off = 0, i;
    size_t plain_len = 0;
    uint8_t kind = 0, complete = 0;
    kl_status_t st;

    CHECK(flow_to_m3(&f, NULL) == KL_OK);
    f.m4_len = sizeof f.m4;
    st = kl_sender_finish(f.tx, f.m3, f.m3_len, f.m4, &f.m4_len, &f.tx_ch);
    CHECK(st == KL_OK);
    st = kl_receiver_finish(f.rx, f.m4, f.m4_len, &f.rx_ch);
    CHECK(st == KL_OK);
    if (g_failures) {
        flow_free(&f);
        return;
    }

    /* SECRET needs sender approval and receiver readiness. */
    CHECK(kl_channel_seal(f.tx_ch, KL_KIND_SECRET, secret, sizeof secret - 1, 8, sealed,
                          &sealed_len, &count) == KL_ERR_NOT_APPROVED);
    CHECK(kl_channel_approve_one_transfer(f.tx_ch) == KL_OK);
    CHECK(kl_channel_set_ready_to_receive(f.rx_ch, 1) == KL_OK);
    sealed_len = sizeof sealed;
    CHECK(kl_channel_seal(f.tx_ch, KL_KIND_SECRET, secret, sizeof secret - 1, 8, sealed,
                          &sealed_len, &count) == KL_OK);
    CHECK(count > 1); /* really fragmented */

    for (i = 0; i < count; i++) {
        uint32_t flen;
        CHECK(off + 4 <= sealed_len);
        flen = be32(sealed + off);
        off += 4;
        CHECK(off + flen <= sealed_len);
        plain_len = sizeof plain;
        st = kl_channel_open_fragment(f.rx_ch, sealed + off, flen, plain, &plain_len, &kind,
                                      &complete);
        CHECK(st == KL_OK);
        CHECK(complete == (i + 1 == count ? 1 : 0));
        off += flen;
    }
    CHECK(plain_len == sizeof secret - 1);
    CHECK(memcmp(plain, secret, sizeof secret - 1) == 0);
    CHECK(kind == KL_KIND_SECRET);
    kl_secure_wipe(plain, sizeof plain);

    /* Approval is consumed: a second SECRET is refused. */
    sealed_len = sizeof sealed;
    CHECK(kl_channel_seal(f.tx_ch, KL_KIND_SECRET, secret, sizeof secret - 1, 8, sealed,
                          &sealed_len, &count) == KL_ERR_NOT_APPROVED);
    flow_free(&f);
}

static void test_wrong_code(void) {
    Flow f;
    uint8_t wrong[6];
    kl_status_t st;
    int i;

    /* First get the real code, then run the sender with one digit changed. */
    memset(&f, 0, sizeof f);
    CHECK(kl_receiver_new(RX_ID, sizeof RX_ID - 1, CLAIMS, sizeof CLAIMS - 1, &f.rx) == KL_OK);
    {
        size_t n = sizeof f.code;
        CHECK(kl_receiver_code(f.rx, f.code, &n) == KL_OK);
    }
    memcpy(wrong, f.code, 6);
    wrong[0] = (wrong[0] == '0') ? '1' : '0';
    f.m1_len = sizeof f.m1;
    CHECK(kl_receiver_begin_attempt(f.rx, f.m1, &f.m1_len) == KL_OK);
    f.m2_len = sizeof f.m2;
    CHECK(kl_sender_start(wrong, 6, f.m1, f.m1_len, TX_ID, sizeof TX_ID - 1, PURPOSE,
                          sizeof PURPOSE - 1, CLAIMS, sizeof CLAIMS - 1, f.m2, &f.m2_len,
                          &f.tx) == KL_OK);
    f.m3_len = sizeof f.m3;
    CHECK(kl_receiver_handle_m2(f.rx, f.m2, f.m2_len, f.m3, &f.m3_len) == KL_OK);
    f.m4_len = sizeof f.m4;
    st = kl_sender_finish(f.tx, f.m3, f.m3_len, f.m4, &f.m4_len, &f.tx_ch);
    CHECK(st == KL_ERR_CONFIRMATION_FAILED);
    CHECK(f.tx_ch == NULL);
    /* The pending state is consumed even on failure. */
    f.m4_len = sizeof f.m4;
    CHECK(kl_sender_finish(f.tx, f.m3, f.m3_len, f.m4, &f.m4_len, &f.tx_ch) != KL_OK);

    /* Receiver side: bogus M4 values burn the three-failure budget, then the code is gone. */
    for (i = 0; i < 3; i++) {
        uint8_t bogus[33];
        uint8_t m1b[BUF];
        size_t m1b_len = sizeof m1b;
        kl_channel_t* ch = NULL;
        memset(bogus, 0, sizeof bogus);
        bogus[0] = 0x14;
        if (i > 0) CHECK(kl_receiver_begin_attempt(f.rx, m1b, &m1b_len) == KL_OK);
        st = kl_receiver_finish(f.rx, bogus, sizeof bogus, &ch);
        CHECK(st != KL_OK);
        CHECK(ch == NULL);
    }
    {
        uint8_t code[6];
        size_t n = sizeof code;
        CHECK(kl_receiver_code(f.rx, code, &n) == KL_ERR_TOO_MANY_FAILURES);
    }
    flow_free(&f);
}

static void test_tamper_closes_channel(void) {
    Flow f;
    static const uint8_t msg[] = "synthetic-control";
    uint8_t sealed[BUF], plain[BUF];
    size_t sealed_len = sizeof sealed, count = 0, plain_len = sizeof plain;
    uint8_t kind = 0, complete = 0;
    uint32_t flen;
    kl_status_t st;

    CHECK(flow_to_m3(&f, NULL) == KL_OK);
    f.m4_len = sizeof f.m4;
    CHECK(kl_sender_finish(f.tx, f.m3, f.m3_len, f.m4, &f.m4_len, &f.tx_ch) == KL_OK);
    CHECK(kl_receiver_finish(f.rx, f.m4, f.m4_len, &f.rx_ch) == KL_OK);
    CHECK(kl_channel_seal(f.tx_ch, KL_KIND_CONTROL, msg, sizeof msg - 1, 256, sealed,
                          &sealed_len, &count) == KL_OK);
    CHECK(count == 1);
    flen = be32(sealed);
    sealed[4 + flen - 1] ^= 0x01; /* flip a tag bit */
    st = kl_channel_open_fragment(f.rx_ch, sealed + 4, flen, plain, &plain_len, &kind,
                                  &complete);
    CHECK(st != KL_OK);
    CHECK(complete == 0);
    CHECK(kl_channel_is_closed(f.rx_ch) == 1);
    flow_free(&f);
}

static void test_null_and_buffers(void) {
    kl_receiver_t* r = NULL;
    uint8_t small[2];
    uint8_t code[6];
    size_t n;
    CHECK(kl_receiver_new(NULL, 1, NULL, 0, &r) == KL_ERR_NULL);
    CHECK(kl_receiver_new(RX_ID, sizeof RX_ID - 1, CLAIMS, sizeof CLAIMS - 1, NULL) ==
          KL_ERR_NULL);
    CHECK(kl_receiver_new(RX_ID, sizeof RX_ID - 1, CLAIMS, sizeof CLAIMS - 1, &r) == KL_OK);
    n = sizeof small;
    CHECK(kl_receiver_code(r, small, &n) == KL_ERR_BUFFER);
    CHECK(n == 6); /* required size reported, nothing consumed */
    n = sizeof code;
    CHECK(kl_receiver_code(r, code, &n) == KL_OK && n == 6);
    CHECK(kl_receiver_code(NULL, code, &n) == KL_ERR_NULL);
    kl_receiver_free(r);
    kl_receiver_free(NULL);
    kl_sender_free(NULL);
    kl_channel_free(NULL);
    CHECK(kl_channel_is_closed(NULL) == 1);
}

int main(void) {
    if (kl_abi_version() != KL_ABI_VERSION) {
        fprintf(stderr, "ABI mismatch: library %u, header %u\n", (unsigned)kl_abi_version(),
                (unsigned)KL_ABI_VERSION);
        return 2;
    }
    test_abi_and_wipe();
    test_round_trip();
    test_wrong_code();
    test_tamper_closes_channel();
    test_null_and_buffers();
    printf("C consumer: %d checks, %d failures\n", g_checks, g_failures);
    return g_failures == 0 ? 0 : 1;
}
