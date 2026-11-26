package dev.area51.android.mfaprovider

import android.app.Activity
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class KeyToWrap {
  lateinit var key: String
}

/**
 * ADR-0016: the vault key, wrapped by an AES-GCM key that lives in the Android Keystore and can
 * only be used right after a strong biometric on that very cipher. A new fingerprint or face
 * invalidates the key; the stored wrap is then deleted and the user types the password.
 */
@TauriPlugin
class KeystorePlugin(private val host: Activity) : Plugin(host) {
  private val strong = BiometricManager.Authenticators.BIOMETRIC_STRONG

  @Command
  fun availability(invoke: Invoke) {
    val ready = BiometricManager.from(host).canAuthenticate(strong) ==
      BiometricManager.BIOMETRIC_SUCCESS
    invoke.resolve(JSObject().put("available", ready))
  }

  @Command
  fun enrol(invoke: Invoke) {
    val key = Base64.decode(invoke.parseArgs(KeyToWrap::class.java).key, Base64.NO_WRAP)
    try {
      deleteWrap()
      val cipher = Cipher.getInstance(TRANSFORMATION)
      cipher.init(Cipher.ENCRYPT_MODE, createKey())
      authenticate(invoke, cipher, "Turn on fingerprint unlock") { proven ->
        val sealed = proven.doFinal(key)
        wrapFile().writeBytes(proven.iv + sealed)
        key.fill(0)
        invoke.resolve()
      }
    } catch (failure: Exception) {
      key.fill(0)
      invoke.reject("could not create the key", "platform", failure)
    }
  }

  @Command
  fun release(invoke: Invoke) {
    val stored = if (wrapFile().exists()) wrapFile().readBytes() else null
    if (stored == null || stored.size <= IV_BYTES) {
      invoke.reject("no key is stored", "invalidated")
      return
    }
    try {
      val cipher = Cipher.getInstance(TRANSFORMATION)
      cipher.init(Cipher.DECRYPT_MODE, existingKey(), GCMParameterSpec(TAG_BITS, stored, 0, IV_BYTES))
      authenticate(invoke, cipher, "Unlock your vault") { proven ->
        val key = proven.doFinal(stored, IV_BYTES, stored.size - IV_BYTES)
        invoke.resolve(JSObject().put("key", Base64.encodeToString(key, Base64.NO_WRAP)))
        key.fill(0)
      }
    } catch (failure: KeyPermanentlyInvalidatedException) {
      discard()
      invoke.reject("the key was invalidated", "invalidated")
    } catch (failure: java.security.UnrecoverableKeyException) {
      discard()
      invoke.reject("the key is gone", "invalidated")
    } catch (failure: Exception) {
      invoke.reject("could not use the key", "platform", failure)
    }
  }

  @Command
  fun remove(invoke: Invoke) {
    discard()
    invoke.resolve()
  }

  private fun authenticate(
    invoke: Invoke,
    cipher: Cipher,
    title: String,
    onProven: (Cipher) -> Unit,
  ) {
    host.runOnUiThread {
      val prompt = BiometricPrompt(
        host as FragmentActivity,
        ContextCompat.getMainExecutor(host),
        object : BiometricPrompt.AuthenticationCallback() {
          override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
            val proven = result.cryptoObject?.cipher
            if (proven == null) {
              invoke.reject("no cipher came back", "platform")
              return
            }
            try {
              onProven(proven)
            } catch (failure: Exception) {
              invoke.reject("could not use the key", "platform", failure)
            }
          }

          override fun onAuthenticationError(code: Int, message: CharSequence) {
            invoke.reject(message.toString(), errorKind(code))
          }
        },
      )
      val info = BiometricPrompt.PromptInfo.Builder()
        .setTitle(title)
        .setNegativeButtonText("Use password")
        .setAllowedAuthenticators(strong)
        .setConfirmationRequired(false)
        .build()
      prompt.authenticate(info, BiometricPrompt.CryptoObject(cipher))
    }
  }

  private fun errorKind(code: Int): String = when (code) {
    BiometricPrompt.ERROR_USER_CANCELED,
    BiometricPrompt.ERROR_NEGATIVE_BUTTON,
    BiometricPrompt.ERROR_CANCELED -> "cancelled"
    BiometricPrompt.ERROR_LOCKOUT,
    BiometricPrompt.ERROR_LOCKOUT_PERMANENT -> "failed"
    BiometricPrompt.ERROR_NO_BIOMETRICS,
    BiometricPrompt.ERROR_HW_NOT_PRESENT,
    BiometricPrompt.ERROR_HW_UNAVAILABLE -> "unavailable"
    else -> "platform"
  }

  private fun createKey(): SecretKey {
    val spec = KeyGenParameterSpec.Builder(
      KEY_ALIAS,
      KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
    )
      .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
      .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
      .setKeySize(256)
      .setUserAuthenticationRequired(true)
      .setInvalidatedByBiometricEnrollment(true)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      spec.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
    }
    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
    generator.init(spec.build())
    return generator.generateKey()
  }

  private fun existingKey(): SecretKey {
    val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
    return store.getKey(KEY_ALIAS, null) as? SecretKey
      ?: throw java.security.UnrecoverableKeyException("no key")
  }

  private fun discard() {
    deleteWrap()
    val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
    if (store.containsAlias(KEY_ALIAS)) store.deleteEntry(KEY_ALIAS)
  }

  private fun deleteWrap() {
    wrapFile().delete()
  }

  private fun wrapFile() = File(host.filesDir, WRAP_FILE)

  private companion object {
    const val KEYSTORE = "AndroidKeyStore"
    const val KEY_ALIAS = "mfa_provider_vault_key"
    const val WRAP_FILE = "biometric.wrap"
    const val TRANSFORMATION = "AES/GCM/NoPadding"
    const val IV_BYTES = 12
    const val TAG_BITS = 128
  }
}
