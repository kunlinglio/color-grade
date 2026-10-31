use gpui_kit::*;

/// Scopes panel (waveform / vectorscope / histogram). Placeholder for now.
pub struct Scopes {}

impl Scopes {
    pub fn new() -> Self {
        Scopes {}
    }
}

impl Render for Scopes {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(text!("Scopes"))
    }
}
