//! Word Waker UI — macOS menu bar application.
//!
//! Se connecte au daemon word-waker via Unix Domain Socket et affiche
//! l'état du pipeline de détection de mot-clé en temps réel dans la barre de menu.
//!
//! # Architecture
//!
//! ```text
//! daemon (socket server)  ──IPC──▶  ui (socket client)
//!                                       │
//!                                  NSStatusBar
//!                                  (emoji 🎤/⏸)
//! ```
//!
//! # Utilisation
//!
//! ```bash
//! # Lancer l'UI (le daemon doit déjà tourner)
//! cargo run --release -p ui
//!
//! # Avec un socket personnalisé
//! WAKEWORD_SOCKET_PATH=/tmp/mon_socket.sock cargo run --release -p ui
//! ```

fn main() {
    println!("Word Waker UI — placeholder");
    println!("Voir ui/stack.md et ui/backlog.md pour le plan d'implémentation.");
    println!("Exécute `cargo test -p ui` pour les tests (quand implémentés).");
}
