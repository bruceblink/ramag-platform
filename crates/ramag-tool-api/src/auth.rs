//! API HTTP/gRPC 认证编辑器；认证密钥只在工作区执行副本和加密存储中流转。

use super::*;
use gpui_kit::ClickEvent;
use ramag_domain::entities::{ApiAuth, ApiKeyLocation, ApiOAuth2Config};
use ramag_ui::PointerDropdownMenu as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthKind {
    None,
    Basic,
    Bearer,
    OAuth2,
    ApiKey,
}

impl AuthKind {
    const ALL: [Self; 5] = [
        Self::None,
        Self::Basic,
        Self::Bearer,
        Self::OAuth2,
        Self::ApiKey,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Basic => "Basic",
            Self::Bearer => "Bearer",
            Self::OAuth2 => "OAuth2 Client Credentials",
            Self::ApiKey => "API Key",
        }
    }
}

pub(crate) struct ApiAuthEditor {
    pub(crate) kind: AuthKind,
    username: Entity<InputState>,
    password: Entity<InputState>,
    bearer_token: Entity<InputState>,
    token_url: Entity<InputState>,
    client_id: Entity<InputState>,
    client_secret: Entity<InputState>,
    scope: Entity<InputState>,
    api_key_name: Entity<InputState>,
    api_key_value: Entity<InputState>,
    api_key_location: ApiKeyLocation,
}

impl ApiAuthEditor {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ApiView>) -> Self {
        Self {
            kind: AuthKind::None,
            username: super::api_input(window, cx, "用户名", ""),
            password: masked_input(window, cx, "密码"),
            bearer_token: masked_input(window, cx, "Bearer Token"),
            token_url: super::api_input(window, cx, "Token URL", ""),
            client_id: super::api_input(window, cx, "Client ID", ""),
            client_secret: masked_input(window, cx, "Client Secret"),
            scope: super::api_input(window, cx, "Scope（可选）", ""),
            api_key_name: super::api_input(window, cx, "Key 名称", ""),
            api_key_value: masked_input(window, cx, "Key 值"),
            api_key_location: ApiKeyLocation::Header,
        }
    }

    #[cfg(test)]
    pub(crate) fn label(&self) -> &'static str {
        self.kind.label()
    }

    pub(crate) fn select_kind(
        &mut self,
        kind: AuthKind,
        window: &mut Window,
        cx: &mut Context<ApiView>,
    ) {
        self.kind = kind;
        for field in [
            &self.username,
            &self.password,
            &self.bearer_token,
            &self.token_url,
            &self.client_id,
            &self.client_secret,
            &self.scope,
            &self.api_key_name,
            &self.api_key_value,
        ] {
            field.update(cx, |input, cx| input.set_value(String::new(), window, cx));
        }
    }

    pub(crate) fn to_auth(&self, cx: &App) -> Result<ApiAuth> {
        Ok(match self.kind {
            AuthKind::None => ApiAuth::None,
            AuthKind::Basic => ApiAuth::Basic {
                username: super::input_value(&self.username, cx),
                password: super::input_value(&self.password, cx),
            },
            AuthKind::Bearer => ApiAuth::Bearer {
                token: super::input_value(&self.bearer_token, cx),
            },
            AuthKind::OAuth2 => ApiAuth::OAuth2 {
                config: ApiOAuth2Config {
                    token_url: super::input_value(&self.token_url, cx),
                    client_id: super::input_value(&self.client_id, cx),
                    client_secret: super::input_value(&self.client_secret, cx),
                    scope: optional_value(&self.scope, cx),
                },
            },
            AuthKind::ApiKey => ApiAuth::ApiKey {
                name: super::input_value(&self.api_key_name, cx),
                value: super::input_value(&self.api_key_value, cx),
                location: self.api_key_location,
            },
        })
    }

    pub(crate) fn set_auth(
        &mut self,
        auth: &ApiAuth,
        window: &mut Window,
        cx: &mut Context<ApiView>,
    ) {
        self.select_kind(
            match auth {
                ApiAuth::None => AuthKind::None,
                ApiAuth::Basic { .. } => AuthKind::Basic,
                ApiAuth::Bearer { .. } => AuthKind::Bearer,
                ApiAuth::OAuth2 { .. } => AuthKind::OAuth2,
                ApiAuth::ApiKey { .. } => AuthKind::ApiKey,
            },
            window,
            cx,
        );
        match auth {
            ApiAuth::None => {}
            ApiAuth::Basic { username, password } => {
                set_input(&self.username, username, window, cx);
                set_input(&self.password, password, window, cx);
            }
            ApiAuth::Bearer { token } => set_input(&self.bearer_token, token, window, cx),
            ApiAuth::OAuth2 { config } => {
                set_input(&self.token_url, &config.token_url, window, cx);
                set_input(&self.client_id, &config.client_id, window, cx);
                set_input(&self.client_secret, &config.client_secret, window, cx);
                set_input(
                    &self.scope,
                    config.scope.as_deref().unwrap_or_default(),
                    window,
                    cx,
                );
            }
            ApiAuth::ApiKey {
                name,
                value,
                location,
            } => {
                self.api_key_location = *location;
                set_input(&self.api_key_name, name, window, cx);
                set_input(&self.api_key_value, value, window, cx);
            }
        }
    }

    pub(crate) fn render(
        &self,
        cx: &mut Context<ApiView>,
        theme: &gpui_kit::component::Theme,
    ) -> gpui_kit::AnyElement {
        let current = self.kind;
        let view = cx.entity();
        let selector = ramag_ui::clickable_button("api-auth-kind")
            .debug_selector(|| "api-auth-kind".into())
            .xsmall()
            .outline()
            .dropdown_caret(true)
            .label(current.label())
            .pointer_dropdown_menu(move |mut menu, _, _| {
                for kind in AuthKind::ALL {
                    let view = view.clone();
                    menu = menu.item(
                        ramag_ui::menu_item(kind.label())
                            .checked(kind == current)
                            .on_click(move |_: &ClickEvent, window, app| {
                                view.update(app, |view, cx| {
                                    view.auth_editor.select_kind(kind, window, cx);
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            });
        let fields = match current {
            AuthKind::None => div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("不发送认证信息"),
            AuthKind::Basic => row()
                .child(field("用户名", Input::new(&self.username).small()))
                .child(field(
                    "密码",
                    Input::new(&self.password).small().mask_toggle(),
                )),
            AuthKind::Bearer => v_flex().gap(px(5.0)).child(field(
                "Bearer Token",
                Input::new(&self.bearer_token).small().mask_toggle(),
            )),
            AuthKind::OAuth2 => v_flex()
                .gap(px(6.0))
                .child(row().child(field("Token URL", Input::new(&self.token_url).small())))
                .child(
                    row()
                        .child(field("Client ID", Input::new(&self.client_id).small()))
                        .child(field(
                            "Client Secret",
                            Input::new(&self.client_secret).small().mask_toggle(),
                        )),
                )
                .child(field("Scope", Input::new(&self.scope).small())),
            AuthKind::ApiKey => {
                let location = self.api_key_location;
                let view = cx.entity();
                let location_selector = ramag_ui::clickable_button("api-auth-key-location")
                    .debug_selector(|| "api-auth-key-location".into())
                    .xsmall()
                    .outline()
                    .dropdown_caret(true)
                    .label(match location {
                        ApiKeyLocation::Header => "Header",
                        ApiKeyLocation::Query => "Query",
                    })
                    .pointer_dropdown_menu(move |mut menu, _, _| {
                        for (value, label) in [
                            (ApiKeyLocation::Header, "Header"),
                            (ApiKeyLocation::Query, "Query"),
                        ] {
                            let view = view.clone();
                            menu = menu.item(
                                ramag_ui::menu_item(label)
                                    .checked(value == location)
                                    .on_click(move |_: &ClickEvent, _, app| {
                                        view.update(app, |view, cx| {
                                            view.auth_editor.api_key_location = value;
                                            cx.notify();
                                        });
                                    }),
                            );
                        }
                        menu
                    });
                row()
                    .child(field("Key 名称", Input::new(&self.api_key_name).small()))
                    .child(field(
                        "Key 值",
                        Input::new(&self.api_key_value).small().mask_toggle(),
                    ))
                    .child(location_selector)
            }
        };
        v_flex()
            .id("api-auth-editor")
            .debug_selector(|| "api-auth-editor".into())
            .w_full()
            .min_w_0()
            .gap(px(6.0))
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Auth"),
                    )
                    .child(selector),
            )
            .child(fields)
            .into_any_element()
    }
}

fn optional_value(field: &Entity<InputState>, cx: &App) -> Option<String> {
    let value = super::input_value(field, cx);
    (!value.is_empty()).then_some(value)
}

fn masked_input(
    window: &mut Window,
    cx: &mut Context<ApiView>,
    placeholder: &'static str,
) -> Entity<InputState> {
    let field = super::api_input(window, cx, placeholder, "");
    field.update(cx, |state, cx| state.set_masked(true, window, cx));
    field
}

fn set_input(
    field: &Entity<InputState>,
    value: &str,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) {
    field.update(cx, |input, cx| {
        input.set_value(value.to_string(), window, cx)
    });
}
