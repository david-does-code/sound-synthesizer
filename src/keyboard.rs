use evdev::{Device, InputEventKind, Key};
use std::sync::mpsc;
use std::thread;

/// Events our keyboard reader sends to the main loop.
#[derive(Debug)]
pub enum KeyboardEvent {
    NoteOn { midi: u8, name: &'static str },
    NoteOff { midi: u8 },
    Quit,
}

/// Maps evdev key codes to (MIDI note, display name).
/// Same piano layout as before, but now we read physical key state directly.
fn evdev_key_to_note(key: Key) -> Option<(u8, &'static str)> {
    match key {
        // White keys (home row)
        Key::KEY_A => Some((60, "C4")),
        Key::KEY_S => Some((62, "D4")),
        Key::KEY_D => Some((64, "E4")),
        Key::KEY_F => Some((65, "F4")),
        Key::KEY_G => Some((67, "G4")),
        Key::KEY_H => Some((69, "A4")),
        Key::KEY_J => Some((71, "B4")),
        Key::KEY_K => Some((72, "C5")),
        Key::KEY_L => Some((74, "D5")),
        Key::KEY_SEMICOLON => Some((76, "E5")),

        // Black keys (top row)
        Key::KEY_W => Some((61, "C#4")),
        Key::KEY_E => Some((63, "D#4")),
        Key::KEY_T => Some((66, "F#4")),
        Key::KEY_Y => Some((68, "G#4")),
        Key::KEY_U => Some((70, "A#4")),
        Key::KEY_O => Some((73, "C#5")),
        Key::KEY_P => Some((75, "D#5")),

        _ => None,
    }
}

/// Find the keyboard device in /dev/input/.
/// Looks for a device that has typical keyboard keys.
fn find_keyboard() -> Option<Device> {
    let mut candidates = Vec::new();
    let devices = evdev::enumerate();
    for (_path, device) in devices {
        if let Some(keys) = device.supported_keys() {
            // A real keyboard:
            // - supports letter keys, ESC, and numbers
            // - has LEDs (caps lock, num lock indicators)
            // - does NOT have relative axes (that's a mouse)
            let has_basic_keys = keys.contains(Key::KEY_A)
                && keys.contains(Key::KEY_ESC)
                && keys.contains(Key::KEY_1);
            let has_leds = device.supported_leds().is_some();
            let has_mouse_axes = device.supported_relative_axes().is_some();

            if has_basic_keys && has_leds && !has_mouse_axes {
                let name = device.name().unwrap_or("unknown").to_string();
                candidates.push((device, name));
            }
        }
    }
    // Prefer devices with more keys (the main keyboard interface, not media keys)
    candidates.sort_by_key(|(device, _)| {
        std::cmp::Reverse(device.supported_keys().map_or(0, |k| k.iter().count()))
    });
    candidates.into_iter().next().map(|(device, _)| device)
}

/// Spawn a background thread that reads raw keyboard events and sends
/// them as KeyboardEvents over a channel.
///
/// This reads directly from Linux evdev (/dev/input/event*), which gives
/// us real key press AND release events — something the terminal can't do.
/// The kernel sends: value=1 for press, value=0 for release, value=2 for repeat.
pub fn spawn_keyboard_listener() -> mpsc::Receiver<KeyboardEvent> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut device = find_keyboard().expect(
            "No keyboard found in /dev/input/. Make sure you're in the 'input' group.",
        );

        loop {
            // fetch_events() blocks until events are available
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if let InputEventKind::Key(key) = event.kind() {
                            let value = event.value();

                            // ESC press → quit
                            if key == Key::KEY_ESC && value == 1 {
                                let _ = tx.send(KeyboardEvent::Quit);
                                return;
                            }

                            if let Some((midi, name)) = evdev_key_to_note(key) {
                                let msg = match value {
                                    1 => KeyboardEvent::NoteOn { midi, name }, // press
                                    0 => KeyboardEvent::NoteOff { midi },      // release
                                    _ => continue, // 2 = repeat, ignore it
                                };
                                if tx.send(msg).is_err() {
                                    return; // main thread dropped the receiver
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("evdev error: {e}");
                    return;
                }
            }
        }
    });

    rx
}
