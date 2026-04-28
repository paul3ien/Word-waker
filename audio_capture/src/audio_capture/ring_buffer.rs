use crossbeam::queue::ArrayQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Wrapper autour d'un `ArrayQueue<f32>` lock-free partagé entre
/// le callback RT (producteur) et le thread consommateur (consommateur).
pub struct AudioRingBuffer {
    queue: Arc<ArrayQueue<f32>>,
    dropped_samples: Arc<AtomicUsize>,
}

impl AudioRingBuffer {
    /// Crée un nouveau ring buffer avec la capacité donnée (en samples).
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
            dropped_samples: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Retourne un `Arc` vers la queue pour le producteur (callback RT).
    pub fn producer_handle(&self) -> Arc<ArrayQueue<f32>> {
        Arc::clone(&self.queue)
    }

    /// Retourne un `Arc` vers la queue pour le consommateur (thread DSP).
    pub fn consumer_handle(&self) -> Arc<ArrayQueue<f32>> {
        Arc::clone(&self.queue)
    }

    /// Retourne un `Arc` vers le compteur de samples perdus (lisible depuis
    /// n'importe quel thread).
    pub fn dropped_samples_handle(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.dropped_samples)
    }

    /// Nombre total de samples perdus depuis la création du buffer.
    pub fn dropped_count(&self) -> usize {
        self.dropped_samples.load(Ordering::Relaxed)
    }
}

/// Pousse un sample dans la queue (RT-safe : zéro allocation, zéro lock).
///
/// Utilise `force_push` : si la queue est pleine, l'élément le plus ancien
/// est écrasé et `dropped` est incrémenté.
#[inline]
pub fn push_sample(queue: &ArrayQueue<f32>, dropped: &AtomicUsize, sample: f32) {
    if queue.force_push(sample).is_some() {
        dropped.fetch_add(1, Ordering::Relaxed);
    }
}

/// Draine tous les samples disponibles sans bloquer.
///
/// Retourne un `Vec<f32>` vide si la queue est vide.
pub fn drain_available(queue: &ArrayQueue<f32>) -> Vec<f32> {
    let mut samples = Vec::new();
    while let Some(s) = queue.pop() {
        samples.push(s);
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cree_buffer_avec_bonne_capacite() {
        let rb = AudioRingBuffer::new(32_000);
        let q = rb.producer_handle();
        assert_eq!(q.capacity(), 32_000);
    }

    #[test]
    fn producer_et_consumer_handle_meme_queue() {
        let rb = AudioRingBuffer::new(1024);
        let producer = rb.producer_handle();
        let consumer = rb.consumer_handle();
        assert!(Arc::ptr_eq(&producer, &consumer));
    }

    #[test]
    fn fifo_n_samples() {
        let rb = AudioRingBuffer::new(16);
        let q = rb.producer_handle();
        let dropped = rb.dropped_samples_handle();
        for i in 0..10 {
            push_sample(&q, &dropped, i as f32);
        }
        let result = drain_available(&rb.consumer_handle());
        assert_eq!(result.len(), 10);
        for (i, &v) in result.iter().enumerate() {
            assert_eq!(v, i as f32, "ordre FIFO non respecté à l'index {}", i);
        }
    }

    #[test]
    fn overflow_incremente_dropped_samples() {
        let capacity = 4;
        let rb = AudioRingBuffer::new(capacity);
        let q = rb.producer_handle();
        let dropped = rb.dropped_samples_handle();
        // Pousse capacity + 3 samples → 3 doivent être écrasés
        for i in 0..(capacity + 3) {
            push_sample(&q, &dropped, i as f32);
        }
        assert_eq!(rb.dropped_count(), 3);
    }

    #[test]
    fn drain_buffer_vide_retourne_vec_vide() {
        let rb = AudioRingBuffer::new(64);
        let result = drain_available(&rb.consumer_handle());
        assert!(result.is_empty());
    }

    #[test]
    fn audio_ring_buffer_est_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AudioRingBuffer>();
    }

    #[test]
    fn thread_safety_producteur_consommateur() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::thread;

        const N: usize = 1_000_000;
        let rb = AudioRingBuffer::new(4096);
        let producer_q = rb.producer_handle();
        let consumer_q = rb.consumer_handle();
        let final_drain_q = rb.consumer_handle(); // pour le drain final depuis le main
        let dropped = rb.dropped_samples_handle();
        let dropped_producer = Arc::clone(&dropped);
        let running = Arc::new(AtomicBool::new(true));
        let running_consumer = Arc::clone(&running);

        // Thread producteur : pousse 1M samples
        let producer = thread::spawn(move || {
            for i in 0..N {
                push_sample(&producer_q, &dropped_producer, i as f32);
            }
        });

        // Thread consommateur : draine en continu tant que le signal est actif
        let consumer = thread::spawn(move || {
            let mut total = 0usize;
            while running_consumer.load(Ordering::Acquire) {
                total += drain_available(&consumer_q).len();
                std::hint::spin_loop();
            }
            total
        });

        // Attendre la fin du producteur, puis signaler l'arrêt du consommateur
        producer.join().expect("thread producteur panic");
        running.store(false, Ordering::Release);
        let consumed = consumer.join().expect("thread consommateur panic");

        // Drainer les samples restants que le consommateur n'a pas eu le temps de lire
        let remaining = drain_available(&final_drain_q).len();
        let drops = dropped.load(Ordering::Relaxed);

        // Invariant : produit = consommé + restant + perdus
        assert_eq!(
            consumed + remaining + drops,
            N,
            "consumed={}, remaining={}, drops={}, N={}",
            consumed, remaining, drops, N
        );
    }
}
