package `in`.minori.key.phase0.strongbox

import android.content.Context
import android.content.pm.PackageManager
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.Signature
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import java.util.Date
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec

enum class Status { SUPPORTED, UNSUPPORTED, UNVERIFIABLE, FAILED }

class Result(val name: String, val status: Status, val reason: String) {
    fun line() = "CHECK $name $status $reason"
}

/**
 * Phase 0 StrongBox verification. Synthetic random data only; no real credentials. Every key is
 * StrongBox-requested with setIsStrongBoxBacked(true) and is never retried as TEE or software.
 */
internal object StrongBoxChecks {
    const val ALIAS_PREFIX = "kagilink_phase0_sb_"
    private const val ATTESTATION_OID = "1.3.6.1.4.1.11129.2.1.17"
    private const val PROVIDER = "AndroidKeyStore"
    private val random = SecureRandom()

    fun keyStore(): KeyStore = KeyStore.getInstance(PROVIDER).apply { load(null) }
    fun randomBytes(n: Int) = ByteArray(n).also { random.nextBytes(it) }

    fun levelName(level: Int) = when (level) {
        KeyProperties.SECURITY_LEVEL_SOFTWARE -> "Software"
        KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT -> "TEE"
        KeyProperties.SECURITY_LEVEL_STRONGBOX -> "StrongBox"
        else -> "Unknown($level)"
    }

    fun describe(e: Throwable): String =
        (e.javaClass.simpleName + (e.message?.let { ": $it" } ?: "")).replace('\n', ' ').take(180)

    fun isStrongBoxUnavailable(e: Throwable): Boolean {
        var t: Throwable? = e
        while (t != null) { if (t is StrongBoxUnavailableException) return true; t = t.cause }
        return false
    }

    fun failure(name: String, e: Throwable, what: String) =
        if (isStrongBoxUnavailable(e)) Result(name, Status.UNSUPPORTED, "StrongBoxUnavailableException during $what")
        else Result(name, Status.FAILED, "$what: ${describe(e)}")

    fun deleteAlias(alias: String) {
        try { keyStore().deleteEntry(alias) } catch (ignored: Exception) { /* verified by the cleanup check */ }
    }

    fun runNonBiometric(context: Context): List<Result> {
        val out = mutableListOf<Result>()
        val flag = context.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)
        out += Result("feature_flag", if (flag) Status.SUPPORTED else Status.UNSUPPORTED,
            "FEATURE_STRONGBOX_KEYSTORE=$flag (informational, not proof of key behavior)")
        out += ecChecks()
        out += aesChecks()
        out += cleanup()
        out += overall(out)
        return out
    }

    private fun ecChecks(): List<Result> {
        val alias = ALIAS_PREFIX + "ec"
        val challenge = randomBytes(32)
        try {
            val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setAttestationChallenge(challenge)
                .setIsStrongBoxBacked(true)
                .build()
            val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, PROVIDER)
            generator.initialize(spec)
            val pair = generator.generateKeyPair()
            val info = KeyFactory.getInstance(pair.private.algorithm, PROVIDER)
                .getKeySpec(pair.private, KeyInfo::class.java)
            val sign = if (info.securityLevel != KeyProperties.SECURITY_LEVEL_STRONGBOX) {
                Result("ecdsa_p256_sign", Status.FAILED, "key created but securityLevel=${levelName(info.securityLevel)}")
            } else {
                val message = randomBytes(64)
                val signer = Signature.getInstance("SHA256withECDSA").apply { initSign(pair.private); update(message) }
                val signature = signer.sign()
                val good = Signature.getInstance("SHA256withECDSA").apply { initVerify(pair.public); update(message) }.verify(signature)
                val tampered = message.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }
                val bad = Signature.getInstance("SHA256withECDSA").apply { initVerify(pair.public); update(tampered) }.verify(signature)
                if (good && !bad) Result("ecdsa_p256_sign", Status.SUPPORTED, "securityLevel=StrongBox, sign/verify ok, tampered message rejected")
                else Result("ecdsa_p256_sign", Status.FAILED, "signature verify good=$good tamperedAccepted=$bad")
            }
            val attest = if (sign.status == Status.SUPPORTED) attestation(alias, challenge, pair.public.encoded)
            else Result("attestation_ec", Status.UNVERIFIABLE, "no usable StrongBox EC key to attest")
            return listOf(sign, attest)
        } catch (e: Exception) {
            return listOf(failure("ecdsa_p256_sign", e, "EC key generation/use"),
                Result("attestation_ec", Status.UNVERIFIABLE, "no EC key was created"))
        } finally {
            deleteAlias(alias)
        }
    }

    private fun attestation(alias: String, challenge: ByteArray, publicKey: ByteArray): Result {
        val name = "attestation_ec"
        try {
            val chain = keyStore().getCertificateChain(alias)?.map { it as X509Certificate }
            if (chain.isNullOrEmpty()) return Result(name, Status.UNVERIFIABLE, "no certificate chain returned")
            val now = Date()
            chain.forEach { it.checkValidity(now) }
            for (i in 0 until chain.size - 1) chain[i].verify(chain[i + 1].publicKey)
            val root = chain.last()
            val selfSigned = try { root.verify(root.publicKey); true } catch (ignored: Exception) { false }
            if (!MessageDigest.isEqual(chain[0].publicKey.encoded, publicKey))
                return Result(name, Status.FAILED, "leaf certificate public key differs from generated key")
            val extension = chain[0].getExtensionValue(ATTESTATION_OID)
                ?: return Result(name, Status.FAILED, "attestation extension missing in leaf certificate")
            val description = KeyDescription.parse(extension)
            val spki = MessageDigest.getInstance("SHA-256").digest(root.publicKey.encoded)
                .joinToString("") { "%02x".format(it) }
            val facts = "chain=${chain.size} sigs=ok validity=ok attVer=${description.attestationVersion} " +
                "attLevel=${levelName(description.attestationSecurityLevel)} kmLevel=${levelName(description.keymasterSecurityLevel)} " +
                "root=\"${root.subjectX500Principal.name.take(60)}\" rootSelfSigned=$selfSigned rootSpkiSha256=$spki"
            if (!MessageDigest.isEqual(description.attestationChallenge, challenge))
                return Result(name, Status.FAILED, "challenge mismatch; $facts")
            if (description.attestationSecurityLevel != KeyProperties.SECURITY_LEVEL_STRONGBOX ||
                description.keymasterSecurityLevel != KeyProperties.SECURITY_LEVEL_STRONGBOX)
                return Result(name, Status.FAILED, "attestation level is not StrongBox; $facts")
            return Result(name, Status.SUPPORTED, "challenge=match $facts [offline: chain consistency only; root pinning and revocation NOT VERIFIED]")
        } catch (e: IllegalArgumentException) {
            return Result(name, Status.UNVERIFIABLE, "attestation extension not parsed: ${describe(e)}")
        } catch (e: Exception) {
            return Result(name, Status.FAILED, "chain verification: ${describe(e)}")
        }
    }

    private fun aesChecks(): List<Result> {
        val alias = ALIAS_PREFIX + "aes"
        try {
            val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setIsStrongBoxBacked(true)
                .build()
            val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
            generator.init(spec)
            val key: SecretKey = generator.generateKey()
            val info = SecretKeyFactory.getInstance(key.algorithm, PROVIDER).getKeySpec(key, KeyInfo::class.java) as KeyInfo
            val gcm = if (info.securityLevel != KeyProperties.SECURITY_LEVEL_STRONGBOX) {
                Result("aes256_gcm", Status.FAILED, "key created but securityLevel=${levelName(info.securityLevel)}")
            } else if (info.keySize != 256) {
                Result("aes256_gcm", Status.FAILED, "keySize=${info.keySize}")
            } else {
                val plain = randomBytes(48)
                val enc = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key) }
                val iv = enc.iv
                val sealed = enc.doFinal(plain)
                val dec = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, iv)) }
                val roundTrip = MessageDigest.isEqual(dec.doFinal(sealed), plain)
                val tampered = sealed.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }
                val tagRejected = try {
                    Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, iv)) }.doFinal(tampered)
                    false
                } catch (ignored: AEADBadTagException) { true }
                if (roundTrip && tagRejected) Result("aes256_gcm", Status.SUPPORTED, "securityLevel=StrongBox keySize=256, round trip ok, tampered ciphertext rejected")
                else Result("aes256_gcm", Status.FAILED, "roundTrip=$roundTrip tamperRejected=$tagRejected")
            }
            val chain = try { keyStore().getCertificateChain(alias) } catch (ignored: Exception) { null }
            val attest = if (chain.isNullOrEmpty())
                Result("attestation_aes", Status.UNVERIFIABLE, "platform returns no attestation chain for a symmetric key (expected)")
            else Result("attestation_aes", Status.UNVERIFIABLE, "unexpected chain of ${chain.size} for a symmetric key; not evaluated")
            return listOf(gcm, attest)
        } catch (e: Exception) {
            return listOf(failure("aes256_gcm", e, "AES key generation/use"),
                Result("attestation_aes", Status.UNVERIFIABLE, "no AES key was created"))
        } finally {
            deleteAlias(alias)
        }
    }

    fun cleanup(): Result {
        return try {
            val left = keyStore().aliases().toList().filter { it.startsWith(ALIAS_PREFIX) }
            if (left.isEmpty()) Result("temp_keys_deleted", Status.SUPPORTED, "no $ALIAS_PREFIX* aliases remain")
            else {
                left.forEach { deleteAlias(it) }
                Result("temp_keys_deleted", Status.FAILED, "${left.size} alias(es) had to be force-deleted")
            }
        } catch (e: Exception) {
            Result("temp_keys_deleted", Status.UNVERIFIABLE, describe(e))
        }
    }

    private fun overall(results: List<Result>): Result {
        val core = results.filter { it.name == "ecdsa_p256_sign" || it.name == "aes256_gcm" || it.name == "attestation_ec" }
        val status = when {
            core.any { it.status == Status.FAILED } -> Status.FAILED
            core.any { it.status == Status.UNSUPPORTED } -> Status.UNSUPPORTED
            core.any { it.status == Status.UNVERIFIABLE } -> Status.UNVERIFIABLE
            else -> Status.SUPPORTED
        }
        return Result("strongbox_overall", status,
            "from ecdsa_p256_sign+aes256_gcm+attestation_ec; biometric per-use NOT RUN here; no TEE/software fallback attempted")
    }
}
