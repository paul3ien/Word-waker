//! Construction et capture audio via AudioDeviceCreateIOProcID (CoreAudio HAL direct).
//!
//! Le callback HAL est appelé par CoreAudio avec les données d'entrée déjà
//! présentes dans `inInputData` — aucune ré-entrance, aucune allocation.

use crate::audio_capture::config::AudioCaptureConfig;
use crate::audio_capture::error::AudioCaptureError;
use crate::audio_capture::ffi::{
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM, kAudioOutputUnitProperty_CurrentDevice,
    kAudioOutputUnitProperty_EnableIO, kAudioUnitManufacturer_Apple,
    kAudioUnitProperty_StreamFormat, kAudioUnitScope_Global, kAudioUnitScope_Input,
    kAudioUnitScope_Output, kAudioUnitSubType_HALOutput, kAudioUnitType_Output, noErr,
    AudioBufferList, AudioComponentDescription, AudioComponentFindNext,
    AudioComponentInstanceDispose, AudioComponentInstanceNew, AudioDeviceCreateIOProcID,
    AudioDeviceDestroyIOProcID, AudioDeviceIOProcID, AudioDeviceStart, AudioDeviceStop,
    AudioObjectID, AudioOutputUnitStop, AudioStreamBasicDescription, AudioTimeStamp, AudioUnit,
    AudioUnitInitialize, AudioUnitSetProperty, AudioUnitUninitialize,
};
use crate::audio_capture::ring_buffer::push_sample;
use crossbeam::queue::ArrayQueue;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Contexte RT — passé via inClientData au callback AudioDeviceIOProc.
// CRITIQUE : aucune allocation, aucun lock.
// ---------------------------------------------------------------------------

struct CallbackContext {
    producer: Arc<ArrayQueue<f32>>,
    dropped: Arc<AtomicUsize>,
    /// Compteur de déclenchements du callback.
    fires: Arc<AtomicUsize>,
}

// SAFETY : le callback RT CoreAudio s'exécute sur un thread géré par macOS ;
// accès séquentiel garanti (un seul callback actif à la fois).
unsafe impl Send for CallbackContext {}
unsafe impl Sync for CallbackContext {}

// ---------------------------------------------------------------------------
// Callback AudioDeviceIOProc — données INPUT directement dans inInputData.
// Aucun besoin d'AudioUnitRender, pas de ré-entrance.
// ---------------------------------------------------------------------------

unsafe extern "C" fn device_io_callback(
    _device: AudioObjectID,
    _now: *const AudioTimeStamp,
    in_input_data: *const AudioBufferList,
    _input_time: *const AudioTimeStamp,
    _out_output_data: *mut AudioBufferList,
    _output_time: *const AudioTimeStamp,
    client_data: *mut libc::c_void,
) -> i32 {
    let ctx = &mut *(client_data as *mut CallbackContext);
    ctx.fires.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    if in_input_data.is_null() {
        return noErr;
    }
    let n_bufs = (*in_input_data).mNumberBuffers as usize;
    if n_bufs == 0 {
        return noErr;
    }
    let buffer = &(*in_input_data).mBuffers[0];
    if buffer.mData.is_null() || buffer.mDataByteSize == 0 {
        return noErr;
    }

    // Les données sont en Float32 (format Mix CoreAudio par défaut).
    let n = buffer.mDataByteSize as usize / mem::size_of::<f32>();
    let samples = std::slice::from_raw_parts(buffer.mData as *const f32, n);
    for &s in samples {
        push_sample(&ctx.producer, &ctx.dropped, s);
    }
    noErr
}

// ---------------------------------------------------------------------------
// Struct principale
// ---------------------------------------------------------------------------

/// Encapsule la capture microphone via AudioDeviceCreateIOProcID.
/// L'AUHAL est conservé uniquement pour la validation du device et la
/// configuration du format lors de `new()`.
#[allow(dead_code)]
pub struct AudioUnitCapture {
    /// AUHAL utilisé pour la validation et la configuration initiale.
    pub(crate) unit: AudioUnit,
    /// Device d'entrée sélectionné.
    device_id: AudioObjectID,
    /// IOProcID enregistré sur le device (null jusqu'à `register_input_callback`).
    proc_id: AudioDeviceIOProcID,
    /// Contexte partagé avec le callback (null jusqu'à `register_input_callback`).
    ctx: *mut CallbackContext,
    /// Indique si le device a été démarré (pour l'idempotence de `stop`).
    started: AtomicBool,
    /// Compteur debug de déclenchements.
    dbg_fires: Option<Arc<AtomicUsize>>,
    dbg_last_render_err: Option<Arc<AtomicUsize>>,
}

// SAFETY : unit, proc_id et ctx sont accédés séquentiellement.
unsafe impl Send for AudioUnitCapture {}
unsafe impl Sync for AudioUnitCapture {}

impl Drop for AudioUnitCapture {
    fn drop(&mut self) {
        if !self.proc_id.is_null() {
            unsafe {
                AudioDeviceStop(self.device_id, self.proc_id);
                AudioDeviceDestroyIOProcID(self.device_id, self.proc_id);
            }
        }
        unsafe {
            AudioOutputUnitStop(self.unit);
            AudioUnitUninitialize(self.unit);
            AudioComponentInstanceDispose(self.unit);
        }
        if !self.ctx.is_null() {
            unsafe { drop(Box::from_raw(self.ctx)) };
        }
    }
}

impl AudioUnitCapture {
    /// Crée et valide l'AudioUnit AUHAL pour le device `device_id`.
    ///
    /// Initialise l'AUHAL avec le format Float32 mono pour validation.
    /// La capture réelle utilise `AudioDeviceCreateIOProcID` (plus fiable).
    pub fn new(
        device_id: AudioObjectID,
        config: &AudioCaptureConfig,
    ) -> Result<Self, AudioCaptureError> {
        // 1. Trouver le composant AUHAL.
        let desc = AudioComponentDescription {
            componentType: kAudioUnitType_Output,
            componentSubType: kAudioUnitSubType_HALOutput,
            componentManufacturer: kAudioUnitManufacturer_Apple,
            componentFlags: 0,
            componentFlagsMask: 0,
        };
        let component = unsafe { AudioComponentFindNext(ptr::null_mut(), &desc) };
        if component.is_null() {
            return Err(AudioCaptureError::UnitCreationFailed(0));
        }

        // 2. Instancier l'AudioUnit.
        let mut unit: AudioUnit = ptr::null_mut();
        let status = unsafe { AudioComponentInstanceNew(component, &mut unit) };
        if status != noErr {
            return Err(AudioCaptureError::UnitCreationFailed(status));
        }

        // 3. Activer l'IO entrée (scope Input, element 1).
        let enable: u32 = 1;
        let status = unsafe {
            AudioUnitSetProperty(
                unit,
                kAudioOutputUnitProperty_EnableIO,
                kAudioUnitScope_Input,
                1,
                &enable as *const u32 as *const libc::c_void,
                mem::size_of::<u32>() as u32,
            )
        };
        if status != noErr {
            unsafe { AudioComponentInstanceDispose(unit) };
            return Err(AudioCaptureError::PropertySetFailed(status));
        }

        // 4. Désactiver l'IO sortie (scope Output, element 0).
        let disable: u32 = 0;
        let status = unsafe {
            AudioUnitSetProperty(
                unit,
                kAudioOutputUnitProperty_EnableIO,
                kAudioUnitScope_Output,
                0,
                &disable as *const u32 as *const libc::c_void,
                mem::size_of::<u32>() as u32,
            )
        };
        if status != noErr {
            unsafe { AudioComponentInstanceDispose(unit) };
            return Err(AudioCaptureError::PropertySetFailed(status));
        }

        // 5. Sélectionner le device CoreAudio.
        let status = unsafe {
            AudioUnitSetProperty(
                unit,
                kAudioOutputUnitProperty_CurrentDevice,
                kAudioUnitScope_Global,
                0,
                &device_id as *const AudioObjectID as *const libc::c_void,
                mem::size_of::<AudioObjectID>() as u32,
            )
        };
        if status != noErr {
            unsafe { AudioComponentInstanceDispose(unit) };
            return Err(AudioCaptureError::PropertySetFailed(status));
        }

        // 6. Configurer le format Float32 mono (scope Output, element 1).
        let asbd = AudioStreamBasicDescription {
            mSampleRate: config.sample_rate,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat
                | kAudioFormatFlagIsPacked
                | kAudioFormatFlagIsNonInterleaved,
            mBytesPerPacket: 4,
            mFramesPerPacket: 1,
            mBytesPerFrame: 4,
            mChannelsPerFrame: 1,
            mBitsPerChannel: 32,
            mReserved: 0,
        };
        let status = unsafe {
            AudioUnitSetProperty(
                unit,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Output,
                1,
                &asbd as *const AudioStreamBasicDescription as *const libc::c_void,
                mem::size_of::<AudioStreamBasicDescription>() as u32,
            )
        };
        if status != noErr {
            unsafe { AudioComponentInstanceDispose(unit) };
            return Err(AudioCaptureError::PropertySetFailed(status));
        }

        // 7. Initialiser l'AudioUnit (valide la configuration).
        let status = unsafe { AudioUnitInitialize(unit) };
        if status != noErr {
            unsafe { AudioComponentInstanceDispose(unit) };
            return Err(AudioCaptureError::UnitCreationFailed(status));
        }

        Ok(Self {
            unit,
            device_id,
            proc_id: ptr::null_mut(),
            ctx: ptr::null_mut(),
            started: AtomicBool::new(false),
            dbg_fires: None,
            dbg_last_render_err: None,
        })
    }

    /// Enregistre le callback de capture HAL via `AudioDeviceCreateIOProcID`.
    ///
    /// Les données PCM arrivent directement dans le callback (pas d'AudioUnitRender).
    /// Aucune allocation dans le callback — toutes les ressources sont pré-allouées ici.
    pub fn register_input_callback(
        &mut self,
        producer: Arc<ArrayQueue<f32>>,
        dropped: Arc<AtomicUsize>,
        _buffer_capacity: usize,
    ) -> Result<(), AudioCaptureError> {
        let fires = Arc::new(AtomicUsize::new(0));
        let last_render_err = Arc::new(AtomicUsize::new(0));
        let ctx = Box::new(CallbackContext {
            producer,
            dropped,
            fires: Arc::clone(&fires),
        });
        let ctx_ptr = Box::into_raw(ctx);

        let mut proc_id: AudioDeviceIOProcID = ptr::null_mut();
        let status = unsafe {
            AudioDeviceCreateIOProcID(
                self.device_id,
                device_io_callback,
                ctx_ptr as *mut libc::c_void,
                &mut proc_id,
            )
        };
        if status != noErr {
            unsafe { drop(Box::from_raw(ctx_ptr)) };
            return Err(AudioCaptureError::PropertySetFailed(status));
        }

        // Libérer l'éventuel contexte précédent.
        if !self.proc_id.is_null() {
            unsafe {
                AudioDeviceStop(self.device_id, self.proc_id);
                AudioDeviceDestroyIOProcID(self.device_id, self.proc_id);
            }
        }
        if !self.ctx.is_null() {
            unsafe { drop(Box::from_raw(self.ctx)) };
        }

        self.proc_id = proc_id;
        self.ctx = ctx_ptr;
        self.dbg_fires = Some(fires);
        self.dbg_last_render_err = Some(last_render_err);
        Ok(())
    }

    /// Démarre la capture (active le callback IO).
    /// Idempotent : un deuxième appel sans stop intermédiaire est un no-op.
    pub fn start(&self) -> Result<(), AudioCaptureError> {
        if self.started.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let status = unsafe { AudioDeviceStart(self.device_id, self.proc_id) };
        if status != noErr {
            self.started.store(false, Ordering::SeqCst);
            return Err(AudioCaptureError::UnitStartFailed(status));
        }
        Ok(())
    }

    /// Arrête la capture (désactive le callback IO).
    /// Idempotent : appeler `stop()` plusieurs fois est sûr et sans effet supplémentaire.
    pub fn stop(&self) -> Result<(), AudioCaptureError> {
        if !self.started.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        let status = unsafe { AudioDeviceStop(self.device_id, self.proc_id) };
        if status != noErr {
            return Err(AudioCaptureError::UnitStopFailed(status));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_capture::{
        config::AudioCaptureConfig, device::get_default_input_device, error::AudioCaptureError,
        ring_buffer::AudioRingBuffer,
    };

    /// Crée l'AudioUnit sans erreur sur le device d'entrée par défaut.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn audio_unit_capture_new_ok_sur_device_reel() {
        let device_id = get_default_input_device().expect("Pas de device d'entrée");
        let config = AudioCaptureConfig::default();
        let result = AudioUnitCapture::new(device_id, &config);
        assert!(
            result.is_ok(),
            "AudioUnitCapture::new a échoué : {:?}",
            result.err()
        );
    }

    /// Un device invalide (0xFFFFFFFF) doit retourner PropertySetFailed ou UnitCreationFailed.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn audio_unit_capture_new_echec_device_invalide() {
        let config = AudioCaptureConfig::default();
        let result = AudioUnitCapture::new(0xFFFF_FFFF, &config);
        assert!(
            result.is_err(),
            "Attendu une erreur pour un device invalide"
        );
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("Attendu une erreur"),
        };
        match err {
            AudioCaptureError::PropertySetFailed(_) | AudioCaptureError::UnitCreationFailed(_) => {}
            e => panic!("Type d'erreur inattendu : {e}"),
        }
    }

    /// Enregistre le callback, démarre 100 ms, vérifie que des samples ont été reçus.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn register_callback_capture_samples_apres_100ms() {
        use std::time::Duration;

        let device_id = get_default_input_device().expect("Pas de device d'entrée");
        let config = AudioCaptureConfig::default();
        let ring = AudioRingBuffer::new(config.ring_capacity);
        let producer = ring.producer_handle();
        let dropped = ring.dropped_samples_handle();
        let consumer = ring.consumer_handle();

        let mut unit = AudioUnitCapture::new(device_id, &config).expect("new failed");
        unit.register_input_callback(producer, dropped, config.buffer_size_frames as usize)
            .expect("register_input_callback failed");
        unit.start().expect("start failed");

        std::thread::sleep(Duration::from_millis(200));

        unit.stop().expect("stop failed");

        let fires = unit
            .dbg_fires
            .as_ref()
            .map(|a| a.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(0);
        eprintln!("[DEBUG] callback fires={fires}");

        let mut count = 0usize;
        while consumer.pop().is_some() {
            count += 1;
        }
        assert!(
            count > 0,
            "Aucun sample capturé après 200 ms (fires={fires})"
        );
    }

    /// Vérifie que `AudioCaptureError` est Send + Sync (invariant existant).
    #[test]
    fn audio_unit_capture_est_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AudioCaptureError>();
    }

    /// Start → 200 ms → Stop : Ok aux deux étapes, ring buffer non vide.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn start_stop_ok_et_ring_buffer_rempli() {
        use std::time::Duration;

        let device_id = get_default_input_device().expect("Pas de device");
        let config = AudioCaptureConfig::default();
        let ring = AudioRingBuffer::new(config.ring_capacity);
        let consumer = ring.consumer_handle();

        let mut unit = AudioUnitCapture::new(device_id, &config).expect("new failed");
        unit.register_input_callback(
            ring.producer_handle(),
            ring.dropped_samples_handle(),
            config.buffer_size_frames as usize,
        )
        .expect("register_input_callback failed");

        unit.start().expect("start failed");
        std::thread::sleep(Duration::from_millis(200));
        unit.stop().expect("stop failed");

        let count = {
            let mut n = 0usize;
            while consumer.pop().is_some() {
                n += 1;
            }
            n
        };
        assert!(count > 0, "Ring buffer vide après start/stop de 200 ms");
    }

    /// Double stop() : idempotent, pas de panic, pas d'erreur.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn double_stop_idempotent() {
        use std::time::Duration;

        let device_id = get_default_input_device().expect("Pas de device");
        let config = AudioCaptureConfig::default();
        let ring = AudioRingBuffer::new(config.ring_capacity);

        let mut unit = AudioUnitCapture::new(device_id, &config).expect("new failed");
        unit.register_input_callback(
            ring.producer_handle(),
            ring.dropped_samples_handle(),
            config.buffer_size_frames as usize,
        )
        .expect("register_input_callback failed");

        unit.start().expect("start 1 failed");
        std::thread::sleep(Duration::from_millis(50));
        unit.stop().expect("stop 1 failed");
        // Deuxième stop : doit être silencieux (no-op).
        unit.stop().expect("stop 2 (double) failed");
    }

    /// Drop sans stop explicite : le process ne doit pas rester suspendu.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn drop_sans_stop_explicite_propre() {
        use std::time::Duration;

        let device_id = get_default_input_device().expect("Pas de device");
        let config = AudioCaptureConfig::default();
        let ring = AudioRingBuffer::new(config.ring_capacity);

        let mut unit = AudioUnitCapture::new(device_id, &config).expect("new failed");
        unit.register_input_callback(
            ring.producer_handle(),
            ring.dropped_samples_handle(),
            config.buffer_size_frames as usize,
        )
        .expect("register_input_callback failed");

        unit.start().expect("start failed");
        std::thread::sleep(Duration::from_millis(50));
        // Drop ici sans appel à stop() — le Drop doit tout nettoyer.
        drop(unit);
        // Si on arrive ici sans hang ni panic, le test passe.
    }
}
