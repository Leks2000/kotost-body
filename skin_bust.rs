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
    // First person's shadow copy (the head alone): nothing, and no verdict either (its body is hidden).
    if kb::mode() == kb::Mode::HeadOnly {
        return;
    }
    let own = game
        .player_model
        .as_ref()
        .is_some_and(|p| std::ptr::eq(p, m));
    let key = if own { u32::MAX } else { m.id };
    let sprites = &game.gui.atlas.sprites;
    // A chestplate over it (not an elytra nor clothes another mod marked): a vanilla one is drawn here in
    // the figure's shape (its torso and cups, the game's box left undrawn with the body's); another mod's,
    // or an invisible player: the game's own body, no bust.
    let armor = chest.filter(|c| *c != "minecraft:elytra" && !kb::is_cloth(c));
    let plate = armor.and_then(|c| {
        let rect = sprites.get(kb::armor_texture(c)?.as_str())?;
        let tint = if c == "minecraft:leather_chestplate" {
            render::armor_tint(Some(0xA06540), m.extra.armor[1].as_ref())
        } else {
            0xFFFF_FFFF
        };
        Some(([rect.u0, rect.v0, rect.u1, rect.v1], tint))
    });
    if (armor.is_some() && plate.is_none()) || horizon_client::mobs::trader::is_invisible(m) {
        kb::note_body(m.id, false, false);
        kb::debug(key, &format!("skip: chest {chest:?}"));
        return;
    }
    let item = kb::wears_item(key);
    let skin = kb::skin_bust_wanted(key, m.slim, || {
        let mut all = game.remote_player_names();
        all.push((u32::MAX, game.own_profile_name()));
        all
    });
    // Every bust wearer (item or skin) gets the hourglass body.
    let shaped = (item || skin) && kb::tune().hourglass;
    kb::note_body(m.id, shaped, item || skin);
    if plate.is_some() && (item || skin) {
        // Under the chestplate: no bust item either (the cups stand for it).
        kb::cover(key);
    }
    let bust = skin && !kb::covered(key);
    if !(shaped || bust || (plate.is_some() && (item || skin))) {
        kb::debug(key, &format!("not wanted (slim {}, item {item}, covered {})", m.slim, kb::covered(key)));
        return;
    }
    let Some(rect) = m
        .skin
        .as_deref()
        .and_then(|s| sprites.get(s))
        .or_else(|| sprites.get(&m.kind.texture))
    else {
        kb::debug(key, &format!("no sprite for {:?} / {}", m.skin, m.kind.texture));
        return;
    };
    let uv = [rect.u0, rect.v0, rect.u1, rect.v1];
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
    let jacket = !poses.is_hidden_named("jacket");
    let red = m.hurt_time > 0 || m.death_time > 0;
    let overlay = horizon_client::overlay::living(0.0, red);
    if let Some((plate_uv, tint)) = plate {
        // Armour shows no hurt flash (the game's armour layers: NO_OVERLAY).
        let armor_overlay = horizon_client::overlay::living(0.0, false);
        // The chestplate in the figure's shape: its torso where the game's box was (hourglass on), and
        // the cups over the bust, jiggling as the bust does.
        if shaped {
            let torso = kb::armor_body();
            let mut still = anim::Poses::pooled();
            still.reset(&torso);
            render::draw(&torso, bone, &still, plate_uv, tint, light, armor_overlay, out);
        }
        let cups = kb::armor_bust();
        kb::wearer(m, own, partial);
        let jiggle = kb::current().unwrap_or_default();
        let mut posed = anim::Poses::pooled();
        posed.reset(cups);
        kb::apply(&mut posed, jiggle);
        render::draw(cups, bone, &posed, plate_uv, tint, light, armor_overlay, out);
        kb::debug(key, "chestplate in shape");
        return;
    }
    if shaped {
        // In place of the torso box the model left undrawn (`shape_poses`).
        let torso = kb::hourglass_body(jacket);
        horizon_client::mobs::render_model(&torso, bone, &|p: &horizon_client::mobs::Part| p.pose, uv, light, overlay, out);
    }
    if !bust {
        kb::debug(key, "hourglass only (item bust or covered)");
        return;
    }
    let model = kb::skin_bust(jacket);
    kb::wearer(m, own, partial);
    let jiggle = kb::current().unwrap_or_default();
    let mut bust = anim::Poses::pooled();
    bust.reset(model);
    kb::apply(&mut bust, jiggle);
    kb::debug(key, &format!("drawn at {:?}", bone.w_axis));
    horizon_client::mobs::render_model(model, bone, &|p| bust.pose(p), uv, light, overlay, out);
}
