fn kotost_body_shape_mob(m: &Mob, s: &State) {
    crate::kotost_body::setup_mob(m);
    crate::kotost_body::eat_begin(m, s.partial);
}
