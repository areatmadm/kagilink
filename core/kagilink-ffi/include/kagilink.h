/*
 * KagiLink C ABI (hand-written to match core/kagilink-ffi exactly; ABI version 1).
 * Apache-2.0. Synthetic-data use only until independent review (D012/D014/D016).
 *
 * Conventions
 *  - Opaque handles are created and freed only by this library. *_free(NULL) is a no-op.
 *    A freed handle must not be used again.
 *  - Output buffers: `uint8_t* out, size_t* out_len`. *out_len is the capacity on entry and
 *    the written length on success. If too small the call returns KL_ERR_BUFFER, sets
 *    *out_len to the required size, writes nothing and consumes no protocol state. `out` may
 *    be NULL with *out_len == 0 for a size query. Outputs are written only on success.
 *  - Inputs: a NULL pointer is accepted only with length 0. Lengths above 1 MiB are rejected
 *    (KL_ERR_LIMITS).
 *  - Handles are single-threaded; a concurrent call returns KL_ERR_BUSY (never blocks).
 *  - KL_ERR_PANIC means an internal panic was caught: discard the handle.
 *  - No function logs, calls back, or returns error text. Never place real credentials
 *    through this API before the project's review gates are met.
 *  - Wipe rule: the receiver code digits, received plaintext and sealed fragments are
 *    secrets or secret-derived. Call kl_secure_wipe on those host buffers as soon as they are
 *    used. The library cannot erase host copies (JNI arrays, Swift Data, UI strings).
 *  - Build note: release packaging should use panic=abort (set at workspace/build level).
 */
#ifndef KAGILINK_H
#define KAGILINK_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define KL_ABI_VERSION 1u

/* Stable numeric values. 1..17 map one-to-one from the core Error enum. */
typedef enum kl_status_t {
  KL_OK = 0,
  KL_ERR_EXPIRED = 1,             /* 60 s code/session deadline passed */
  KL_ERR_NOT_ACTIVE = 2,          /* session consumed, destroyed or inactive */
  KL_ERR_TOO_MANY_FAILURES = 3,   /* three failures: code destroyed */
  KL_ERR_ATTEMPT_IN_PROGRESS = 4,
  KL_ERR_MALFORMED = 5,
  KL_ERR_UNSUPPORTED = 6,         /* version/suite mismatch, no downgrade */
  KL_ERR_WRONG_STATE = 7,
  KL_ERR_PAKE = 8,
  KL_ERR_CONFIRMATION_FAILED = 9, /* wrong code or tampering */
  KL_ERR_BAD_CODE = 10,           /* not exactly six ASCII digits */
  KL_ERR_AUTHENTICATION = 11,
  KL_ERR_FRAMING = 12,
  KL_ERR_LIMITS = 13,
  KL_ERR_COUNTER_EXHAUSTED = 14,
  KL_ERR_NOT_APPROVED = 15,       /* approval/readiness missing or consumed */
  KL_ERR_CLOSED = 16,
  KL_ERR_RANDOMNESS = 17,
  KL_ERR_NULL = 100,
  KL_ERR_BUFFER = 101,
  KL_ERR_BUSY = 102,
  KL_ERR_PANIC = 103,
  KL_ERR_ABI = 104                /* reserved for bindings reporting a version mismatch */
} kl_status_t;

/* Message kinds for kl_channel_seal / kl_channel_open_fragment. */
#define KL_KIND_CONTROL 1
#define KL_KIND_SECRET 2
#define KL_KIND_RESULT 3

typedef struct kl_receiver_t kl_receiver_t;
typedef struct kl_sender_t kl_sender_t;
typedef struct kl_channel_t kl_channel_t;

/* Returns KL_ABI_VERSION. Bindings must refuse to run on mismatch. */
uint32_t kl_abi_version(void);

/* Zero `len` bytes at `ptr` with volatile semantics. NULL or len 0 is a no-op. */
void kl_secure_wipe(uint8_t* ptr, size_t len);

/* ---- Receiver (owns the code, 60 s deadline and three-failure budget) ---- */

/* New session with a fresh code. device_id <= 32 bytes, claims <= 64 bytes (claims are
 * unverified assertions). On failure *out is NULL. */
kl_status_t kl_receiver_new(const uint8_t* device_id, size_t device_id_len,
                            const uint8_t* claims, size_t claims_len,
                            kl_receiver_t** out);

/* Write the six ASCII digits (no NUL terminator). Fails once expired/consumed/destroyed.
 * Display only while active; wipe the host copy afterwards. */
kl_status_t kl_receiver_code(kl_receiver_t* r, uint8_t* out, size_t* out_len);

/* Start an attempt; writes M1. A still-open earlier attempt counts as a failure. */
kl_status_t kl_receiver_begin_attempt(kl_receiver_t* r, uint8_t* out_m1, size_t* out_len);

/* Process M2 from the sender; writes M3. */
kl_status_t kl_receiver_handle_m2(kl_receiver_t* r, const uint8_t* m2, size_t m2_len,
                                  uint8_t* out_m3, size_t* out_len);

/* Verify M4. On success the code is consumed and *out_channel is a new channel; on any
 * failure *out_channel is NULL and a failure is counted. */
kl_status_t kl_receiver_finish(kl_receiver_t* r, const uint8_t* m4, size_t m4_len,
                               kl_channel_t** out_channel);

void kl_receiver_free(kl_receiver_t* r);

/* ---- Sender ---- */

/* Process M1 and write M2. `code` is exactly six ASCII digits typed/entered by the user
 * (KL_ERR_BAD_CODE otherwise). device_id <= 32, purpose <= 32, claims <= 64 bytes.
 * On failure *out_sender is NULL. */
kl_status_t kl_sender_start(const uint8_t* code, size_t code_len,
                            const uint8_t* m1, size_t m1_len,
                            const uint8_t* device_id, size_t device_id_len,
                            const uint8_t* purpose, size_t purpose_len,
                            const uint8_t* claims, size_t claims_len,
                            uint8_t* out_m2, size_t* out_len,
                            kl_sender_t** out_sender);

/* Verify M3, write M4, return the channel. The pending protocol state is CONSUMED on every
 * outcome except KL_ERR_NULL / KL_ERR_BUFFER / KL_ERR_BUSY (rejected before any state is
 * touched). After a consuming call the handle is an empty shell: finish again returns
 * KL_ERR_WRONG_STATE, and it must still be released with kl_sender_free. *out_channel is
 * NULL on failure. */
kl_status_t kl_sender_finish(kl_sender_t* s, const uint8_t* m3, size_t m3_len,
                             uint8_t* out_m4, size_t* out_len,
                             kl_channel_t** out_channel);

void kl_sender_free(kl_sender_t* s);

/* ---- Channel ---- */

/* Sender: allow exactly one SECRET seal. Call only after the platform's own user approval. */
kl_status_t kl_channel_approve_one_transfer(kl_channel_t* c);

/* Receiver: nonzero = a verified safe input target exists; 0 withdraws readiness. */
kl_status_t kl_channel_set_ready_to_receive(kl_channel_t* c, uint8_t ready);

/* Encrypt and fragment one message (1..4096 bytes). `out` receives repeated
 * [uint32 big-endian length][fragment bytes]; *out_count receives the fragment count.
 * max_fragment_payload is clamped to 1..256. KL_ERR_BUFFER (and KL_ERR_NOT_APPROVED /
 * KL_ERR_LIMITS) consume no counter or approval. */
kl_status_t kl_channel_seal(kl_channel_t* c, uint8_t kind,
                            const uint8_t* plaintext, size_t plaintext_len,
                            size_t max_fragment_payload,
                            uint8_t* out, size_t* out_len, size_t* out_count);

/* Feed one received fragment (not the length prefix). *out_complete is 1 only when a full
 * authenticated message was written to out_plain (kind in *out_kind, length in *out_len);
 * otherwise 0 and *out_len is 0. Any protocol error closes the channel. If a completed
 * message does not fit, KL_ERR_BUFFER is returned with the required size and the message is
 * held inside the channel: call again with fragment_len == 0 and a larger buffer (a
 * non-empty fragment while a message is held closes the channel). Wipe out_plain after use. */
kl_status_t kl_channel_open_fragment(kl_channel_t* c, const uint8_t* fragment,
                                     size_t fragment_len,
                                     uint8_t* out_plain, size_t* out_len,
                                     uint8_t* out_kind, uint8_t* out_complete);

/* Close, erase keys and any partial/held data. */
kl_status_t kl_channel_close(kl_channel_t* c);

/* 1 if closed, NULL, busy or unusable; 0 if open. */
uint8_t kl_channel_is_closed(kl_channel_t* c);

void kl_channel_free(kl_channel_t* c);

#ifdef __cplusplus
}
#endif

#endif /* KAGILINK_H */
