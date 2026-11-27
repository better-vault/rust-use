pub(super) const BACKGROUND: u32 = 0x26_25_22;

pub(super) const LIGHT: u32 = 0xEB_EC_D0;

pub(super) const DARK: u32 = 0x7B_9A_5A;

pub(super) const CARD: u32 = 0x30_2E_2B;

pub(super) const BUTTON: u32 = 0x3A_38_35;

pub(super) const BUTTON_HOVER: u32 = 0x45_43_3F;

pub(super) const TEXT: u32 = 0xE8_E6_E3;

pub(super) const MUTED: u32 = 0x9A_98_94;

pub(super) const SELECTED: u32 = 0xFFFF_3380;

pub(super) const LAST_MOVE: u32 = 0xFFFF_3359;

pub(super) const TARGET: u32 = 0x0000_0024;

pub(super) const SQUARE: f32 = 80.;

pub(super) const BOARD_SIZE: f32 = SQUARE * 8.;

pub(super) const FRAME_RADIUS: f32 = 10.;

pub(super) const PIECE_SIZE: f32 = SQUARE * 0.86;

pub(super) const DOT_SIZE: f32 = SQUARE * 0.28;

pub(super) const LABEL_SIZE: f32 = 11.;

pub(super) const PANEL_WIDTH: f32 = 300.;

pub(super) const CHECK_TINT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><defs><radialGradient id="check"><stop stop-color="#dc322f"/><stop offset=".65" stop-color="#dc322f" stop-opacity=".7"/><stop offset="1" stop-color="#dc322f" stop-opacity="0"/></radialGradient></defs><circle cx="40" cy="40" r="40" fill="url(#check)"/></svg>"##;
