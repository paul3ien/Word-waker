use audio_capture::{AudioCapture, AudioCaptureConfig};

fn main() {
    let _ = AudioCapture::new(AudioCaptureConfig::default());
}
