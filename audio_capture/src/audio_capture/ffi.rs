 //! Bindings FFI vers les API C de CoreAudio et AudioToolbox.
//!
//! Tous les types sont définis manuellement pour éviter toute dépendance
//! vers des crates de bindings tiers — conformément aux contraintes du stack.

#![allow(non_camel_case_types, non_snake_case, dead_code, non_upper_case_globals)]

use libc::c_void;

// ---------------------------------------------------------------------------
// Types de base CoreAudio
// ---------------------------------------------------------------------------

/// Code de retour CoreAudio (0 = succès).
pub type OSStatus = i32;

/// Identifiant opaque d'un objet audio (device, stream, …).
pub type AudioObjectID = u32;

/// Sélecteur de propriété sur un `AudioObjectID`.
#[repr(C)]
pub struct AudioObjectPropertyAddress {
    pub mSelector: u32,
    pub mScope: u32,
    pub mElement: u32,
}

/// Description d'un format de stream audio (ASBD).
#[repr(C)]
pub struct AudioStreamBasicDescription {
    pub mSampleRate: f64,
    pub mFormatID: u32,
    pub mFormatFlags: u32,
    pub mBytesPerPacket: u32,
    pub mFramesPerPacket: u32,
    pub mBytesPerFrame: u32,
    pub mChannelsPerFrame: u32,
    pub mBitsPerChannel: u32,
    pub mReserved: u32,
}

/// Un buffer audio dans une `AudioBufferList`.
#[repr(C)]
pub struct AudioBuffer {
    pub mNumberChannels: u32,
    pub mDataByteSize: u32,
    pub mData: *mut c_void,
}

/// Liste de buffers audio passée au callback RT.
///
/// En pratique pour une configuration mono non-entrelacée, `mNumberBuffers == 1`.
#[repr(C)]
pub struct AudioBufferList {
    pub mNumberBuffers: u32,
    /// Tableau de longueur variable — accès uniquement via pointeur.
    pub mBuffers: [AudioBuffer; 1],
}

// ---------------------------------------------------------------------------
// Constantes CoreAudio (AudioObject)
// ---------------------------------------------------------------------------

/// Objet système global (point d'entrée pour les requêtes matérielles).
pub const kAudioObjectSystemObject: AudioObjectID = 1;

/// Scope d'entrée (microphone).
pub const kAudioObjectPropertyScopeInput: u32 = 0x696E7075; // 'inpu'

/// Scope global (propriétés non directionnelles).
pub const kAudioObjectPropertyScopeGlobal: u32 = 0x676C6F62; // 'glob'

/// Élément principal (master).
pub const kAudioObjectPropertyElementMain: u32 = 0;

/// Sélecteur : device d'entrée par défaut.
pub const kAudioHardwarePropertyDefaultInputDevice: u32 = 0x64496E20; // 'dIn '

/// Sélecteur : nom du device.
pub const kAudioObjectPropertyName: u32 = 0x6C4E616D; // 'lNam'

/// Sélecteur : format du stream courant d'un device (scope Input = entrée micro).
pub const kAudioDevicePropertyStreamFormat: u32 = 0x73666D74; // 'sfmt'

/// Sélecteur : fréquence d'échantillonnage nominale courante du device.
pub const kAudioDevicePropertyNominalSampleRate: u32 = 0x6E737274; // 'nsrt'

// ---------------------------------------------------------------------------
// Constantes de format audio (AudioToolbox)
// ---------------------------------------------------------------------------

/// Format PCM linéaire.
pub const kAudioFormatLinearPCM: u32 = 0x6C70636D; // 'lpcm'

/// Flag : échantillons en virgule flottante.
pub const kAudioFormatFlagIsFloat: u32 = 1 << 0;

/// Flag : ordre natif (little-endian sur Apple Silicon).
pub const kAudioFormatFlagIsBigEndian: u32 = 1 << 1;

/// Flag : valeurs signées.
pub const kAudioFormatFlagIsSignedInteger: u32 = 1 << 2;

/// Flag : données empaquetées (pas de padding).
pub const kAudioFormatFlagIsPacked: u32 = 1 << 3;

/// Flag : buffers non entrelacés (un buffer par canal).
pub const kAudioFormatFlagIsNonInterleaved: u32 = 1 << 5;

/// Code de succès CoreAudio.
pub const noErr: OSStatus = 0;

// ---------------------------------------------------------------------------
// Fonctions FFI extern "C"
// ---------------------------------------------------------------------------

extern "C" {
    /// Lit la valeur d'une propriété d'un objet audio.
    pub fn AudioObjectGetPropertyData(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        ioDataSize: *mut u32,
        outData: *mut c_void,
    ) -> OSStatus;

    /// Écrit la valeur d'une propriété d'un objet audio.
    pub fn AudioObjectSetPropertyData(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        inDataSize: u32,
        inData: *const c_void,
    ) -> OSStatus;

    /// Retourne la taille (en octets) de la valeur d'une propriété.
    pub fn AudioObjectGetPropertyDataSize(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        outDataSize: *mut u32,
    ) -> OSStatus;
}

// ---------------------------------------------------------------------------
// Tests unitaires
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn no_err_est_zero() {
        assert_eq!(noErr, 0);
    }

    #[test]
    fn taille_audio_object_property_address() {
        // 3 × u32 = 12 octets
        assert_eq!(size_of::<AudioObjectPropertyAddress>(), 12);
    }

    #[test]
    fn taille_audio_stream_basic_description() {
        // f64 (8) + 8 × u32 (32) = 40 octets
        assert_eq!(size_of::<AudioStreamBasicDescription>(), 40);
    }

    #[test]
    fn taille_audio_buffer() {
        // 2 × u32 (8) + *mut c_void (8 sur 64-bit) = 16 octets
        assert_eq!(size_of::<AudioBuffer>(), 16);
    }
}
