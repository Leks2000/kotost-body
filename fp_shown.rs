fn kotost_fp_shown(game: &crate::game::Game<'_>) -> std::ops::ControlFlow<bool> {
    use horizon_client::kotost_body as kb;
    if kb::mode() == kb::Mode::Body {
        let alive = game.player.as_ref().is_some_and(|p| !p.health.dead_or_dying());
        return std::ops::ControlFlow::Break(alive);
    }
    std::ops::ControlFlow::Continue(())
}
