//! Gestion du NSStatusItem dans la barre de menu macOS.
//!
//! Affiche l'emoji et gère le menu contextuel via AppKit natif.
//! Utilise `icrate` pour les bindings AppKit idiomatiques.

use icrate::objc2::rc::Id;
use icrate::AppKit::{NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use icrate::Foundation::{MainThreadMarker, NSString};

use crate::error::UiError;

/// Représente l'icône dans la barre de menu macOS.
/// Wrapper autour du NSStatusItem AppKit.
pub struct MenuBarIcon {
    /// Le NSStatusItem sous-jacent
    status_item: Id<NSStatusItem>,
}

impl MenuBarIcon {
    /// Crée un nouveau NSStatusItem dans la barre de menu.
    pub fn new(mtm: MainThreadMarker) -> Result<Self, UiError> {
        let status_bar = unsafe { NSStatusBar::systemStatusBar() };
        let status_item = unsafe { status_bar.statusItemWithLength(NSVariableStatusItemLength) };

        // Créer et associer le menu
        let menu = Self::build_menu(mtm)?;
        unsafe {
            status_item.setMenu(Some(&menu));
        }

        Ok(Self { status_item })
    }

    /// Définit le texte affiché dans la barre de menu (emoji).
    pub fn set_title(&self, title: &str) {
        unsafe {
            let ns_title = NSString::from_str(title);
            // Utiliser le bouton du status item pour setTitle (recommandé)
            self.status_item.setTitle(Some(&ns_title));
        }
    }

    /// Définit le tooltip affiché au survol.
    pub fn set_tooltip(&self, tooltip: &str) {
        unsafe {
            let ns_tooltip = NSString::from_str(tooltip);
            self.status_item.setToolTip(Some(&ns_tooltip));
        }
    }

    /// Construit le menu contextuel du StatusItem.
    fn build_menu(mtm: MainThreadMarker) -> Result<Id<NSMenu>, UiError> {
        let menu = NSMenu::new(mtm);

        // Ajouter le séparateur
        let separator = NSMenuItem::separatorItem(mtm);
        menu.addItem(&separator);

        // Item "Quitter"
        let quit_title = NSString::from_str("Quitter");
        let key_equiv = NSString::from_str("q");
        let quit_item = NSMenuItem::new(mtm);
        unsafe {
            quit_item.setTitle(&quit_title);
            quit_item.setKeyEquivalent(&key_equiv);
        }
        menu.addItem(&quit_item);

        Ok(menu)
    }
}
