use gpui_kit::component::IconName;

use super::TableTreeSection;

/// Selects the compact object-tree icon for each table metadata section.
pub(super) fn section_icon(section: TableTreeSection) -> IconName {
    match section {
        TableTreeSection::Keys | TableTreeSection::Indexes => IconName::File,
        TableTreeSection::ForeignKeys => IconName::ArrowRight,
        TableTreeSection::Triggers => IconName::Network,
    }
}
