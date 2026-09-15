    use super::*;

    fn record() -> KafkaMessageRecord {
        KafkaMessageRecord {
            topic: "events".into(),
            partition: 0,
            offset: 1,
            timestamp: None,
            key: Some(b"order-42".to_vec()),
            value: Some(b"Order Created".to_vec()),
            headers: vec![KafkaMessageHeader {
                key: "trace-id".into(),
                value: Some(b"abc-99".to_vec()),
            }],
        }
    }

    #[test]
    fn literal_search_remains_case_insensitive_and_field_scoped() -> Result<()> {
        let scan = KafkaMessageQuery::by_offset("events", vec![0], 0, Some(2));
        let query = KafkaMessageSearchQuery::new("created", scan)
            .with_fields(vec![KafkaMessageSearchField::Value]);
        let matcher = MessageSearchMatcher::new(&query)?;
        assert!(message_matches(&record(), &matcher));

        let key_only = KafkaMessageSearchQuery::new("created", query.scan.clone())
            .with_fields(vec![KafkaMessageSearchField::Key]);
        let matcher = MessageSearchMatcher::new(&key_only)?;
        assert!(!message_matches(&record(), &matcher));
        Ok(())
    }

    #[test]
    fn regex_search_matches_keys_and_headers_without_changing_scan_bounds() -> Result<()> {
        let scan = KafkaMessageQuery::by_offset("events", vec![0], 0, Some(2));
        let key_query = KafkaMessageSearchQuery::new(r"^order-[0-9]+$", scan.clone())
            .with_fields(vec![KafkaMessageSearchField::Key])
            .with_mode(KafkaMessageSearchMode::Regex);
        let matcher = MessageSearchMatcher::new(&key_query)?;
        assert!(message_matches(&record(), &matcher));

        let header_query = KafkaMessageSearchQuery::new(r"^trace-", scan)
            .with_fields(vec![KafkaMessageSearchField::Headers])
            .with_mode(KafkaMessageSearchMode::Regex);
        let matcher = MessageSearchMatcher::new(&header_query)?;
        assert!(message_matches(&record(), &matcher));
        Ok(())
    }
