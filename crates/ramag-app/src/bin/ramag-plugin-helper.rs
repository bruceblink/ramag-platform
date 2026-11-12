//! 进程外插件协议的跨平台测试辅助进程，不属于 Ramag 主程序入口。

use ramag_app::{PluginProcessMessage, PluginReady, PluginResponse, read_frame, write_frame};

#[tokio::main]
async fn main() {
    let mut reader = tokio::io::stdin();
    let mut writer = tokio::io::stdout();
    loop {
        let message = match read_frame(&mut reader).await {
            Ok(message) => message,
            Err(_) => return,
        };
        match message {
            PluginProcessMessage::Hello(hello) => {
                if write_frame(
                    &mut writer,
                    &PluginProcessMessage::Ready(PluginReady {
                        protocol_version: hello.protocol_version,
                        plugin_id: hello.plugin_id,
                        api_version: hello.api_version,
                        capabilities: hello.granted_capabilities,
                    }),
                )
                .await
                .is_err()
                {
                    return;
                }
            }
            PluginProcessMessage::Invoke(invoke) => {
                if write_frame(
                    &mut writer,
                    &PluginProcessMessage::Response(PluginResponse {
                        request_id: invoke.request_id,
                        ok: true,
                        output: invoke.input,
                        error: None,
                    }),
                )
                .await
                .is_err()
                {
                    return;
                }
            }
            PluginProcessMessage::Shutdown(_) => return,
            _ => return,
        }
    }
}
