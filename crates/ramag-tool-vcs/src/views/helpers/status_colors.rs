use super::{FileTabSource, GroupKind};

/// Git 工作区共用的状态颜色；展示层从当前主题创建，避免各处维护固定色值。
#[derive(Clone, Copy)]
pub(in crate::views) struct GitStatusColors {
    warning: gpui_kit::Hsla,
    success: gpui_kit::Hsla,
    danger: gpui_kit::Hsla,
    info: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
}

impl GitStatusColors {
    pub(in crate::views) fn from_theme(theme: &gpui_kit::component::Theme) -> Self {
        Self {
            warning: theme.warning,
            success: theme.success,
            danger: theme.danger,
            info: theme.info,
            accent: theme.accent,
        }
    }

    pub(in crate::views) fn group(
        self,
        kind: GroupKind,
        fallback: gpui_kit::Hsla,
    ) -> gpui_kit::Hsla {
        match kind {
            GroupKind::Conflict => self.danger,
            GroupKind::Staged => self.accent,
            GroupKind::Unstaged => self.warning,
            GroupKind::Untracked => fallback,
        }
    }

    pub(in crate::views) fn letter(self, code: &str, fallback: gpui_kit::Hsla) -> gpui_kit::Hsla {
        match code {
            "M" => self.warning,
            "A" => self.success,
            "D" | "U" => self.danger,
            "R" | "C" | "T" => self.info,
            _ => fallback,
        }
    }

    pub(in crate::views) fn tag(self) -> gpui_kit::Hsla {
        self.warning
    }

    pub(in crate::views) fn remote(self) -> gpui_kit::Hsla {
        self.info
    }

    pub(in crate::views) fn file_tab(
        self,
        source: &FileTabSource,
        fallback: gpui_kit::Hsla,
    ) -> gpui_kit::Hsla {
        match source {
            FileTabSource::Changes(kind) => self.group(*kind, fallback),
            FileTabSource::ProjectFiles => self.info,
            FileTabSource::Commit { .. } => self.accent,
            FileTabSource::Compare { .. } => self.success,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GitStatusColors;
    use crate::views::helpers::{FileTabSource, GroupKind};

    fn colors() -> GitStatusColors {
        GitStatusColors {
            warning: gpui_kit::hsla(0.11, 0.61, 0.51, 1.0),
            success: gpui_kit::hsla(0.22, 0.62, 0.52, 1.0),
            danger: gpui_kit::hsla(0.33, 0.63, 0.53, 1.0),
            info: gpui_kit::hsla(0.44, 0.64, 0.54, 1.0),
            accent: gpui_kit::hsla(0.55, 0.65, 0.55, 1.0),
        }
    }

    #[test]
    fn change_letters_use_theme_status_colors() {
        let colors = colors();
        let fallback = gpui_kit::hsla(0.66, 0.66, 0.56, 1.0);

        assert_eq!(colors.letter("M", fallback), colors.warning);
        assert_eq!(colors.letter("A", fallback), colors.success);
        assert_eq!(colors.letter("D", fallback), colors.danger);
        assert_eq!(colors.letter("U", fallback), colors.danger);
        for code in ["R", "C", "T"] {
            assert_eq!(colors.letter(code, fallback), colors.info);
        }
        assert_eq!(colors.letter("?", fallback), fallback);
    }

    #[test]
    fn change_groups_use_theme_status_colors() {
        let colors = colors();
        let fallback = gpui_kit::hsla(0.66, 0.66, 0.56, 1.0);

        assert_eq!(colors.group(GroupKind::Staged, fallback), colors.accent);
        assert_eq!(colors.group(GroupKind::Unstaged, fallback), colors.warning);
        assert_eq!(colors.group(GroupKind::Conflict, fallback), colors.danger);
        assert_eq!(colors.group(GroupKind::Untracked, fallback), fallback);
    }

    #[test]
    fn sidebar_reference_icons_use_theme_semantic_colors() {
        let colors = colors();

        assert_eq!(colors.tag(), colors.warning);
        assert_eq!(colors.remote(), colors.info);
    }

    #[test]
    fn file_tab_sources_use_theme_semantic_colors() {
        let colors = colors();
        let fallback = gpui_kit::hsla(0.66, 0.66, 0.56, 1.0);

        assert_eq!(
            colors.file_tab(&FileTabSource::ProjectFiles, fallback),
            colors.info
        );
        assert_eq!(
            colors.file_tab(
                &FileTabSource::Commit {
                    commit_id: "abc1234".into(),
                    change_kind: None,
                },
                fallback,
            ),
            colors.accent
        );
        assert_eq!(
            colors.file_tab(
                &FileTabSource::Compare {
                    from: "main".into(),
                    to: "feature/ui".into(),
                },
                fallback,
            ),
            colors.success
        );
        assert_eq!(
            colors.file_tab(&FileTabSource::Changes(GroupKind::Untracked), fallback),
            fallback
        );
    }
}
