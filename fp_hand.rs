fn kotost_fp_hand(game: &crate::game::Game<'_>, canceled: &mut bool) -> std::ops::ControlFlow<()> {
    use horizon_client::kotost_body as kb;
    // The model's 3D arms and held items replace the game's flat first-person hands
    // (but while eating or drinking the game's own hands show the food before the face).
    let tune = kb::tune();
    if tune.first_person
        && tune.right_arm
        && game.camera_type.first_person()
        && game.sleeping.is_none()
        && !game.spectator()
        && !kb::fp_eating_hand()
    {
        *canceled = true;
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
