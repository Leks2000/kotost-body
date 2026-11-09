fn kotost_fp_shadow_head(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> std::ops::ControlFlow<()> {
    use horizon_client::kotost_body as kb;
    if kb::mode() != kb::Mode::Off
        || !kb::first_person()
        || !game.camera_type.first_person()
        || game.sleeping.is_some()
        || game.spectator()
    {
        return std::ops::ControlFlow::Continue(());
    }
    kb::set_mode(kb::Mode::HeadOnly);
    game.player_model_geometry(camera, alpha, out);
    kb::set_mode(kb::Mode::Off);
    std::ops::ControlFlow::Break(())
}
