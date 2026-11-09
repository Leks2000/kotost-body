fn kotost_bust_pose(
    game: &crate::game::Game<'_>,
    model: &Arc<Part>,
    matrix: glam::Mat4,
    poses: &Poses,
    uv: [f32; 4],
    light: u32,
    gpu: bool,
    bounds: Option<(glam::Vec3, glam::Vec3)>,
    out: &mut Vec<Vertex>,
) -> std::ops::ControlFlow<Option<(glam::Vec3, glam::Vec3)>> {
    use horizon_client::kotost_body as kb;
    if kb::nested() || !(poses.has("bust_left") || poses.has("bust_right")) {
        return std::ops::ControlFlow::Continue(());
    }
    kb::mark_item_bust();
    let Some(jiggle) = kb::current() else {
        return std::ops::ControlFlow::Continue(());
    };
    let mut moved = Poses::pooled();
    moved.copy_from(poses);
    kb::apply(&mut moved, jiggle);
    kb::set_nested(true);
    let drawn = game.cosmetics_model(model, matrix, &moved, uv, light, gpu, bounds, out);
    kb::set_nested(false);
    std::ops::ControlFlow::Break(drawn)
}
