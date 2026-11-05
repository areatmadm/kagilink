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

    /* Receiver side (D026): only attempts that already sent M3 use the three-failure budget.
     * Failure 1 is the wrong-code attempt above (M3 sent, then a bogus M4). Two more full
     * attempts that reach M3 and then get a bogus M4 destroy the code. */
    for (i = 0; i < 3; i++) {
        uint8_t bogus[33];
        uint8_t m1b[BUF], m2b[BUF], m3b[BUF];
        size_t m1b_len = sizeof m1b, m2b_len = sizeof m2b, m3b_len = sizeof m3b;
        kl_sender_t* s2 = NULL;
        kl_channel_t* ch = NULL;
        memset(bogus, 0, sizeof bogus);
        bogus[0] = 0x14;
        if (i > 0) {
            CHECK(kl_receiver_begin_attempt(f.rx, m1b, &m1b_len) == KL_OK);
            CHECK(kl_sender_start(wrong, 6, m1b, m1b_len, TX_ID, sizeof TX_ID - 1, PURPOSE,
                                  sizeof PURPOSE - 1, CLAIMS, sizeof CLAIMS - 1, m2b, &m2b_len,
                                  &s2) == KL_OK);
            CHECK(kl_receiver_handle_m2(f.rx, m2b, m2b_len, m3b, &m3b_len) == KL_OK);
        }
        st = kl_receiver_finish(f.rx, bogus, sizeof bogus, &ch);
        CHECK(st != KL_OK);
        CHECK(ch == NULL);
        kl_sender_free(s2);
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
    CHECK(kl_channel_approve_one_transfer(f.rx_ch) == KL_ERR_CLOSED);
    CHECK(kl_channel_set_ready_to_receive(f.rx_ch, 1) == KL_ERR_CLOSED);
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

/* ---- BLE chunk codec (ABI v2) ---- */

#define MAXMSG 512
#define MAXCHUNKS 300

typedef struct {
    uint8_t data[MAXMSG * 3];
    size_t off[MAXCHUNKS];
    size_t len[MAXCHUNKS];
    size_t count;
} Chunks;

static void make_msg(uint8_t* m, size_t n) {
    size_t i;
    for (i = 0; i < n; i++) m[i] = (uint8_t)(i * 31 + 7);
}

/* Chunk through the ABI and split the length-prefixed output. */
static kl_status_t chunk_all(uint8_t mtype, const uint8_t* msg, size_t msg_len, size_t max_chunk,
                             Chunks* c) {
    uint8_t packed[MAXMSG * 3];
    size_t n = sizeof packed, cnt = 0, off = 0, i, used = 0;
    kl_status_t st = kl_frame_chunk(mtype, msg, msg_len, max_chunk, packed, &n, &cnt);
    if (st != KL_OK) return st;
    if (cnt > MAXCHUNKS) return KL_ERR_LIMITS;
    for (i = 0; i < cnt; i++) {
        uint32_t l = be32(packed + off);
        c->off[i] = used;
        c->len[i] = l;
        memcpy(c->data + used, packed + off + 4, l);
        used += l;
        off += 4 + (size_t)l;
    }
    c->count = cnt;
    return off == n ? KL_OK : KL_ERR_MALFORMED;
}

static kl_status_t push_chunk(kl_reassembler_t* h, const Chunks* c, size_t i, uint8_t* done) {
    return kl_reassembler_push(h, c->data + c->off[i], c->len[i], done);
}

static void test_frame_round_trip(void) {
    static const size_t lens[] = {1, 2, 5, 19, 20, 64, 65, 255, 269, 274, 300, 511, 512};
    uint8_t msg[MAXMSG], back[MAXMSG + 8];
    static Chunks c;
    kl_reassembler_t* h = NULL;
    size_t max_chunk, li, i;

    CHECK(kl_reassembler_new(&h) == KL_OK && h != NULL);
    for (max_chunk = 20; max_chunk <= 244; max_chunk++) {
        for (li = 0; li < sizeof lens / sizeof lens[0]; li++) {
            size_t len = lens[li], n = sizeof back;
            uint8_t mtype = (len % 2 == 0) ? KL_MTYPE_HANDSHAKE : KL_MTYPE_CHANNEL;
            uint8_t t = 0, done = 9;
            int ok = 1;
            make_msg(msg, len);
            if (chunk_all(mtype, msg, len, max_chunk, &c) != KL_OK) {
                CHECK(0);
                continue;
            }
            for (i = 0; i < c.count; i++) {
                if (c.len[i] > max_chunk || push_chunk(h, &c, i, &done) != KL_OK ||
                    (done == 1) != (i + 1 == c.count))
                    ok = 0;
            }
            CHECK(ok);
            CHECK(kl_reassembler_take(h, &t, back, &n) == KL_OK);
            CHECK(t == mtype && n == len && memcmp(back, msg, len) == 0);
        }
    }
    CHECK(kl_reassembler_is_dead(h) == 0);
    kl_reassembler_free(h);
}

static void test_frame_errors(void) {
    uint8_t msg[200];
    static Chunks c;
    kl_reassembler_t* h = NULL;
    uint8_t done = 0, bad[64];

    make_msg(msg, sizeof msg);
    CHECK(chunk_all(KL_MTYPE_CHANNEL, msg, sizeof msg, 20, &c) == KL_OK);
    CHECK(c.count > 4);

    /* duplicate */
    CHECK(kl_reassembler_new(&h) == KL_OK);
    CHECK(push_chunk(h, &c, 0, &done) == KL_OK && done == 0);
    CHECK(push_chunk(h, &c, 0, &done) == KL_ERR_FRAMING);
    CHECK(kl_reassembler_is_dead(h) == 1);
    CHECK(push_chunk(h, &c, 1, &done) == KL_ERR_CLOSED);
    kl_reassembler_free(h);

    /* gap */
    CHECK(kl_reassembler_new(&h) == KL_OK);
    CHECK(push_chunk(h, &c, 0, &done) == KL_OK);
    CHECK(push_chunk(h, &c, 2, &done) == KL_ERR_FRAMING);
    kl_reassembler_free(h);

    /* reorder */
    CHECK(kl_reassembler_new(&h) == KL_OK);
    CHECK(push_chunk(h, &c, 0, &done) == KL_OK);
    CHECK(push_chunk(h, &c, 1, &done) == KL_OK);
    CHECK(push_chunk(h, &c, 3, &done) == KL_ERR_FRAMING);
    kl_reassembler_free(h);

    /* reserved bit */
    CHECK(kl_reassembler_new(&h) == KL_OK);
    memcpy(bad, c.data + c.off[0], c.len[0]);
    bad[0] |= 0x08;
    CHECK(kl_reassembler_push(h, bad, c.len[0], &done) == KL_ERR_MALFORMED);
    CHECK(kl_reassembler_is_dead(h) == 1);
    kl_reassembler_free(h);

    /* declared length 513 (oversize) */
    CHECK(kl_reassembler_new(&h) == KL_OK);
    memcpy(bad, c.data + c.off[0], c.len[0]);
    bad[3] = 0x02;
    bad[4] = 0x01;
    CHECK(kl_reassembler_push(h, bad, c.len[0], &done) == KL_ERR_LIMITS);
    kl_reassembler_free(h);

    /* oversize chunk, empty chunk, ACK chunk as data */
    CHECK(kl_reassembler_new(&h) == KL_OK);
    {
        static uint8_t big[513];
        memset(big, 1, sizeof big);
        CHECK(kl_reassembler_push(h, big, sizeof big, &done) == KL_ERR_MALFORMED);
    }
    kl_reassembler_free(h);
    CHECK(kl_reassembler_new(&h) == KL_OK);
    CHECK(kl_reassembler_push(h, NULL, 0, &done) == KL_ERR_MALFORMED);
    kl_reassembler_free(h);
    CHECK(kl_reassembler_new(&h) == KL_OK);
    bad[0] = 0x04;
    bad[1] = 0;
    CHECK(kl_reassembler_push(h, bad, 2, &done) == KL_ERR_MALFORMED);
    kl_reassembler_free(h);

    /* chunking input errors */
    {
        uint8_t out[64];
        size_t n = sizeof out, cnt = 0;
        CHECK(kl_frame_chunk(0, msg, 10, 20, out, &n, &cnt) == KL_ERR_LIMITS);
        CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, 0, 20, out, &n, &cnt) == KL_ERR_LIMITS);
        CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, 10, KL_FRAME_MIN_CHUNK - 1, out, &n,
                             &cnt) == KL_ERR_LIMITS);
        CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, 10, KL_FRAME_MAX_CHUNK + 1, out, &n,
                             &cnt) == KL_ERR_LIMITS);
    }
}

static void test_frame_null_and_buffers(void) {
    uint8_t msg[100], out[8], big[1024], held[100], t = 0, done = 0;
    size_t n, cnt = 0, required, tiny_n;
    static Chunks c;
    kl_reassembler_t* h = NULL;
    size_t i;

    make_msg(msg, sizeof msg);
    n = sizeof out;
    CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, NULL, 5, 20, out, &n, &cnt) == KL_ERR_NULL);
    CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, sizeof msg, 20, out, NULL, &cnt) == KL_ERR_NULL);
    CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, sizeof msg, 20, out, &n, NULL) == KL_ERR_NULL);
    /* size query */
    n = 0;
    CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, sizeof msg, 20, NULL, &n, &cnt) ==
          KL_ERR_BUFFER);
    required = n;
    CHECK(required > sizeof msg && cnt == 0);
    /* tiny buffer: nothing written */
    memset(out, 0, sizeof out);
    n = sizeof out;
    CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, sizeof msg, 20, out, &n, &cnt) ==
          KL_ERR_BUFFER);
    CHECK(n == required && out[0] == 0 && out[7] == 0);
    /* exact buffer */
    n = required;
    CHECK(kl_frame_chunk(KL_MTYPE_CHANNEL, msg, sizeof msg, 20, big, &n, &cnt) == KL_OK);
    CHECK(n == required);
    CHECK(chunk_all(KL_MTYPE_CHANNEL, msg, sizeof msg, 20, &c) == KL_OK && c.count == cnt);

    /* reassembler NULLs */
    CHECK(kl_reassembler_new(NULL) == KL_ERR_NULL);
    CHECK(kl_reassembler_push(NULL, msg, 3, &done) == KL_ERR_NULL);
    CHECK(kl_reassembler_is_dead(NULL) == 1);
    kl_reassembler_free(NULL);
    CHECK(kl_reassembler_new(&h) == KL_OK);
    CHECK(kl_reassembler_push(h, c.data, c.len[0], NULL) == KL_ERR_NULL);
    CHECK(kl_reassembler_push(h, NULL, 4, &done) == KL_ERR_NULL);
    CHECK(kl_reassembler_is_dead(h) == 0);
    n = sizeof held;
    CHECK(kl_reassembler_take(NULL, &t, held, &n) == KL_ERR_NULL);
    CHECK(kl_reassembler_take(h, NULL, held, &n) == KL_ERR_NULL);
    CHECK(kl_reassembler_take(h, &t, held, &n) == KL_ERR_WRONG_STATE); /* nothing held */

    /* complete, then tiny take buffer keeps the message held */
    for (i = 0; i < c.count; i++) CHECK(push_chunk(h, &c, i, &done) == KL_OK);
    CHECK(done == 1);
    CHECK(push_chunk(h, &c, 0, &done) == KL_ERR_WRONG_STATE);
    CHECK(kl_reassembler_is_dead(h) == 0);
    tiny_n = 4;
    CHECK(kl_reassembler_take(h, &t, held, &tiny_n) == KL_ERR_BUFFER);
    CHECK(tiny_n == sizeof msg);
    n = 0;
    CHECK(kl_reassembler_take(h, &t, NULL, &n) == KL_ERR_BUFFER && n == sizeof msg);
    n = sizeof held;
    CHECK(kl_reassembler_take(h, &t, held, &n) == KL_OK);
    CHECK(t == KL_MTYPE_CHANNEL && n == sizeof msg && memcmp(held, msg, sizeof msg) == 0);
    kl_reassembler_free(h);
}

static void test_ack(void) {
    uint8_t ack[2], is_ack = 9, seq = 9, one[1];
    size_t n = sizeof ack, n1 = sizeof one;
    int s;

    for (s = 0; s < 256; s += 51) {
        n = sizeof ack;
        CHECK(kl_ack_chunk((uint8_t)s, ack, &n) == KL_OK && n == 2);
        CHECK(kl_parse_ack(ack, 2, &is_ack, &seq) == KL_OK);
        CHECK(is_ack == 1 && seq == (uint8_t)s);
    }
    CHECK(kl_ack_chunk(255, ack, &n) == KL_OK);
    CHECK(kl_parse_ack(ack, 2, &is_ack, &seq) == KL_OK && is_ack == 1 && seq == 255);
    CHECK(kl_ack_chunk(1, one, &n1) == KL_ERR_BUFFER && n1 == 2);
    CHECK(kl_ack_chunk(1, ack, NULL) == KL_ERR_NULL);
    CHECK(kl_ack_chunk(1, NULL, &n) == KL_ERR_NULL);
    /* not an ACK: OK status, flag cleared */
    ack[0] = 0x01;
    ack[1] = 1;
    CHECK(kl_parse_ack(ack, 2, &is_ack, &seq) == KL_OK && is_ack == 0 && seq == 0);
    CHECK(kl_parse_ack(ack, 1, &is_ack, &seq) == KL_OK && is_ack == 0);
    CHECK(kl_parse_ack(NULL, 0, &is_ack, &seq) == KL_OK && is_ack == 0);
    CHECK(kl_parse_ack(NULL, 2, &is_ack, &seq) == KL_ERR_NULL);
    CHECK(kl_parse_ack(ack, 2, NULL, &seq) == KL_ERR_NULL);
    CHECK(kl_parse_ack(ack, 2, &is_ack, NULL) == KL_ERR_NULL);
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
    test_frame_round_trip();
    test_frame_errors();
    test_frame_null_and_buffers();
    test_ack();
    printf("C consumer: %d checks, %d failures\n", g_checks, g_failures);
    return g_failures == 0 ? 0 : 1;
}
