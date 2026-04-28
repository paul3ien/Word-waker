//! Sélection et identification du device audio d'entrée par défaut via CoreAudio.

use crate::audio_capture::config::AudioCaptureConfig;
use crate::audio_capture::error::AudioCaptureError;
use crate::audio_capture::ffi::{
    kAudioDevicePropertyNominalSampleRate,
    kAudioHardwarePropertyDefaultInputDevice, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, noErr,
    AudioObjectGetPropertyData, AudioObjectID, AudioObjectPropertyAddress,
};
use libc::c_void;

/// Retourne l'identifiant du device d'entrée audio par défaut.
///
/// Retourne `Err(AudioCaptureError::DeviceNotFound)` si aucun device n'est
/// disponible ou si CoreAudio retourne une erreur.
pub fn get_default_input_device() -> Result<AudioObjectID, AudioCaptureError> {
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    let mut device_id: AudioObjectID = 0;
    let mut data_size = std::mem::size_of::<AudioObjectID>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            std::ptr::null(),
            &mut data_size,
            &mut device_id as *mut AudioObjectID as *mut c_void,
        )
    };

    if status != noErr || device_id == 0 {
        return Err(AudioCaptureError::DeviceNotFound);
    }

    Ok(device_id)
}

/// Retourne le nom du device audio pour les logs.
///
/// En cas d'échec, retourne une chaîne de substitution lisible.
pub fn device_name(_device_id: AudioObjectID) -> String {
    // La récupération du nom via CoreAudio requiert des types CFString (CoreFoundation).
    // Pour éviter une dépendance vers CoreFoundation à ce stade, on retourne
    // une description synthétique suffisante pour les logs.
    format!("AudioDevice(id={})", _device_id)
}

/// Vérifie que le device audio est joignable et supporte la capture PCM.
///
/// Sur Apple Silicon, tout device d'entrée valide retourné par CoreAudio
/// supporte Float32 via l'AudioUnit AUHAL (qui gère la conversion de format
/// et de fréquence d'échantillonnage). On vérifie simplement que le device
/// répond à `kAudioDevicePropertyNominalSampleRate`.
pub fn check_format_support(
    device_id: AudioObjectID,
    _config: &AudioCaptureConfig,
) -> Result<(), AudioCaptureError> {
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyNominalSampleRate,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    let mut sample_rate: f64 = 0.0;
    let mut data_size = std::mem::size_of::<f64>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            &mut data_size,
            &mut sample_rate as *mut f64 as *mut c_void,
        )
    };

    if status != noErr || sample_rate <= 0.0 {
        return Err(AudioCaptureError::FormatUnsupported);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_capture::ffi::{
        kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
        kAudioFormatLinearPCM, AudioStreamBasicDescription,
    };

    /// Valide un ASBD synthétique : FormatID == LinearPCM et ChannelsPerFrame > 0.
    /// Le flag IsFloat n'est pas exigé (format physique peut être entier).
    fn validate_asbd(asbd: &AudioStreamBasicDescription) -> Result<(), AudioCaptureError> {
        if asbd.mFormatID != kAudioFormatLinearPCM {
            return Err(AudioCaptureError::FormatUnsupported);
        }
        if asbd.mChannelsPerFrame == 0 {
            return Err(AudioCaptureError::FormatUnsupported);
        }
        Ok(())
}

    /// Test d'intégration réel — nécessite un microphone système.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn get_default_input_device_retourne_id_non_nul() {
        let result = get_default_input_device();
        assert!(result.is_ok(), "Aucun device d'entrée trouvé : {:?}", result);
        assert_ne!(result.unwrap(), 0, "L'ID du device ne doit pas être 0");
    }

    /// Test mock — vérifie que DeviceNotFound est bien formattable.
    #[cfg(feature = "mock_audio")]
    #[test]
    fn mock_device_not_found_retourne_erreur() {
        let err = AudioCaptureError::DeviceNotFound;
        assert!(!err.to_string().is_empty());
        assert!(matches!(err, AudioCaptureError::DeviceNotFound));
    }

    #[test]
    fn device_name_retourne_chaine_non_vide() {
        let name = device_name(42);
        assert!(!name.is_empty());
        assert!(name.contains("42"));
    }

    /// Test d'intégration réel — nécessite un microphone système.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn check_format_support_ok_sur_device_reel() {
        let device_id = get_default_input_device()
            .expect("Microphone requis pour ce test");
        let config = crate::audio_capture::config::AudioCaptureConfig::default();
        let result = check_format_support(device_id, &config);
        assert!(result.is_ok(), "Format non supporté : {:?}", result);
    }

    /// Test mock — vérifie qu'un ASBD avec un FormatID non-PCM est rejeté.
    #[test]
    fn validate_asbd_rejette_format_non_pcm() {
        let asbd = AudioStreamBasicDescription {
            mSampleRate: 44100.0,
            mFormatID: 0xDEAD_BEEF, // format inconnu → rejet attendu
            mFormatFlags: kAudioFormatFlagIsFloat,
            mBytesPerPacket: 4,
            mFramesPerPacket: 1,
            mBytesPerFrame: 4,
            mChannelsPerFrame: 1,
            mBitsPerChannel: 32,
            mReserved: 0,
        };
        assert!(matches!(
            validate_asbd(&asbd),
            Err(AudioCaptureError::FormatUnsupported)
        ));
    }

    /// Test mock — vérifie qu'un ASBD PCM valide (même entier) est accepté.
    #[test]
    fn validate_asbd_accepte_format_float32_mono() {
        let asbd = AudioStreamBasicDescription {
            mSampleRate: 44100.0,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked | kAudioFormatFlagIsNonInterleaved,
            mBytesPerPacket: 4,
            mFramesPerPacket: 1,
            mBytesPerFrame: 4,
            mChannelsPerFrame: 1,
            mBitsPerChannel: 32,
            mReserved: 0,
        };
        assert!(validate_asbd(&asbd).is_ok());
    }

    /// Test mock — vérifie qu'un ASBD avec 0 canal est rejeté.
    #[test]
    fn validate_asbd_rejette_zero_canaux() {
        let asbd = AudioStreamBasicDescription {
            mSampleRate: 44100.0,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat,
            mBytesPerPacket: 0,
            mFramesPerPacket: 1,
            mBytesPerFrame: 0,
            mChannelsPerFrame: 0, // → rejet attendu
            mBitsPerChannel: 32,
            mReserved: 0,
        };
        assert!(matches!(
            validate_asbd(&asbd),
            Err(AudioCaptureError::FormatUnsupported)
        ));
    }
}
