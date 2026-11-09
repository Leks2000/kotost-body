fn kotost_fp_head_item() -> std::ops::ControlFlow<()> {
    if horizon_client::kotost_body::mode() == horizon_client::kotost_body::Mode::Body {
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
