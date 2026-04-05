#[derive(Debug, Clone, PartialEq)]
pub enum ConfigKey {
    FileSystem,
    UserSystem,
    Panelz,
}

impl ConfigKey {
    // 定义一个静态数组包含所有成员
    pub const ALL: [Self; 3] = [Self::FileSystem, Self::UserSystem, Self::Panelz];

    pub fn path(&self) -> &'static str {
        match self {
            Self::FileSystem => "etc/fsystem.json",
            Self::UserSystem => "etc/user.json",
            Self::Panelz => "etc/panelz.json",
        }
    }
}
