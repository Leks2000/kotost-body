fn kotost_fp_body(game: &crate::game::Game<'_>, camera: DVec3, out: &mut Vec<Vertex>) {
    use horizon_client::kotost_body as kb;
    if kb::mode() != kb::Mode::Off
        || !kb::first_person()
        || !game.camera_type.first_person()
        || game.sleeping.is_some()
        || game.spectator()
    {
        return;
    }
    let Some(m) = game.player_model.as_ref() else {
        return;
    };
    let alpha = game.clock.camera_fraction();
    let f = alpha as f32;
    let pitch = m.previous_head_pitch + (m.head_pitch - m.previous_head_pitch) * f;
    let mut turn = m.head_yaw - m.previous_head_yaw;
    if turn > 180.0 {
        turn -= 360.0;
    } else if turn < -180.0 {
        turn += 360.0;
    }
    let yaw = (m.previous_head_yaw + turn * f).to_radians();
    let ahead = DVec3::new(-f64::from(yaw.sin()), 0.0, f64::from(yaw.cos()));
    let back = kb::back(pitch);
    game.mark_player(out.len(), false);
    kb::set_mode(kb::Mode::Body);
    game.player_model_geometry(camera + ahead * back, alpha, out);
    kb::set_mode(kb::Mode::Off);
}
