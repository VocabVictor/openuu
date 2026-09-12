use super::*;

#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
/// MouseButton represents a mouse button,
/// and is used in for example
/// [mouse_click](trait.MouseControllable.html#tymethod.mouse_click).
/// WARNING: Types with the prefix Scroll
/// IS NOT intended to be used, and may not work on
/// all operating systems.
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Middle mouse button
    Middle,
    /// Right mouse button
    Right,
    /// Back mouse button
    Back,
    /// Forward mouse button
    Forward,

    /// Scroll up button
    ScrollUp,
    /// Scroll down button
    ScrollDown,
    /// Scroll left button
    ScrollLeft,
    /// Scroll right button
    ScrollRight,
}

/// Representing an interface and a set of mouse functions every
/// operating system implementation _should_ implement.
pub trait MouseControllable {
    // https://stackoverflow.com/a/33687996
    /// Offer the ability to confer concrete type.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Offer the ability to confer concrete type.
    fn as_mut_any(&mut self) -> &mut dyn std::any::Any;

    /// Lets the mouse cursor move to the specified x and y coordinates.
    ///
    /// The topleft corner of your monitor screen is x=0 y=0. Move
    /// the cursor down the screen by increasing the y and to the right
    /// by increasing x coordinate.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_move_to(500, 200);
    /// ```
    fn mouse_move_to(&mut self, x: i32, y: i32);

    /// Lets the mouse cursor move the specified amount in the x and y
    /// direction.
    ///
    /// The amount specified in the x and y parameters are added to the
    /// current location of the mouse cursor. A positive x values lets
    /// the mouse cursor move an amount of `x` pixels to the right. A negative
    /// value for `x` lets the mouse cursor go to the left. A positive value
    /// of y
    /// lets the mouse cursor go down, a negative one lets the mouse cursor go
    /// up.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_move_relative(100, 100);
    /// ```
    fn mouse_move_relative(&mut self, x: i32, y: i32);

    /// Push down one of the mouse buttons
    ///
    /// Push down the mouse button specified by the parameter `button` of
    /// type [MouseButton](enum.MouseButton.html)
    /// and holds it until it is released by
    /// [mouse_up](trait.MouseControllable.html#tymethod.mouse_up).
    /// Calls to [mouse_move_to](trait.MouseControllable.html#tymethod.
    /// mouse_move_to) or
    /// [mouse_move_relative](trait.MouseControllable.html#tymethod.
    /// mouse_move_relative)
    /// will work like expected and will e.g. drag widgets or highlight text.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_down(MouseButton::Left);
    /// ```
    fn mouse_down(&mut self, button: MouseButton) -> ResultType;

    /// Lift up a pushed down mouse button
    ///
    /// Lift up a previously pushed down button (by invoking
    /// [mouse_down](trait.MouseControllable.html#tymethod.mouse_down)).
    /// If the button was not pushed down or consecutive calls without
    /// invoking [mouse_down](trait.MouseControllable.html#tymethod.mouse_down)
    /// will emit lift up events. It depends on the
    /// operating system whats actually happening – my guess is it will just
    /// get ignored.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_up(MouseButton::Right);
    /// ```
    fn mouse_up(&mut self, button: MouseButton);

    /// Click a mouse button
    ///
    /// it's essentially just a consecutive invocation of
    /// [mouse_down](trait.MouseControllable.html#tymethod.mouse_down) followed
    /// by a [mouse_up](trait.MouseControllable.html#tymethod.mouse_up). Just
    /// for
    /// convenience.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_click(MouseButton::Right);
    /// ```
    fn mouse_click(&mut self, button: MouseButton);

    /// Scroll the mouse (wheel) left or right
    ///
    /// Positive numbers for length lets the mouse wheel scroll to the right
    /// and negative ones to the left. The value that is specified translates
    /// to `lines` defined by the operating system and is essentially one 15°
    /// (click)rotation on the mouse wheel. How many lines it moves depends
    /// on the current setting in the operating system.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_scroll_x(2);
    /// ```
    fn mouse_scroll_x(&mut self, length: i32);

    /// Scroll the mouse (wheel) up or down
    ///
    /// Positive numbers for length lets the mouse wheel scroll down
    /// and negative ones up. The value that is specified translates
    /// to `lines` defined by the operating system and is essentially one 15°
    /// (click)rotation on the mouse wheel. How many lines it moves depends
    /// on the current setting in the operating system.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_scroll_y(2);
    /// ```
    fn mouse_scroll_y(&mut self, length: i32);
}
