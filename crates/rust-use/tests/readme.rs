//! Compiles and runs the README usage example.
#![cfg(all(feature = "macros", feature = "agent"))]

#[test]
#[allow(unused_must_use, reason = "Keep the README usage example verbatim.")]
fn usage() -> Result<(), Box<dyn std::error::Error>> {
    use rust_use::prelude::*;

    /// A button that mutes audio.
    #[derive(Primitive, Default)]
    struct MuteButton {
        #[agent(value)]
        muted: bool,
    }

    #[actions]
    impl MuteButton {
        /// Mute or unmute.
        fn toggle(&mut self) {
            self.muted = !self.muted;
        }
    }

    let ui = Ui::new();
    ui.mount(MuteButton::default()).label("Mute");

    let agent = ui.agent();
    println!("{}", agent.snapshot());
    // rev 2
    // @1 mute-button "Mute" =false !toggle

    assert_eq!(
        agent.snapshot().to_string(),
        "rev 2\n@1 mute-button \"Mute\" =false !toggle"
    );
    agent.act("Mute toggle")?;
    Ok(())
}
