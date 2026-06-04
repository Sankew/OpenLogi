//! Manual smoke-test for the OS-level mouse hook.
//!
//! Prints every mouse event to stdout and passes all events through unchanged.
//! Press Ctrl-C to stop.
//!
//! # Linux permissions
//!
//! Requires read access to `/dev/input/eventN` and write access to
//! `/dev/uinput`. Add your user to the `input` group and apply a udev rule:
//!
//! ```sh
//! sudo usermod -aG input $USER
//! echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' \
//!     | sudo tee /etc/udev/rules.d/99-uinput.rules
//! sudo udevadm trigger /dev/uinput
//! # log out and back in, then:
//! cargo run --example print_events -p openlogi-hook
//! ```

use openlogi_hook::{EventDisposition, Hook};

fn main() {
    if !Hook::has_accessibility() {
        eprintln!("error: Accessibility permission not granted");
        std::process::exit(1);
    }

    let hook = match Hook::start(|event| {
        println!("{event:?}");
        EventDisposition::PassThrough
    }) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: failed to start hook: {e}");
            std::process::exit(1);
        }
    };

    println!("Hook running — move the mouse or click buttons. Press Ctrl-C to stop.");

    // Block until Ctrl-C.
    let (tx, rx) = std::sync::mpsc::channel();
    #[allow(clippy::expect_used)]
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .expect("failed to set Ctrl-C handler");
    rx.recv().ok();

    hook.stop();
    println!("Hook stopped.");
}
