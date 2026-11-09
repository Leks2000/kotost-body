fn kotost_fp_covered(slot: Slot) -> std::ops::ControlFlow<bool> {
    use horizon_client::kotost_body::{Mode, mode};
    let head = matches!(slot, Slot::Head);
    match mode() {
        Mode::HeadOnly if !head => std::ops::ControlFlow::Break(true),
        Mode::Body if head => std::ops::ControlFlow::Break(true),
        _ => std::ops::ControlFlow::Continue(()),
    }
}
