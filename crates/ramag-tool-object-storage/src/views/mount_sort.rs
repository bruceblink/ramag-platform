use gpui_kit::component::IconName;
use ramag_domain::entities::ObjectStorageMount;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MountSortColumn {
    Bucket,
    RootPath,
}

impl MountSortColumn {
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Bucket => "bucket",
            Self::RootPath => "root-path",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MountSort {
    pub column: MountSortColumn,
    pub ascending: bool,
}

/// Preserve region groups while sorting a visible mount list by one displayed column.
pub(super) fn sort_mounts(mounts: &mut [&ObjectStorageMount], sort: Option<MountSort>) {
    mounts.sort_by(|left, right| {
        let region_order = left.region.cmp(&right.region);
        if !region_order.is_eq() {
            return region_order;
        }
        let Some(sort) = sort else {
            return (&left.bucket, &left.root_prefix).cmp(&(&right.bucket, &right.root_prefix));
        };
        let ordering = match sort.column {
            MountSortColumn::Bucket => left.bucket.to_lowercase().cmp(&right.bucket.to_lowercase()),
            MountSortColumn::RootPath => left
                .root_prefix
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .cmp(
                    &right
                        .root_prefix
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase(),
                ),
        };
        if sort.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

pub(super) fn mount_sort_icon(sort: Option<MountSort>, column: MountSortColumn) -> IconName {
    match sort.filter(|sort| sort.column == column) {
        Some(sort) if sort.ascending => IconName::ArrowUp,
        Some(_) => IconName::ArrowDown,
        None => IconName::ChevronsUpDown,
    }
}

pub(super) fn mount_sort_description(
    sort: Option<MountSort>,
    column: MountSortColumn,
    label: &'static str,
) -> String {
    match sort.filter(|sort| sort.column == column) {
        Some(sort) if sort.ascending => format!("{label}，当前升序，点击切换为降序"),
        Some(_) => format!("{label}，当前降序，点击切换为升序"),
        None => format!("{label}，点击按此列排序"),
    }
}

pub(super) fn next_mount_sort(current: Option<MountSort>, column: MountSortColumn) -> MountSort {
    match current {
        Some(current) if current.column == column => MountSort {
            column,
            ascending: !current.ascending,
        },
        _ => MountSort {
            column,
            ascending: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MountSort, MountSortColumn, mount_sort_description, mount_sort_icon, next_mount_sort,
        sort_mounts,
    };
    use gpui_kit::component::IconName;
    use ramag_domain::entities::{
        CloudProvider, HttpsEndpoint, ObjectStorageAccountId, ObjectStorageMount,
        ObjectStorageMountId,
    };

    fn mount(region: &str, bucket: &str, root_prefix: Option<&str>) -> ObjectStorageMount {
        ObjectStorageMount {
            id: ObjectStorageMountId::new(),
            account_id: ObjectStorageAccountId::new(),
            bucket: bucket.into(),
            region: region.into(),
            endpoint: HttpsEndpoint::parse_official(
                CloudProvider::AliyunOss,
                "https://oss-cn-hangzhou.aliyuncs.com",
            )
            .expect("test endpoint should be valid"),
            root_prefix: root_prefix.map(str::to_owned),
            created_at: None,
            storage_class: None,
        }
    }

    fn labels(mounts: &[&ObjectStorageMount]) -> Vec<String> {
        mounts
            .iter()
            .map(|mount| {
                format!(
                    "{}:{}:{}",
                    mount.region,
                    mount.bucket,
                    mount.root_prefix.as_deref().unwrap_or("root")
                )
            })
            .collect()
    }

    #[test]
    fn mount_sort_keeps_region_groups_and_sorts_visible_columns_stably() {
        let source = [
            mount("cn-hangzhou", "zeta", None),
            mount("cn-hangzhou", "beta", Some("z/")),
            mount("cn-hangzhou", "Alpha", Some("a/")),
            mount("us-west", "aardvark", None),
        ];
        let mut rows: Vec<_> = source.iter().collect();

        sort_mounts(
            &mut rows,
            Some(next_mount_sort(None, MountSortColumn::Bucket)),
        );
        assert_eq!(
            labels(&rows),
            [
                "cn-hangzhou:Alpha:a/",
                "cn-hangzhou:beta:z/",
                "cn-hangzhou:zeta:root",
                "us-west:aardvark:root",
            ]
        );

        let bucket_ascending = next_mount_sort(None, MountSortColumn::Bucket);
        sort_mounts(
            &mut rows,
            Some(next_mount_sort(
                Some(bucket_ascending),
                MountSortColumn::Bucket,
            )),
        );
        assert_eq!(
            labels(&rows),
            [
                "cn-hangzhou:zeta:root",
                "cn-hangzhou:beta:z/",
                "cn-hangzhou:Alpha:a/",
                "us-west:aardvark:root",
            ]
        );

        sort_mounts(
            &mut rows,
            Some(next_mount_sort(None, MountSortColumn::RootPath)),
        );
        assert_eq!(
            labels(&rows),
            [
                "cn-hangzhou:zeta:root",
                "cn-hangzhou:Alpha:a/",
                "cn-hangzhou:beta:z/",
                "us-west:aardvark:root",
            ]
        );
    }

    #[test]
    fn mount_sort_headers_expose_direction_and_reset_when_the_column_changes() {
        let bucket = next_mount_sort(None, MountSortColumn::Bucket);
        assert!(matches!(
            mount_sort_icon(Some(bucket), MountSortColumn::Bucket),
            IconName::ArrowUp
        ));
        assert!(matches!(
            mount_sort_icon(Some(bucket), MountSortColumn::RootPath),
            IconName::ChevronsUpDown
        ));
        assert_eq!(
            mount_sort_description(Some(bucket), MountSortColumn::Bucket, "Bucket"),
            "Bucket，当前升序，点击切换为降序"
        );
        assert_eq!(
            next_mount_sort(
                Some(MountSort {
                    ascending: false,
                    ..bucket
                }),
                MountSortColumn::RootPath
            ),
            MountSort {
                column: MountSortColumn::RootPath,
                ascending: true,
            }
        );
    }
}
