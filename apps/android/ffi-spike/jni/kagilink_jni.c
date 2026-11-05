/* Thin JNI shim over the KagiLink C ABI (core/kagilink-ffi/include/kagilink.h).
 * Binding spike (D021): synthetic data only. All raw-pointer handling lives in this one file.
 *
 * Rules:
 *  - Handles cross JNI as jlong and are opaque to Kotlin. The library, not this shim, owns
 *    validation (0 -> NULL -> KL_ERR_NULL).
 *  - Every Java byte[] input is copied to native memory, passed to the library, then WIPED and
 *    freed. Every output buffer is a native scratch copy that is copied back to the Java array only
 *    on KL_OK, then wiped and freed. No JNI critical sections or pinned arrays are held across
 *    library calls, and no Java String is ever used for a secret.
 *  - The library cannot erase the Java-side arrays: Kotlin must call Native.secureWipe on them.
 *  - A null Java array becomes (NULL, 0). A NULL pointer with a non-zero length therefore cannot be
 *    produced through this shim by construction.
 *  - Scratch allocation or JNI failure returns KL_ERR_NULL (100) without touching library state.
 */
#include <jni.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "kagilink.h"

typedef struct {
  uint8_t* p;
  size_t n;
} In;

typedef struct {
  uint8_t* p;
  size_t cap; /* capacity in, written/required length out */
} Out;

static int in_get(JNIEnv* env, jbyteArray a, In* o) {
  o->p = NULL;
  o->n = 0;
  if (a == NULL) return 0;
  jsize n = (*env)->GetArrayLength(env, a);
  uint8_t* p = (uint8_t*)calloc(n > 0 ? (size_t)n : 1, 1);
  if (p == NULL) return -1;
  if (n > 0) {
    (*env)->GetByteArrayRegion(env, a, 0, n, (jbyte*)p);
    if ((*env)->ExceptionCheck(env)) {
      free(p);
      return -1;
    }
  }
  o->p = p;
  o->n = (size_t)n;
  return 0;
}

static void in_put(In* o) {
  if (o->p != NULL) {
    kl_secure_wipe(o->p, o->n > 0 ? o->n : 1);
    free(o->p);
  }
  o->p = NULL;
  o->n = 0;
}

/* lenArr[0] is the capacity offered on entry (clamped to the array length). */
static int out_begin(JNIEnv* env, jbyteArray arr, jlongArray lenArr, Out* o) {
  jlong cap = 0;
  o->p = NULL;
  o->cap = 0;
  if (lenArr == NULL) return -1;
  (*env)->GetLongArrayRegion(env, lenArr, 0, 1, &cap);
  if ((*env)->ExceptionCheck(env)) return -1;
  if (arr == NULL || cap < 0) return arr == NULL ? 0 : -1;
  jsize alen = (*env)->GetArrayLength(env, arr);
  size_t c = (size_t)cap < (size_t)alen ? (size_t)cap : (size_t)alen;
  o->p = (uint8_t*)calloc(c > 0 ? c : 1, 1);
  if (o->p == NULL) return -1;
  o->cap = c;
  return 0;
}

static void out_end(JNIEnv* env, Out* o, jbyteArray arr, jlongArray lenArr, kl_status_t st) {
  /* The length slot carries information only on KL_OK (bytes written) and KL_ERR_BUFFER (bytes
   * required); on every other status the library left it as the capacity, so report 0. */
  jlong len = (st == KL_OK || st == KL_ERR_BUFFER) ? (jlong)o->cap : 0;
  if (lenArr != NULL) (*env)->SetLongArrayRegion(env, lenArr, 0, 1, &len);
  if (st == KL_OK && arr != NULL && o->p != NULL && o->cap > 0) {
    (*env)->SetByteArrayRegion(env, arr, 0, (jsize)o->cap, (const jbyte*)o->p);
  }
  if (o->p != NULL) {
    kl_secure_wipe(o->p, o->cap > 0 ? o->cap : 1);
    free(o->p);
  }
  o->p = NULL;
}

/* Publishes a freshly created handle to Java. Returns -1 if the write failed (pending JNI
 * exception); the caller must then free the handle itself so it cannot leak or be orphaned. */
static int handle_out(JNIEnv* env, jlongArray arr, void* ptr) {
  if (arr != NULL && ptr != NULL) {
    jlong v = (jlong)(uintptr_t)ptr;
    (*env)->SetLongArrayRegion(env, arr, 0, 1, &v);
    if ((*env)->ExceptionCheck(env)) return -1;
  }
  return 0;
}

#define J(name) JNIEXPORT jint JNICALL Java_com_areatm_kagilink_spike_Native_##name
#define JV(name) JNIEXPORT void JNICALL Java_com_areatm_kagilink_spike_Native_##name
#define H(T, v) ((T*)(uintptr_t)(v))
#define FAIL_NULL ((jint)KL_ERR_NULL)

J(abiVersion)(JNIEnv* env, jclass c) {
  (void)env;
  (void)c;
  return (jint)kl_abi_version();
}

JV(secureWipe)(JNIEnv* env, jclass c, jbyteArray a) {
  (void)c;
  if (a == NULL) return;
  jsize n = (*env)->GetArrayLength(env, a);
  if (n <= 0) return;
  uint8_t* zero = (uint8_t*)calloc((size_t)n, 1);
  if (zero == NULL) return;
  (*env)->SetByteArrayRegion(env, a, 0, n, (const jbyte*)zero);
  free(zero);
}

J(receiverNew)(JNIEnv* env, jclass c, jbyteArray id, jbyteArray claims, jlongArray outH) {
  (void)c;
  In a, b;
  if (in_get(env, id, &a) != 0) return FAIL_NULL;
  if (in_get(env, claims, &b) != 0) {
    in_put(&a);
    return FAIL_NULL;
  }
  kl_receiver_t* r = NULL;
  kl_status_t st = kl_receiver_new(a.p, a.n, b.p, b.n, outH != NULL ? &r : NULL);
  jint rc = (jint)st;
  if (handle_out(env, outH, r) != 0) {
    kl_receiver_free(r);
    rc = FAIL_NULL;
  }
  in_put(&a);
  in_put(&b);
  return rc;
}

J(receiverCode)(JNIEnv* env, jclass c, jlong h, jbyteArray out, jlongArray outLen) {
  (void)c;
  Out o;
  if (out_begin(env, out, outLen, &o) != 0) return FAIL_NULL;
  kl_status_t st = kl_receiver_code(H(kl_receiver_t, h), o.p, &o.cap);
  out_end(env, &o, out, outLen, st);
  return (jint)st;
}

J(receiverBeginAttempt)(JNIEnv* env, jclass c, jlong h, jbyteArray out, jlongArray outLen) {
  (void)c;
  Out o;
  if (out_begin(env, out, outLen, &o) != 0) return FAIL_NULL;
  kl_status_t st = kl_receiver_begin_attempt(H(kl_receiver_t, h), o.p, &o.cap);
  out_end(env, &o, out, outLen, st);
  return (jint)st;
}

J(receiverHandleM2)(JNIEnv* env, jclass c, jlong h, jbyteArray m2, jbyteArray out,
                    jlongArray outLen) {
  (void)c;
  In a;
  Out o;
  if (in_get(env, m2, &a) != 0) return FAIL_NULL;
  if (out_begin(env, out, outLen, &o) != 0) {
    in_put(&a);
    return FAIL_NULL;
  }
  kl_status_t st = kl_receiver_handle_m2(H(kl_receiver_t, h), a.p, a.n, o.p, &o.cap);
  out_end(env, &o, out, outLen, st);
  in_put(&a);
  return (jint)st;
}

J(receiverFinish)(JNIEnv* env, jclass c, jlong h, jbyteArray m4, jlongArray outCh) {
  (void)c;
  In a;
  if (in_get(env, m4, &a) != 0) return FAIL_NULL;
  kl_channel_t* ch = NULL;
  kl_status_t st = kl_receiver_finish(H(kl_receiver_t, h), a.p, a.n, outCh != NULL ? &ch : NULL);
  jint rc = (jint)st;
  if (handle_out(env, outCh, ch) != 0) {
    kl_channel_free(ch);
    rc = FAIL_NULL;
  }
  in_put(&a);
  return rc;
}

JV(receiverFree)(JNIEnv* env, jclass c, jlong h) {
  (void)env;
  (void)c;
  kl_receiver_free(H(kl_receiver_t, h));
}

J(senderStart)(JNIEnv* env, jclass c, jbyteArray code, jbyteArray m1, jbyteArray devId,
               jbyteArray purpose, jbyteArray claims, jbyteArray out, jlongArray outLen,
               jlongArray outSender) {
  (void)c;
  In code_, m1_, dev_, pur_, cla_;
  Out o;
  jint rc = FAIL_NULL;
  code_.p = m1_.p = dev_.p = pur_.p = cla_.p = NULL;
  code_.n = m1_.n = dev_.n = pur_.n = cla_.n = 0;
  o.p = NULL;
  o.cap = 0;
  if (in_get(env, code, &code_) != 0 || in_get(env, m1, &m1_) != 0 ||
      in_get(env, devId, &dev_) != 0 || in_get(env, purpose, &pur_) != 0 ||
      in_get(env, claims, &cla_) != 0 || out_begin(env, out, outLen, &o) != 0) {
    goto done;
  }
  {
    kl_sender_t* s = NULL;
    kl_status_t st =
        kl_sender_start(code_.p, code_.n, m1_.p, m1_.n, dev_.p, dev_.n, pur_.p, pur_.n, cla_.p,
                        cla_.n, o.p, &o.cap, outSender != NULL ? &s : NULL);
    rc = (jint)st;
    if (handle_out(env, outSender, s) != 0) {
      kl_sender_free(s);
      st = KL_ERR_NULL;
      rc = FAIL_NULL;
    }
    out_end(env, &o, out, outLen, st);
  }
done:
  if (o.p != NULL) {
    kl_secure_wipe(o.p, o.cap > 0 ? o.cap : 1);
    free(o.p);
  }
  in_put(&code_);
  in_put(&m1_);
  in_put(&dev_);
  in_put(&pur_);
  in_put(&cla_);
  return rc;
}

J(senderFinish)(JNIEnv* env, jclass c, jlong h, jbyteArray m3, jbyteArray out, jlongArray outLen,
                jlongArray outCh) {
  (void)c;
  In a;
  Out o;
  if (in_get(env, m3, &a) != 0) return FAIL_NULL;
  if (out_begin(env, out, outLen, &o) != 0) {
    in_put(&a);
    return FAIL_NULL;
  }
  kl_channel_t* ch = NULL;
  kl_status_t st = kl_sender_finish(H(kl_sender_t, h), a.p, a.n, o.p, &o.cap,
                                    outCh != NULL ? &ch : NULL);
  jint rc = (jint)st;
  if (handle_out(env, outCh, ch) != 0) {
    kl_channel_free(ch);
    st = KL_ERR_NULL;
    rc = FAIL_NULL;
  }
  out_end(env, &o, out, outLen, st);
  in_put(&a);
  return rc;
}

JV(senderFree)(JNIEnv* env, jclass c, jlong h) {
  (void)env;
  (void)c;
  kl_sender_free(H(kl_sender_t, h));
}

J(channelApproveOneTransfer)(JNIEnv* env, jclass c, jlong h) {
  (void)env;
  (void)c;
  return (jint)kl_channel_approve_one_transfer(H(kl_channel_t, h));
}

J(channelSetReady)(JNIEnv* env, jclass c, jlong h, jint ready) {
  (void)env;
  (void)c;
  return (jint)kl_channel_set_ready_to_receive(H(kl_channel_t, h), ready != 0 ? 1 : 0);
}

J(channelSeal)(JNIEnv* env, jclass c, jlong h, jint kind, jbyteArray plain, jlong maxFrag,
               jbyteArray out, jlongArray outLen, jlongArray outCount) {
  (void)c;
  In a;
  Out o;
  /* Validate before any cast or allocation: kind is KL_KIND_CONTROL..KL_KIND_RESULT and the
   * fragment payload size is 1..256 (the library's clamp range, now enforced strictly here). */
  if (kind < KL_KIND_CONTROL || kind > KL_KIND_RESULT) return (jint)KL_ERR_LIMITS;
  if (maxFrag < 1 || maxFrag > 256) return (jint)KL_ERR_LIMITS;
  if (in_get(env, plain, &a) != 0) return FAIL_NULL;
  if (out_begin(env, out, outLen, &o) != 0) {
    in_put(&a);
    return FAIL_NULL;
  }
  size_t count = 0;
  kl_status_t st = kl_channel_seal(H(kl_channel_t, h), (uint8_t)kind, a.p, a.n,
                                   (size_t)maxFrag, o.p, &o.cap,
                                   outCount != NULL ? &count : NULL);
  if (outCount != NULL) {
    jlong cnt = (jlong)count;
    (*env)->SetLongArrayRegion(env, outCount, 0, 1, &cnt);
  }
  out_end(env, &o, out, outLen, st);
  in_put(&a);
  return (jint)st;
}

/* outInfo[0] = kind, outInfo[1] = complete (1 only when a whole authenticated message is in out). */
J(channelOpenFragment)(JNIEnv* env, jclass c, jlong h, jbyteArray frag, jbyteArray outPlain,
                       jlongArray outLen, jintArray outInfo) {
  (void)c;
  In a;
  Out o;
  if (outInfo == NULL) return FAIL_NULL;
  if (in_get(env, frag, &a) != 0) return FAIL_NULL;
  if (out_begin(env, outPlain, outLen, &o) != 0) {
    in_put(&a);
    return FAIL_NULL;
  }
  uint8_t kind = 0, complete = 0;
  kl_status_t st = kl_channel_open_fragment(H(kl_channel_t, h), a.p, a.n, o.p, &o.cap, &kind,
                                            &complete);
  jint info[2] = {(jint)kind, (jint)complete};
  (*env)->SetIntArrayRegion(env, outInfo, 0, 2, info);
  out_end(env, &o, outPlain, outLen, st);
  in_put(&a);
  return (jint)st;
}

J(channelClose)(JNIEnv* env, jclass c, jlong h) {
  (void)env;
  (void)c;
  return (jint)kl_channel_close(H(kl_channel_t, h));
}

J(channelIsClosed)(JNIEnv* env, jclass c, jlong h) {
  (void)env;
  (void)c;
  return (jint)kl_channel_is_closed(H(kl_channel_t, h));
}

JV(channelFree)(JNIEnv* env, jclass c, jlong h) {
  (void)env;
  (void)c;
  kl_channel_free(H(kl_channel_t, h));
}
