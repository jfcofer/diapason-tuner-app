package dev.jfcofer.diapason.core_platform

import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioManager
import io.flutter.embedding.engine.plugins.FlutterPlugin

/**
 * The Android side of `core_platform`: answers the audio capabilities channel (docs/adr/0024).
 *
 * It only reports what the platform says. Choosing an input preset from the answers is policy and
 * lives in Rust (`diapason_session::choose_input_preset`).
 */
class CorePlatformPlugin : FlutterPlugin {
    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        AudioCapabilitiesApi.setUp(
            binding.binaryMessenger,
            AudioCapabilities(binding.applicationContext),
        )
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        AudioCapabilitiesApi.setUp(binding.binaryMessenger, null)
    }
}

/** Reads the device's audio properties. Each answer is null when the platform gives none. */
private class AudioCapabilities(private val context: Context) : AudioCapabilitiesApi {
    override fun read(): AudioCapabilitiesMessage {
        val audio = context.getSystemService(AudioManager::class.java)
        val packages = context.packageManager
        return AudioCapabilitiesMessage(
            unprocessedSource =
                audio
                    ?.getProperty(AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED)
                    ?.toBoolean(),
            lowLatency = packages.hasSystemFeature(PackageManager.FEATURE_AUDIO_LOW_LATENCY),
            proAudio = packages.hasSystemFeature(PackageManager.FEATURE_AUDIO_PRO),
            nativeSampleRate =
                audio?.getProperty(AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)?.toLongOrNull(),
            nativeFramesPerBuffer =
                audio?.getProperty(AudioManager.PROPERTY_OUTPUT_FRAMES_PER_BUFFER)?.toLongOrNull(),
        )
    }
}
