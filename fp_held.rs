fn kotost_fp_held() -> std::ops::ControlFlow<()> {
    use horizon_client::kotost_body as kb;
    // First person: the items are in the model's hands only when its right arm is shown (the game's
    // flat hands are hidden then); else they are the game's own first-person ones (also while eating).
    if kb::mode() == kb::Mode::Body && (!kb::tune().right_arm || kb::fp_eating_hand()) {
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
