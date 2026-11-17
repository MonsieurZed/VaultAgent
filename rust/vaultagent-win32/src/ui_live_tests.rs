//! Tests Win32 réels de l'interface, ignorés par défaut : ils ouvrent une vraie icône et de vraies fenêtres.

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::ui_controls::Control;
    use crate::ui_icon::Icon;
    use crate::ui_menu::MenuEntry;
    use crate::ui_system::{SingleInstance, work_area};
    use crate::ui_tray::{BalloonKind, TrayIcon};
    use crate::ui_window::{UiEvent, UiHandler, WindowKind, WindowRef, WindowSpec, create_window};

    const MUTEX: &str = "VaultAgentLiveTestMutex";

    struct Recorder(Rc<Cell<u32>>);

    impl UiHandler for Recorder {
        /// Compte les événements reçus.
        fn handle(&mut self, _window: WindowRef, _event: UiEvent) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    #[ignore = "cree une vraie icone de barre et une vraie fenetre"]
    fn icon_window_controls_and_tray_are_created() {
        let icon = Icon::shield().unwrap();
        assert!(icon.raw() != 0);
        let seen = Rc::new(Cell::new(0));
        let spec = WindowSpec {
            class: "VaultAgentLiveTest",
            title: "VaultAgent test",
            kind: WindowKind::Dialog,
            rect: (0, 0, 200, 120),
        };
        let window = create_window(&spec, Box::new(Recorder(seen.clone()))).unwrap();
        assert!(seen.get() >= 1);
        let edit = window.add_control(Control::Edit, "abc", 1, (5, 5, 100, 24)).unwrap();
        assert_eq!(edit.text(), "abc");
        let check = window
            .add_control(Control::Checkbox, "ok", 2, (5, 40, 100, 24))
            .unwrap();
        check.set_checked(true);
        assert!(check.is_checked());
        let tray = TrayIcon::add(window, &icon, "VaultAgent test").unwrap();
        tray.set_tip("VaultAgent test 2").unwrap();
        tray.balloon("VaultAgent", "test", BalloonKind::Info).unwrap();
        window.destroy();
    }

    #[test]
    #[ignore = "lit l'ecran et cree un mutex nomme"]
    fn work_area_and_single_instance_answer() {
        let (left, top, right, bottom) = work_area().unwrap();
        assert!(right > left && bottom > top);
        let first = SingleInstance::acquire(MUTEX).unwrap();
        assert!(first.is_some());
        assert!(SingleInstance::acquire(MUTEX).unwrap().is_none());
        let entries = [MenuEntry::Separator];
        assert_eq!(entries.len(), 1);
    }
}
