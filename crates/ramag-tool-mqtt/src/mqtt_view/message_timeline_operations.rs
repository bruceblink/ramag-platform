impl MqttView {
    /// Pauses only the current window's message rendering; the MQTT subscription
    /// remains connected and the driver continues to honor its bounded sink.
    fn toggle_message_timeline_pause(&mut self) {
        if self.subscription_running && !self.subscription_stopping {
            self.message_timeline_paused = !self.message_timeline_paused;
        }
    }

    /// Clears the local timeline without sending a command to the Broker or
    /// changing the retained-message state.
    fn clear_message_timeline(&mut self) {
        self.messages.clear();
    }

    fn append_received_message(&mut self, message: MqttMessage) -> bool {
        append_timeline_message(
            &mut self.messages,
            self.message_timeline_paused,
            message,
        )
    }
}

fn append_timeline_message(
    messages: &mut std::collections::VecDeque<MqttMessage>,
    paused: bool,
    message: MqttMessage,
) -> bool {
    if paused {
        return false;
    }
    if messages.len() >= MAX_MESSAGES {
        messages.pop_front();
    }
    messages.push_back(message);
    true
}
