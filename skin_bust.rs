fn kotost_skin_bust(
    game: &crate::game::Game<'_>,
    m: &Mob,
    chest: Option<&str>,
    camera: glam::DVec3,
    partial: f32,
    light: u32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    use horizon_client::kotost_body as kb;
    use horizon_client::mobs::{anim, render};
    // First person's shadow copy (the head alone), a chestplate over it, an invisible player: none.
    let own = game
        .player_model
        .as_ref()
        .is_some_and(|p| std::ptr::eq(p, m));
    let key = if own { u32::MAX } else { m.id };
    if kb::mode() == kb::Mode::HeadOnly
        || chest.is_some_and(|c| c != "minecraft:elytra")
        || horizon_client::mobs::trader::is_invisible(m)
    {
        kb::debug(key, &format!("skip: mode {:?}, chest {chest:?}", kb::mode()));
        return;
    }
    if !kb::skin_bust_wanted(key, m.slim, || {
        let mut all = game.remote_player_names();
        all.push((u32::MAX, game.own_profile_name()));
        all
    }) {
        kb::debug(key, &format!("not wanted (slim {})", m.slim));
        return;
    }
    let sprites = &game.gui.atlas.sprites;
    let Some(rect) = m
        .skin
        .as_deref()
        .and_then(|s| sprites.get(s))
        .or_else(|| sprites.get(&m.kind.texture))
    else {
        kb::debug(key, &format!("no sprite for {:?} / {}", m.skin, m.kind.texture));
        return;
    };
    // The body as it is drawn this frame: its bone's matrix, and whether it (and its jacket) show.
    let (layer, _) = render::body(m);
    let body = m.kind.layer(&layer).unwrap_or(&m.kind.model);
    let mut poses = anim::Poses::pooled();
    poses.reset(body);
    anim::setup(m, &anim::state(m, partial), &mut poses, &game.res.mob_data().animations);
    if poses.is_hidden_named("body") {
        kb::debug(key, "body hidden");
        return;
    }
    let base = render::base_transform(m, partial, camera);
    let Some(bone) = render::part_matrix(body, base, &poses, "body", 0.0) else {
        kb::debug(key, "no body bone");
        return;
    };
    let model = kb::skin_bust(!poses.is_hidden_named("jacket"));
    kb::wearer(m, own, partial);
    let jiggle = kb::current().unwrap_or_default();
    let mut bust = anim::Poses::pooled();
    bust.reset(model);
    kb::apply(&mut bust, jiggle);
    kb::debug(key, &format!("drawn at {:?}", bone.w_axis));
    let red = m.hurt_time > 0 || m.death_time > 0;
    horizon_client::mobs::render_model(
        model,
        bone,
        &|p| bust.pose(p),
        [rect.u0, rect.v0, rect.u1, rect.v1],
        light,
        horizon_client::overlay::living(0.0, red),
        out,
    );
}
