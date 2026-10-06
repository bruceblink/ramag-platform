use ramag_domain::entities::{CloudProvider, ObjectStorageAccount};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AccountSortColumn {
    Name,
    Provider,
    ReadOnly,
    BucketCount,
}

impl AccountSortColumn {
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Provider => "provider",
            Self::ReadOnly => "status",
            Self::BucketCount => "buckets",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AccountSort {
    pub column: AccountSortColumn,
    pub ascending: bool,
}

/// Start a new column in ascending order and toggle direction on repeated clicks.
pub(super) fn next_account_sort(
    current: Option<AccountSort>,
    column: AccountSortColumn,
) -> AccountSort {
    match current {
        Some(current) if current.column == column => AccountSort {
            column,
            ascending: !current.ascending,
        },
        _ => AccountSort {
            column,
            ascending: true,
        },
    }
}

/// Sort only the visible row copy so account identity, source order, and selection stay intact.
pub(super) fn sort_accounts(accounts: &mut [ObjectStorageAccount], sort: AccountSort) {
    accounts.sort_by(|left, right| {
        let ordering = match sort.column {
            AccountSortColumn::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            AccountSortColumn::Provider => {
                provider_rank(left.provider).cmp(&provider_rank(right.provider))
            }
            AccountSortColumn::ReadOnly => left.read_only.cmp(&right.read_only),
            AccountSortColumn::BucketCount => {
                left.manual_buckets.len().cmp(&right.manual_buckets.len())
            }
        };
        if sort.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

pub(super) fn account_sort_direction(
    sort: Option<AccountSort>,
    column: AccountSortColumn,
) -> Option<ramag_ui::SortDirection> {
    match sort.filter(|sort| sort.column == column) {
        Some(sort) if sort.ascending => Some(ramag_ui::SortDirection::Ascending),
        Some(_) => Some(ramag_ui::SortDirection::Descending),
        None => None,
    }
}

pub(super) fn account_sort_description(
    sort: Option<AccountSort>,
    column: AccountSortColumn,
    label: &'static str,
) -> String {
    match sort.filter(|sort| sort.column == column) {
        Some(sort) if sort.ascending => format!("{label}，当前升序，点击切换为降序"),
        Some(_) => format!("{label}，当前降序，点击切换为升序"),
        None => format!("{label}，点击按此列排序"),
    }
}

fn provider_rank(provider: CloudProvider) -> u8 {
    match provider {
        CloudProvider::AliyunOss => 0,
        CloudProvider::TencentCos => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountSort, AccountSortColumn, next_account_sort, sort_accounts};
    use ramag_domain::entities::{CloudProvider, ManualBucket, ObjectStorageAccount};

    fn account(
        name: &str,
        provider: CloudProvider,
        read_only: bool,
        buckets: usize,
    ) -> ObjectStorageAccount {
        let mut account = ObjectStorageAccount::new(name, provider);
        account.read_only = read_only;
        account.manual_buckets = (0..buckets)
            .map(|index| ManualBucket::new(format!("bucket-{index}"), "region"))
            .collect();
        account
    }

    fn names(accounts: &[ObjectStorageAccount]) -> Vec<&str> {
        accounts
            .iter()
            .map(|account| account.name.as_str())
            .collect()
    }

    #[test]
    fn sorting_starts_ascending_and_toggles_only_the_active_column() {
        let name = next_account_sort(None, AccountSortColumn::Name);
        assert_eq!(
            name,
            AccountSort {
                column: AccountSortColumn::Name,
                ascending: true,
            }
        );
        assert_eq!(
            next_account_sort(Some(name), AccountSortColumn::Name),
            AccountSort {
                column: AccountSortColumn::Name,
                ascending: false,
            }
        );
        assert_eq!(
            next_account_sort(
                Some(AccountSort {
                    ascending: false,
                    ..name
                }),
                AccountSortColumn::Provider,
            ),
            AccountSort {
                column: AccountSortColumn::Provider,
                ascending: true,
            }
        );
    }

    #[test]
    fn account_columns_sort_case_insensitively_numerically_and_stably() {
        let source_accounts = vec![
            account("beta", CloudProvider::TencentCos, true, 2),
            account("alpha", CloudProvider::AliyunOss, false, 1),
            account("Alpha", CloudProvider::AliyunOss, false, 1),
            account("gamma", CloudProvider::TencentCos, false, 0),
        ];

        let mut accounts = source_accounts.clone();
        sort_accounts(
            &mut accounts,
            next_account_sort(None, AccountSortColumn::Name),
        );
        assert_eq!(names(&accounts), ["alpha", "Alpha", "beta", "gamma"]);

        let mut accounts = source_accounts.clone();
        sort_accounts(
            &mut accounts,
            next_account_sort(None, AccountSortColumn::BucketCount),
        );
        assert_eq!(names(&accounts), ["gamma", "alpha", "Alpha", "beta"]);

        let mut accounts = source_accounts.clone();
        sort_accounts(
            &mut accounts,
            next_account_sort(None, AccountSortColumn::ReadOnly),
        );
        assert_eq!(names(&accounts), ["alpha", "Alpha", "gamma", "beta"]);

        let mut accounts = source_accounts;
        sort_accounts(
            &mut accounts,
            next_account_sort(None, AccountSortColumn::Provider),
        );
        assert_eq!(names(&accounts), ["alpha", "Alpha", "beta", "gamma"]);
    }

    #[test]
    fn descending_keeps_equal_rows_stable_and_reverses_numeric_order() {
        let mut accounts = vec![
            account("first", CloudProvider::TencentCos, false, 1),
            account("second", CloudProvider::TencentCos, false, 1),
            account("third", CloudProvider::AliyunOss, false, 3),
        ];
        let ascending = next_account_sort(None, AccountSortColumn::BucketCount);
        let descending = next_account_sort(Some(ascending), AccountSortColumn::BucketCount);
        sort_accounts(&mut accounts, descending);
        assert_eq!(names(&accounts), ["third", "first", "second"]);
    }
}
