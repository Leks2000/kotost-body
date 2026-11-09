fn kotost_fp_held() -> std::ops::ControlFlow<()> {
    use horizon_client::kotost_body as kb;
    // First person: the items are in the model's hands only when its right arm is shown (the game's
    // flat hands are hidden then); else they are the game's own first-person ones.
    if kb::mode() == kb::Mode::Body && !kb::tune().right_arm {
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
