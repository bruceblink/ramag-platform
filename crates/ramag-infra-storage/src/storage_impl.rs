#[async_trait]
impl Storage for RedbStorage {
    async fn append_api_history(
        &self,
        workspace_id: &ApiWorkspaceId,
        record: &ApiHistoryRecord,
    ) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let workspace_id = workspace_id.clone();
        let record = record.clone();
        run_blocking(move || repos::api_history_repo::append(db, cipher, workspace_id, record))
            .await
    }

    async fn list_api_history(
        &self,
        workspace_id: &ApiWorkspaceId,
        limit: usize,
    ) -> Result<Vec<ApiHistoryRecord>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let workspace_id = workspace_id.clone();
        run_blocking(move || repos::api_history_repo::list(db, cipher, workspace_id, limit)).await
    }

    async fn clear_api_history(&self, workspace_id: &ApiWorkspaceId) -> Result<()> {
        let db = self.db.clone();
        let workspace_id = workspace_id.clone();
        run_blocking(move || repos::api_history_repo::clear(db, workspace_id)).await
    }

    async fn list_api_workspaces(&self) -> Result<Vec<ApiWorkspace>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::api_workspace_repo::list(db, cipher)).await
    }

    async fn get_api_workspace(&self, id: &ApiWorkspaceId) -> Result<Option<ApiWorkspace>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id = id.to_string();
        run_blocking(move || repos::api_workspace_repo::get(db, cipher, id)).await
    }

    async fn save_api_workspace(&self, workspace: &ApiWorkspace) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let workspace = workspace.clone();
        run_blocking(move || repos::api_workspace_repo::save(db, cipher, workspace)).await
    }

    async fn delete_api_workspace(&self, id: &ApiWorkspaceId) -> Result<()> {
        let db = self.db.clone();
        let id = id.to_string();
        run_blocking(move || repos::api_workspace_repo::delete(db, id)).await
    }

    async fn save_collaboration_share(&self, share: &CollaborationShare) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let share = share.clone();
        run_blocking(move || repos::collaboration_repo::save(db, cipher, share)).await
    }

    async fn list_collaboration_shares(&self) -> Result<Vec<CollaborationShare>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::collaboration_repo::list(db, cipher)).await
    }

    async fn get_collaboration_share(
        &self,
        id: &CollaborationShareId,
    ) -> Result<Option<CollaborationShare>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id = id.clone();
        run_blocking(move || repos::collaboration_repo::get(db, cipher, id)).await
    }

    async fn delete_collaboration_share(&self, id: &CollaborationShareId) -> Result<()> {
        let db = self.db.clone();
        let id = id.clone();
        run_blocking(move || repos::collaboration_repo::delete(db, id)).await
    }

    async fn list_mqtt_profiles(&self) -> Result<Vec<MqttProfile>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::mqtt_profile_repo::list(db, cipher)).await
    }

    async fn get_mqtt_profile(&self, id: &MqttProfileId) -> Result<Option<MqttProfile>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id = id.to_string();
        run_blocking(move || repos::mqtt_profile_repo::get(db, cipher, id)).await
    }

    async fn save_mqtt_profile(&self, profile: &MqttProfile) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let profile = profile.clone();
        run_blocking(move || repos::mqtt_profile_repo::save(db, cipher, profile)).await
    }

    async fn delete_mqtt_profile(&self, id: &MqttProfileId) -> Result<()> {
        let db = self.db.clone();
        let id = id.to_string();
        run_blocking(move || repos::mqtt_profile_repo::delete(db, id)).await
    }

    async fn list_kafka_clusters(&self) -> Result<Vec<KafkaClusterConfig>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::kafka_cluster_repo::list(db, cipher)).await
    }

    async fn get_kafka_cluster(&self, id: &KafkaClusterId) -> Result<Option<KafkaClusterConfig>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id = id.to_string();
        run_blocking(move || repos::kafka_cluster_repo::get(db, cipher, id)).await
    }

    async fn save_kafka_cluster(&self, config: &KafkaClusterConfig) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let config = config.clone();
        run_blocking(move || repos::kafka_cluster_repo::save(db, cipher, config)).await
    }

    async fn delete_kafka_cluster(&self, id: &KafkaClusterId) -> Result<()> {
        let db = self.db.clone();
        let id = id.to_string();
        run_blocking(move || repos::kafka_cluster_repo::delete(db, id)).await
    }

    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::connection_repo::list(db, cipher)).await
    }

    async fn get_connection(&self, id: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id_str = id.to_string();
        run_blocking(move || repos::connection_repo::get(db, cipher, id_str)).await
    }

    async fn save_connection(&self, config: &ConnectionConfig) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let config = config.clone();
        run_blocking(move || repos::connection_repo::save(db, cipher, config)).await
    }

    async fn save_connections(&self, configs: &[ConnectionConfig]) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let configs = configs.to_vec();
        run_blocking(move || repos::connection_repo::save_many(db, cipher, configs)).await
    }

    async fn delete_connection(&self, id: &ConnectionId) -> Result<()> {
        let db = self.db.clone();
        let id_str = id.to_string();
        run_blocking(move || repos::connection_repo::delete(db, id_str)).await
    }

    async fn list_ssh_profiles(&self) -> Result<Vec<SshProfile>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::ssh_profile_repo::list(db, cipher)).await
    }

    async fn get_ssh_profile(&self, id: &SshProfileId) -> Result<Option<SshProfile>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id = id.to_string();
        run_blocking(move || repos::ssh_profile_repo::get(db, cipher, id)).await
    }

    async fn save_ssh_profile(&self, profile: &SshProfile) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let profile = profile.clone();
        run_blocking(move || repos::ssh_profile_repo::save(db, cipher, profile)).await
    }

    async fn delete_ssh_profile(&self, id: &SshProfileId) -> Result<()> {
        let db = self.db.clone();
        let id = id.clone();
        run_blocking(move || repos::ssh_profile_repo::delete(db, id)).await
    }

    async fn list_object_storage_accounts(&self) -> Result<Vec<ObjectStorageAccount>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::object_storage_account_repo::list(db, cipher)).await
    }

    async fn get_object_storage_account(
        &self,
        id: &ObjectStorageAccountId,
    ) -> Result<Option<ObjectStorageAccount>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id = id.to_string();
        run_blocking(move || repos::object_storage_account_repo::get(db, cipher, id)).await
    }

    async fn save_object_storage_account(&self, account: &ObjectStorageAccount) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let account = account.clone();
        run_blocking(move || repos::object_storage_account_repo::save(db, cipher, account)).await
    }

    async fn delete_object_storage_account(
        &self,
        id: &ObjectStorageAccountId,
        workspace_preference_key: &str,
    ) -> Result<()> {
        let db = self.db.clone();
        let id = id.clone();
        let preference_key = workspace_preference_key.to_string();
        run_blocking(move || {
            repos::object_storage_account_repo::delete_with_preference(db, id, preference_key)
        })
        .await
    }

    async fn list_repos(&self) -> Result<Vec<RepoConfig>> {
        let db = self.db.clone();
        run_blocking(move || repos::repo_repo::list(db)).await
    }

    async fn save_repo(&self, config: &RepoConfig) -> Result<()> {
        let db = self.db.clone();
        let config = config.clone();
        run_blocking(move || repos::repo_repo::save(db, config)).await
    }

    async fn delete_repo(&self, id: &RepoId) -> Result<()> {
        let db = self.db.clone();
        let id = id.clone();
        run_blocking(move || repos::repo_repo::delete(db, id)).await
    }

    async fn append_history(&self, record: &QueryRecord) -> Result<()> {
        let db = self.db.clone();
        let record = record.clone();
        run_blocking(move || repos::history_repo::append(db, record)).await
    }

    async fn list_history(
        &self,
        connection_id: Option<&ConnectionId>,
        limit: usize,
    ) -> Result<Vec<QueryRecord>> {
        let db = self.db.clone();
        let conn_filter = connection_id.cloned();
        run_blocking(move || repos::history_repo::list(db, conn_filter, limit)).await
    }

    async fn list_history_bounded(
        &self,
        connection_id: Option<&ConnectionId>,
        limit: usize,
        max_inline_bytes: u64,
    ) -> Result<QueryHistoryPage> {
        let db = self.db.clone();
        let conn_filter = connection_id.cloned();
        run_blocking(move || {
            repos::history_repo::list_bounded(db, conn_filter, limit, max_inline_bytes)
        })
        .await
    }

    async fn delete_history(&self, id: &QueryRecordId) -> Result<()> {
        let db = self.db.clone();
        let id = id.clone();
        run_blocking(move || repos::history_repo::delete(db, id)).await
    }

    async fn clear_history(&self, connection_id: Option<&ConnectionId>) -> Result<()> {
        let db = self.db.clone();
        let conn_filter = connection_id.cloned();
        run_blocking(move || repos::history_repo::clear(db, conn_filter)).await
    }

    async fn get_preference(&self, key: &str) -> Result<Option<String>> {
        let db = self.db.clone();
        let key_owned = key.to_string();
        let result = run_blocking(move || repos::prefs_repo::get(db, key_owned)).await;
        if let Err(error) = &result {
            warn!(operation = "storage_preference_load", error = %error, preference = key, "load preference failed");
        }
        result
    }

    async fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        let db = self.db.clone();
        let key_owned = key.to_string();
        let value = value.to_string();
        let result = run_blocking(move || repos::prefs_repo::set(db, key_owned, value)).await;
        match &result {
            Ok(()) => debug!(
                operation = "storage_preference_save",
                preference = key,
                "preference saved"
            ),
            Err(error) => {
                warn!(operation = "storage_preference_save", error = %error, preference = key, "save preference failed")
            }
        }
        result
    }

    async fn delete_preference(&self, key: &str) -> Result<()> {
        let db = self.db.clone();
        let key_owned = key.to_string();
        let result = run_blocking(move || repos::prefs_repo::delete(db, key_owned)).await;
        match &result {
            Ok(()) => debug!(
                operation = "storage_preference_delete",
                preference = key,
                "preference deleted"
            ),
            Err(error) => {
                warn!(operation = "storage_preference_delete", error = %error, preference = key, "delete preference failed")
            }
        }
        result
    }

    async fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
        let cipher = self.cipher.clone();
        let plain = plain.to_vec();
        run_blocking(move || cipher.read().encrypt_bytes(&plain)).await
    }

    async fn unseal(&self, cipher_blob: &[u8]) -> Result<Vec<u8>> {
        let cipher = self.cipher.clone();
        let blob = cipher_blob.to_vec();
        run_blocking(move || cipher.read().decrypt_bytes(&blob)).await
    }

    async fn clip_save(&self, item: &ClipItem) -> Result<()> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let item = item.clone();
        run_blocking(move || repos::clip_repo::save(db, cipher, item)).await
    }

    async fn clip_get(&self, id: &ClipId) -> Result<Option<ClipItem>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let id = id.to_string();
        run_blocking(move || repos::clip_repo::get(db, cipher, id)).await
    }

    async fn clip_list(&self) -> Result<Vec<ClipItem>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::clip_repo::list(db, cipher)).await
    }

    async fn clip_media_paths(&self) -> Result<Vec<String>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::clip_repo::media_paths(db, cipher)).await
    }

    async fn clip_list_recent(&self, limit: usize) -> Result<Vec<ClipItem>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::clip_repo::list_recent(db, cipher, limit)).await
    }

    async fn clip_list_recent_bounded(
        &self,
        limit: usize,
        max_inline_bytes: u64,
    ) -> Result<Vec<ClipItem>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || {
            repos::clip_repo::list_recent_bounded(db, cipher, limit, max_inline_bytes)
        })
        .await
    }

    async fn clip_search(&self, query: &str, limit: usize) -> Result<Vec<ClipItem>> {
        validate_clip_search_query(query)?;
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let query = query.to_string();
        run_blocking(move || repos::clip_repo::search(db, cipher, query, limit)).await
    }

    async fn clip_search_cancellable(
        &self,
        query: &str,
        limit: usize,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<ClipItem>> {
        validate_clip_search_query(query)?;
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let query = query.to_string();
        run_blocking(move || {
            repos::clip_repo::search_cancellable(db, cipher, query, limit, cancelled)
        })
        .await
    }

    async fn clip_search_cancellable_bounded(
        &self,
        query: &str,
        limit: usize,
        max_inline_bytes: u64,
        cancelled: Arc<AtomicBool>,
    ) -> Result<ClipSearchResult> {
        validate_clip_search_query(query)?;
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let query = query.to_string();
        run_blocking(move || {
            repos::clip_repo::search_cancellable_bounded(
                db,
                cipher,
                query,
                limit,
                max_inline_bytes,
                cancelled,
            )
        })
        .await
    }

    async fn clip_delete(&self, id: &ClipId) -> Result<()> {
        let db = self.db.clone();
        let id_str = id.to_string();
        run_blocking(move || repos::clip_repo::delete(db, id_str)).await
    }

    async fn clip_find_by_hash(&self, hash: &str) -> Result<Option<ClipItem>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        let hash = hash.to_string();
        run_blocking(move || repos::clip_repo::find_by_hash(db, cipher, hash)).await
    }

    async fn clip_clear(&self) -> Result<()> {
        let db = self.db.clone();
        run_blocking(move || repos::clip_repo::clear(db)).await
    }

    async fn clip_prune(&self, max_items: u32, max_age_days: u32) -> Result<Vec<String>> {
        let db = self.db.clone();
        let cipher = self.cipher.clone();
        run_blocking(move || repos::clip_repo::prune(db, cipher, max_items, max_age_days)).await
    }
}