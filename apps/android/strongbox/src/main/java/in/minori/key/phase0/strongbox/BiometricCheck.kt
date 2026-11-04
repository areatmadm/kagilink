package `in`.minori.key.phase0.strongbox

import android.app.Activity
import android.content.DialogInterface
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.CancellationSignal
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import java.security.InvalidAlgorithmParameterException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.SecretKeyFactory

/**
 * Per-use BIOMETRIC_STRONG check for a StrongBox AES-GCM key. Needs the human's fingerprint; it is
 * never simulated. There is no app-side timeout: the system prompt stays until the user acts or cancels.
 */
internal class BiometricCheck(private val activity: Activity, private val report: (Result) -> Unit) {
    private val alias = StrongBoxChecks.ALIAS_PREFIX + "bio"
    private val executor = activity.mainExecutor

    fun start() {
        val manager = activity.getSystemService(BiometricManager::class.java)
        val can = manager.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG)
        if (can != BiometricManager.BIOMETRIC_SUCCESS) {
            val name = when (can) {
                BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED -> "NONE_ENROLLED"
                BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE -> "NO_HARDWARE"
                BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE -> "HW_UNAVAILABLE"
                else -> "code=$can"
            }
            report(Result("biometric_per_use", Status.UNVERIFIABLE, "canAuthenticate(BIOMETRIC_STRONG)=$name"))
            return
        }
        Thread {
            val key = prepare() // StrongBox key generation can take seconds; keep it off the UI thread
            if (key == null) { finish(); return@Thread }
            activity.runOnUiThread { prompt(key) }
        }.start()
    }

    private fun prepare(): SecretKey? {
        try {
            val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setUserAuthenticationRequired(true)
                .setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
                .setInvalidatedByBiometricEnrollment(true)
                .setIsStrongBoxBacked(true)
                .build()
            val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
            generator.init(spec)
            val key = generator.generateKey()
            val info = SecretKeyFactory.getInstance(key.algorithm, "AndroidKeyStore").getKeySpec(key, KeyInfo::class.java) as KeyInfo
            val problems = mutableListOf<String>()
            if (info.securityLevel != KeyProperties.SECURITY_LEVEL_STRONGBOX) problems += "level=${StrongBoxChecks.levelName(info.securityLevel)}"
            if (!info.isUserAuthenticationRequired) problems += "authNotRequired"
            if (info.userAuthenticationType != KeyProperties.AUTH_BIOMETRIC_STRONG) problems += "authType=${info.userAuthenticationType}"
            if (info.userAuthenticationValidityDurationSeconds != 0) problems += "validity=${info.userAuthenticationValidityDurationSeconds}s"
            if (!info.isInvalidatedByBiometricEnrollment) problems += "notInvalidatedByEnrollment"
            if (problems.isNotEmpty()) {
                report(Result("biometric_key", Status.FAILED, problems.joinToString(" ")))
                return null
            }
            report(Result("biometric_key", Status.SUPPORTED,
                "securityLevel=StrongBox per-use BIOMETRIC_STRONG (validity=0) invalidatedByEnrollment=true"))
            return key
        } catch (e: InvalidAlgorithmParameterException) {
            report(Result("biometric_key", Status.UNVERIFIABLE, "key with per-use biometric auth not creatable: ${StrongBoxChecks.describe(e)}"))
        } catch (e: Exception) {
            report(StrongBoxChecks.failure("biometric_key", e, "biometric key generation"))
        }
        return null
    }

    private fun denied(key: SecretKey): Boolean {
        return try {
            Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key) }.doFinal(StrongBoxChecks.randomBytes(16))
            false
        } catch (e: Exception) {
            true
        }
    }

    private fun prompt(key: SecretKey) {
        try {
            val deniedWithoutAuth = denied(key)
            report(Result("biometric_no_auth_denied", if (deniedWithoutAuth) Status.SUPPORTED else Status.FAILED,
                if (deniedWithoutAuth) "use before authentication was refused" else "key was usable WITHOUT authentication"))
            if (!deniedWithoutAuth) { finish(); return }

            val cipher = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key) }
            var done = false
            val callback = object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    if (done) return
                    done = true
                    try {
                        result.cryptoObject!!.cipher!!.doFinal(StrongBoxChecks.randomBytes(32))
                        val secondRefused = denied(key)
                        report(Result("biometric_per_use", if (secondRefused) Status.SUPPORTED else Status.FAILED,
                            if (secondRefused) "use after fresh BIOMETRIC_STRONG auth ok; immediate second use refused (needs a new prompt)"
                            else "second use succeeded without a new prompt"))
                    } catch (e: Exception) {
                        report(Result("biometric_per_use", Status.FAILED, "use after authentication: ${StrongBoxChecks.describe(e)}"))
                    }
                    finish()
                }

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence?) {
                    if (done) return
                    done = true
                    report(Result("biometric_per_use", Status.UNVERIFIABLE, "authentication not completed (error=$errorCode); no result"))
                    finish()
                }
            }
            val prompt = BiometricPrompt.Builder(activity)
                .setTitle("KagiLink StrongBox Phase 0")
                .setSubtitle("Diagnostic only - synthetic data, nothing stored")
                .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
                .setNegativeButton("Cancel", executor, DialogInterface.OnClickListener { _, _ -> })
                .build()
            prompt.authenticate(BiometricPrompt.CryptoObject(cipher), CancellationSignal(), executor, callback)
        } catch (e: Exception) {
            report(Result("biometric_per_use", Status.FAILED, "prompt setup: ${StrongBoxChecks.describe(e)}"))
            finish()
        }
    }

    private fun finish() {
        StrongBoxChecks.deleteAlias(alias)
        report(StrongBoxChecks.cleanup())
    }
}
