//! # コントローラー
//!
//! NESの標準コントローラー入力を管理する。

/// コントローラーのボタン
#[derive(Debug, Clone, Copy)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right,
}

/// コントローラー
#[derive(Debug)]
pub struct Controller {
    /// 各ボタンの状態（押されている=true）
    buttons: u8,
    /// シフトレジスタ
    shift_register: u8,
    /// ストローブモード
    strobe: bool,
}

impl Controller {
    pub fn new() -> Self {
        Self {
            buttons: 0,
            shift_register: 0,
            strobe: false,
        }
    }

    /// ボタンを押す
    pub fn press(&mut self, button: Button) {
        let bit = Self::button_to_bit(button);
        self.buttons |= bit;
    }

    /// ボタンを離す
    pub fn release(&mut self, button: Button) {
        let bit = Self::button_to_bit(button);
        self.buttons &= !bit;
    }

    /// ボタンの状態を設定
    pub fn set_button(&mut self, button: Button, pressed: bool) {
        if pressed {
            self.press(button);
        } else {
            self.release(button);
        }
    }

    /// CPU側からの読み込み（$4016）
    pub fn read(&mut self) -> u8 {
        if self.strobe {
            // ストローブモードの時は常にAボタンの状態を返す
            (self.buttons & 0x01) | 0x40
        } else {
            let value = (self.shift_register & 0x01) | 0x40;
            self.shift_register >>= 1;
            self.shift_register |= 0x80; // パディング
            value
        }
    }

    /// CPU側からの書き込み（$4016）
    pub fn write(&mut self, data: u8) {
        let new_strobe = (data & 0x01) != 0;
        
        // ストローブの立ち下がりエッジでシフトレジスタをロード
        if self.strobe && !new_strobe {
            self.shift_register = self.buttons;
        }
        
        self.strobe = new_strobe;
        
        // ストローブがセットされている間は常にリロード
        if self.strobe {
            self.shift_register = self.buttons;
        }
    }

    fn button_to_bit(button: Button) -> u8 {
        match button {
            Button::A => 0b0000_0001,      // Bit 0
            Button::B => 0b0000_0010,      // Bit 1
            Button::Select => 0b0000_0100, // Bit 2
            Button::Start => 0b0000_1000,  // Bit 3
            Button::Up => 0b0001_0000,     // Bit 4
            Button::Down => 0b0010_0000,   // Bit 5
            Button::Left => 0b0100_0000,   // Bit 6
            Button::Right => 0b1000_0000,  // Bit 7
        }
    }
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_press() {
        let mut controller = Controller::new();
        controller.press(Button::A);
        assert_ne!(controller.buttons & 0b0000_0001, 0);
    }

    #[test]
    fn test_button_release() {
        let mut controller = Controller::new();
        controller.press(Button::A);
        controller.release(Button::A);
        assert_eq!(controller.buttons & 0b0000_0001, 0);
    }
}
