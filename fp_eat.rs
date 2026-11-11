fn kotost_fp_eat(p: &mut Poses, n: &str) {
    if n == "right_arm" {
        crate::kotost_body::eat_arm(p);
    }
}
