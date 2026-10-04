//! 屏幕上的矩形：单位由调用方定（点或比例），y 向下。

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,

    pub y: f32,

    pub width: f32,

    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// 两个矩形的外包框。
    pub fn union(&self, other: &Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: self.right().max(other.right()) - x,
            height: self.bottom().max(other.bottom()) - y,
        }
    }

    /// 点是否在矩形里；`slack` 往四周多算这么多，鼠标稍微偏一点也算指着它。
    pub fn contains(&self, px: f32, py: f32, slack: f32) -> bool {
        px >= self.x - slack
            && px <= self.right() + slack
            && py >= self.y - slack
            && py <= self.bottom() + slack
    }
}
