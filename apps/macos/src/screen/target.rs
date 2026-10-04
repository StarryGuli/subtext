//! 屏幕阅读的目标：一个窗口，或屏幕上的一块区域。

use subtext_coach::screen::Rect;

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// 按窗口号读：窗口被挡住、被拖动都不影响。`bundle` 是它所属应用的 bundle id，给「不处理的应用」名单用。
    Window {
        id: u32,

        owner: String,

        bundle: Option<String>,

        /// 开始时的位置；每次截屏前会重新查。
        bounds: Rect,
    },

    /// 屏幕上固定的一块（CG 坐标）。
    Region { bounds: Rect },
}

impl Target {
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Window { bounds, .. } | Self::Region { bounds } => *bounds,
        }
    }

    /// 给人看的名字：菜单与状态条里用。
    pub fn label(&self) -> String {
        match self {
            Self::Window { owner, .. } => owner.clone(),
            Self::Region { .. } => "框选区域".to_owned(),
        }
    }

    pub fn app(&self) -> Option<&str> {
        match self {
            Self::Window { bundle, owner, .. } => bundle.as_deref().or(Some(owner.as_str())),
            Self::Region { .. } => None,
        }
    }
}
