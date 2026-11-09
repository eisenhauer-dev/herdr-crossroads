pub(crate) fn input_mode(input: u32, mouse_enabled: bool) -> u32 {
    let raw = (input & !(0x1 | 0x2 | 0x4 | 0x10 | 0x40)) | 0x80 | 0x200;
    if mouse_enabled {
        raw | 0x10
    } else {
        raw
    }
}
