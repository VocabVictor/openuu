use super::*;

/// A key on the keyboard.
/// For alphabetical keys, use Key::Layout for a system independent key.
/// If a key is missing, you can use the raw keycode with Key::Raw.
#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// alt key on Linux and Windows (option key on macOS)
    Alt,
    /// backspace key
    Backspace,
    /// caps lock key
    CapsLock,
    // #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// command key on macOS (super key on Linux, windows key on Windows)
    Command,
    /// control key
    Control,
    /// delete key
    Delete,
    /// down arrow key
    DownArrow,
    /// end key
    End,
    /// escape key (esc)
    Escape,
    /// F1 key
    F1,
    /// F10 key
    F10,
    /// F11 key
    F11,
    /// F12 key
    F12,
    /// F2 key
    F2,
    /// F3 key
    F3,
    /// F4 key
    F4,
    /// F5 key
    F5,
    /// F6 key
    F6,
    /// F7 key
    F7,
    /// F8 key
    F8,
    /// F9 key
    F9,
    /// home key
    Home,
    /// left arrow key
    LeftArrow,
    /// meta key (also known as "windows", "super", and "command")
    Meta,
    /// option key on macOS (alt key on Linux and Windows)
    Option, // deprecated, use Alt instead
    /// page down key
    PageDown,
    /// page up key
    PageUp,
    /// return key
    Return,
    /// right arrow key
    RightArrow,
    /// shift key
    Shift,
    /// space key
    Space,
    // #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// super key on linux (command key on macOS, windows key on Windows)
    Super,
    /// tab key (tabulator)
    Tab,
    /// up arrow key
    UpArrow,
    // #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// windows key on Windows (super key on Linux, command key on macOS)
    Windows,
    ///
    Numpad0,
    ///
    Numpad1,
    ///
    Numpad2,
    ///
    Numpad3,
    ///
    Numpad4,
    ///
    Numpad5,
    ///
    Numpad6,
    ///
    Numpad7,
    ///
    Numpad8,
    ///
    Numpad9,
    ///
    Cancel,
    ///
    Clear,
    ///
    Pause,
    ///
    Kana,
    ///
    Hangul,
    ///
    Junja,
    ///
    Final,
    ///
    Hanja,
    ///
    Kanji,
    ///
    Convert,
    ///
    Select,
    ///
    Print,
    ///
    Execute,
    ///
    Snapshot,
    ///
    Insert,
    ///
    Help,
    ///
    Sleep,
    ///
    Separator,
    ///
    VolumeUp,
    ///
    VolumeDown,
    ///
    Mute,
    ///
    Scroll,
    /// scroll lock
    NumLock,
    ///
    RWin,
    ///
    Apps,
    ///
    Multiply,
    ///
    Add,
    ///
    Subtract,
    ///
    Decimal,
    ///
    Divide,
    ///
    Equals,
    ///
    NumpadEnter,
    ///
    RightShift,
    ///
    RightControl,
    ///
    RightAlt,
    ///
    /// Function, /// mac
    /// keyboard layout dependent key
    Layout(char),
    /// raw keycode eg 0x38
    Raw(u16),
}

/// Representing an interface and a set of keyboard functions every
/// operating system implementation _should_ implement.
pub trait KeyboardControllable {
    // https://stackoverflow.com/a/33687996
    /// Offer the ability to confer concrete type.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Offer the ability to confer concrete type.
    fn as_mut_any(&mut self) -> &mut dyn std::any::Any;

    /// Types the string parsed with DSL.
    ///
    /// Typing {+SHIFT}hello{-SHIFT} becomes HELLO.
    /// TODO: Full documentation
    fn key_sequence_parse(&mut self, sequence: &str)
    where
        Self: Sized,
    {
        if let Err(..) = self.key_sequence_parse_try(sequence) {
            println!("Could not parse sequence");
        }
    }
    /// Same as key_sequence_parse except returns any errors
    fn key_sequence_parse_try(&mut self, sequence: &str) -> Result<(), dsl::ParseError>
    where
        Self: Sized,
    {
        dsl::eval(self, sequence)
    }

    /// Types the string
    ///
    /// Emits keystrokes such that the given string is inputted.
    ///
    /// You can use many unicode here like: ❤️. This works
    /// regardless of the current keyboardlayout.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.key_sequence("hello world ❤️");
    /// ```
    fn key_sequence(&mut self, sequence: &str);

    /// presses a given key down
    fn key_down(&mut self, key: Key) -> ResultType;

    /// release a given key formally pressed down by
    /// [key_down](trait.KeyboardControllable.html#tymethod.key_down)
    fn key_up(&mut self, key: Key);

    /// Much like the
    /// [key_down](trait.KeyboardControllable.html#tymethod.key_down) and
    /// [key_up](trait.KeyboardControllable.html#tymethod.key_up)
    /// function they're just invoked consecutively
    fn key_click(&mut self, key: Key);

    ///
    fn get_key_state(&mut self, key: Key) -> bool;
}
