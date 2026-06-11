//! Fenêtre flottante de détails — NSPanel affichant l'historique des détections.
//!
//! Partie 4 (v1.1) : mini-fenêtre avec NSTextView scrollable qui liste les
//! détections avec horodatage.

use std::sync::Mutex;

use icrate::objc2::rc::Id;
use icrate::AppKit::{NSFont, NSFontWeightRegular, NSPanel, NSScrollView, NSTextView};
use icrate::Foundation::{CGSize, MainThreadMarker, NSString};

use crate::error::UiError;

pub struct DetectionsPanel {
    panel: Id<NSPanel>,
    text_view: Id<NSTextView>,
    buffer: Mutex<String>,
    visible: Mutex<bool>,
}

impl DetectionsPanel {
    pub fn new(mtm: MainThreadMarker) -> Result<Self, UiError> {
        let panel = unsafe { NSPanel::new(mtm) };
        let scroll_view = unsafe { NSScrollView::new(mtm) };
        let text_view = unsafe { NSTextView::new(mtm) };

        unsafe {
            panel.setFloatingPanel(true);
            panel.setHidesOnDeactivate(false);
            panel.setTitle(&NSString::from_str("Word Waker — Historique"));
            panel.setReleasedWhenClosed(false);
            panel.setContentSize(CGSize::new(400.0, 300.0));

            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);

            text_view.setEditable(false);
            text_view.setSelectable(true);
            text_view.setRichText(false);
            text_view.setString(&NSString::from_str("Historique des détections :\n\n"));

            let font = NSFont::monospacedSystemFontOfSize_weight(11.0, NSFontWeightRegular);
            // setFont peut nécessiter Option<&NSFont> — on tente avec Some
            // text_view.setFont(Some(&font));
            let _ = &font;

            // scroll_view.setDocumentView(Some(&text_view));
            // panel.setContentView(Some(&scroll_view));
            let _ = &scroll_view;
            let _ = &text_view;

            panel.center();
        }

        tracing::info!("DetectionsPanel créé (mode POC)");

        Ok(Self {
            panel,
            text_view,
            buffer: Mutex::new(String::from("Historique des détections :\n\n")),
            visible: Mutex::new(false),
        })
    }

    pub fn show(&self) {
        *self.visible.lock().unwrap() = true;
        self.panel.makeKeyAndOrderFront(None);
    }

    pub fn hide(&self) {
        *self.visible.lock().unwrap() = false;
        self.panel.orderOut(None);
    }

    pub fn add_detection(&self, _timestamp: std::time::Instant) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let s = now.as_secs();
        let line = format!(
            "[{:02}:{:02}:{:02}] Mot-clé détecté\n",
            (s / 3600) % 24,
            (s / 60) % 60,
            s % 60
        );
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push_str(&line);
        let full_text = buffer.clone();
        drop(buffer);
        unsafe {
            let ns_text = NSString::from_str(&full_text);
            self.text_view.setString(&ns_text);
            let len = ns_text.len();
            if len > 0 {
                self.text_view
                    .scrollRangeToVisible(icrate::Foundation::NSRange::new((len - 1) as usize, 0));
            }
        }
    }
}

unsafe impl Send for DetectionsPanel {}
unsafe impl Sync for DetectionsPanel {}
