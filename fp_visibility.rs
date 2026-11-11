fn kotost_fp_visibility(p: &mut Poses, hidden: u8, spectator: bool) -> std::ops::ControlFlow<()> {
    use crate::kotost_body::{Mode, mode};
    // An hourglass player: the torso box undrawn (the mod draws its own), the arms a little out.
    crate::kotost_body::shape_poses(p);
    let layers = [
        (1, "jacket"),
        (2, "left_sleeve"),
        (3, "right_sleeve"),
        (4, "left_pants"),
        (5, "right_pants"),
        (6, "hat"),
    ];
    match mode() {
        Mode::Off => std::ops::ControlFlow::Continue(()),
        Mode::HeadOnly => {
            for (bit, part) in layers {
                p.visible(part, hidden & (1 << bit) == 0);
            }
            for part in ["body", "right_arm", "left_arm", "right_leg", "left_leg"] {
                p.visible(part, false);
            }
            p.visible("head", true);
            std::ops::ControlFlow::Break(())
        }
        Mode::Body => {
            for (bit, part) in layers {
                p.visible(part, hidden & (1 << bit) == 0);
            }
            for part in ["body", "right_leg", "left_leg"] {
                p.visible(part, !spectator);
            }
            for part in ["head", "hat"] {
                p.visible(part, false);
            }
            // The arms by their settings (without the right one, the game's flat hand shows).
            let tune = crate::kotost_body::tune();
            for (shown, arm, sleeve) in [
                (tune.left_arm, "left_arm", "left_sleeve"),
                (tune.right_arm, "right_arm", "right_sleeve"),
            ] {
                if shown {
                    p.visible(arm, !spectator);
                } else {
                    p.visible(arm, false);
                    p.visible(sleeve, false);
                }
            }
            std::ops::ControlFlow::Break(())
        }
    }
}
