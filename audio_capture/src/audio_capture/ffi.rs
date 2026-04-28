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
// Types AudioUnit / AudioComponent
// ---------------------------------------------------------------------------

/// Instance opaque d'un AudioUnit (pointeur C, nullable).
pub type AudioUnit = *mut c_void;

/// Référence opaque vers un AudioComponent.
pub type AudioComponent = *mut c_void;

/// Description permettant de rechercher un composant audio.
#[repr(C)]
pub struct AudioComponentDescription {
    pub componentType: u32,
    pub componentSubType: u32,
    pub componentManufacturer: u32,
    pub componentFlags: u32,
    pub componentFlagsMask: u32,
}

/// Flags d'action passés au callback de rendu.
pub type AudioUnitRenderActionFlags = u32;

/// Timestamp CoreAudio (seul `mSampleTime` nous est utile ; les autres
/// champs sont réservés ou liés à l'horloge hôte).
#[repr(C)]
pub struct AudioTimeStamp {
    pub mSampleTime: f64,
    pub mHostTime: u64,
    pub mRateScalar: f64,
    pub mWordClockTime: u64,
    /// SMPTETime (80 octets) — padding pour respecter l'ABI Apple.
    pub mSMPTETime: [u8; 24],
    pub mFlags: u32,
    pub mReserved: u32,
}

/// Callback de rendu enregistré sur l'AudioUnit d'entrée.
#[repr(C)]
pub struct AURenderCallbackStruct {
    pub inputProc: Option<
        unsafe extern "C" fn(
            inRefCon: *mut c_void,
            ioActionFlags: *mut AudioUnitRenderActionFlags,
            inTimeStamp: *const AudioTimeStamp,
            inBusNumber: u32,
            inNumberFrames: u32,
            ioData: *mut AudioBufferList,
        ) -> OSStatus,
    >,
    pub inputProcRefCon: *mut c_void,
}

// ---------------------------------------------------------------------------
// Constantes AudioUnit / AudioComponent
// ---------------------------------------------------------------------------

/// Type : Output AudioUnit (AUHAL pour capture).
pub const kAudioUnitType_Output: u32 = 0x61756F75; // 'auou'

/// Sous-type : HAL Output (accès direct au hardware).
pub const kAudioUnitSubType_HALOutput: u32 = 0x6168616C; // 'ahal'

/// Fabricant Apple.
pub const kAudioUnitManufacturer_Apple: u32 = 0x6170706C; // 'appl'

/// Propriété : activer/désactiver l'IO d'entrée ou de sortie.
pub const kAudioOutputUnitProperty_EnableIO: u32 = 2003;

/// Propriété : sélectionner le device CoreAudio à utiliser.
pub const kAudioOutputUnitProperty_CurrentDevice: u32 = 2000;

/// Propriété : format du stream (ASBD) sur un bus donné.
pub const kAudioUnitProperty_StreamFormat: u32 = 8;

/// Propriété : callback de rendu/capture.
pub const kAudioUnitProperty_SetRenderCallback: u32 = 23;

/// Scope : entrée.
pub const kAudioUnitScope_Input: u32 = 1;

/// Scope : sortie.
pub const kAudioUnitScope_Output: u32 = 2;

/// Scope : global (propriétés non directionnelles de l'AudioUnit).
pub const kAudioUnitScope_Global: u32 = 0;

/// Propriété : callback d'entrée pour l'AudioUnit AUHAL (capture micro).
pub const kAudioOutputUnitProperty_SetInputCallback: u32 = 2005;

// ---------------------------------------------------------------------------
// Fonctions FFI AudioComponent / AudioUnit
// ---------------------------------------------------------------------------

extern "C" {
    /// Cherche un composant audio correspondant à la description.
    pub fn AudioComponentFindNext(
        inComponent: AudioComponent,
        inDesc: *const AudioComponentDescription,
    ) -> AudioComponent;

    /// Instancie un AudioUnit à partir d'un AudioComponent.
    pub fn AudioComponentInstanceNew(
        inComponent: AudioComponent,
        outInstance: *mut AudioUnit,
    ) -> OSStatus;

    /// Libère une instance d'AudioUnit.
    pub fn AudioComponentInstanceDispose(inInstance: AudioUnit) -> OSStatus;

    /// Initialise l'AudioUnit (alloue les ressources DSP).
    pub fn AudioUnitInitialize(inUnit: AudioUnit) -> OSStatus;

    /// Libère les ressources DSP de l'AudioUnit.
    pub fn AudioUnitUninitialize(inUnit: AudioUnit) -> OSStatus;

    /// Démarre le flux audio (start du callback RT).
    pub fn AudioOutputUnitStart(ci: AudioUnit) -> OSStatus;

    /// Arrête le flux audio.
    pub fn AudioOutputUnitStop(ci: AudioUnit) -> OSStatus;

    /// Configure une propriété sur l'AudioUnit.
    pub fn AudioUnitSetProperty(
        inUnit: AudioUnit,
        inID: u32,
        inScope: u32,
        inElement: u32,
        inData: *const c_void,
        inDataSize: u32,
    ) -> OSStatus;

    /// Lit une propriété de l'AudioUnit.
    pub fn AudioUnitGetProperty(
        inUnit: AudioUnit,
        inID: u32,
        inScope: u32,
        inElement: u32,
        outData: *mut c_void,
        ioDataSize: *mut u32,
    ) -> OSStatus;

    /// Rend (tire) des données audio depuis l'AudioUnit — utilisé dans le callback
    /// d'entrée pour récupérer les échantillons PCM depuis le hardware.
    pub fn AudioUnitRender(
        inUnit: AudioUnit,
        ioActionFlags: *mut AudioUnitRenderActionFlags,
        inTimeStamp: *const AudioTimeStamp,
        inOutputBusNumber: u32,
        inNumberFrames: u32,
        ioData: *mut AudioBufferList,
    ) -> OSStatus;
}

// ---------------------------------------------------------------------------
// API CoreAudio HAL directe (AudioDeviceIOProc)
// Plus simple pour la capture entrée seule : données livrées directement
// dans le callback sans passer par le mécanisme de rendu AUHAL.
// ---------------------------------------------------------------------------

/// Identifiant opaque d'un IOProc enregistré sur un device CoreAudio.
pub type AudioDeviceIOProcID = *mut c_void;

extern "C" {
    /// Enregistre un callback IO sur un device CoreAudio.
    pub fn AudioDeviceCreateIOProcID(
        inDevice: AudioObjectID,
        inProc: unsafe extern "C" fn(
            AudioObjectID,
            *const AudioTimeStamp,
            *const AudioBufferList,
            *const AudioTimeStamp,
            *mut AudioBufferList,
            *const AudioTimeStamp,
            *mut c_void,
        ) -> OSStatus,
        inClientData: *mut c_void,
        outIOProcID: *mut AudioDeviceIOProcID,
    ) -> OSStatus;

    /// Détruit un IOProc enregistré sur un device CoreAudio.
    pub fn AudioDeviceDestroyIOProcID(
        inDevice: AudioObjectID,
        inIOProcID: AudioDeviceIOProcID,
    ) -> OSStatus;

    /// Démarre le device (active le callback).
    pub fn AudioDeviceStart(
        inDevice: AudioObjectID,
        inProcID: AudioDeviceIOProcID,
    ) -> OSStatus;

    /// Arrête le device (désactive le callback).
    pub fn AudioDeviceStop(
        inDevice: AudioObjectID,
        inProcID: AudioDeviceIOProcID,
    ) -> OSStatus;
}

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

    #[test]
    fn taille_audio_component_description() {
        // 5 × u32 = 20 octets
        assert_eq!(size_of::<AudioComponentDescription>(), 20);
    }
}
