fn kotost_fp_hand(game: &crate::game::Game<'_>, canceled: &mut bool) -> std::ops::ControlFlow<()> {
    use horizon_client::kotost_body as kb;
    // The model's 3D arms and held items replace the game's flat first-person hands.
    let tune = kb::tune();
    if tune.first_person
        && tune.right_arm
        && game.camera_type.first_person()
        && game.sleeping.is_none()
        && !game.spectator()
    {
        *canceled = true;
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
