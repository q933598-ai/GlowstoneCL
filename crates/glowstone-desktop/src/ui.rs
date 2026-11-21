use iced::widget::{
    button, canvas, column, container, image, row, rule, scrollable, stack, svg, text,
    text_input, Space,
};
use iced::{
    Alignment, Border, Color, ContentFit, Element, Length, Rectangle, Shadow, Vector,
};
use glowstone_core::{GameInstance, GamePlatform, InstanceState, LoaderKind};

use crate::auth::{self, DeviceCodePrompt, PollResult};
use crate::store::{self, AccountKind, AccountProfile, PersistedState};

const BG: Color = Color::from_rgb8(10, 15, 26);
const SIDEBAR: Color = Color::from_rgba(0.055, 0.085, 0.15, 0.94);
const PANEL: Color = Color::from_rgba(0.09, 0.13, 0.21, 0.88);
const PANEL_LIGHT: Color = Color::from_rgba(0.15, 0.20, 0.31, 0.72);
const TEXT: Color = Color::from_rgb8(242, 246, 252);
const MUTED: Color = Color::from_rgb8(164, 177, 198);
const SUBTLE: Color = Color::from_rgb8(119, 137, 164);
const CYAN: Color = Color::from_rgb8(105, 218, 235);
const CYAN_DARK: Color = Color::from_rgb8(17, 47, 64);
const PURPLE: Color = Color::from_rgb8(164, 133, 255);
const GREEN: Color = Color::from_rgb8(105, 225, 126);
const ORANGE: Color = Color::from_rgb8(255, 157, 94);
const RED: Color = Color::from_rgb8(255, 112, 125);
const EDGE: Color = Color::from_rgba(0.76, 0.86, 1.0, 0.16);
const EDGE_BRIGHT: Color = Color::from_rgba(0.43, 0.83, 1.0, 0.58);
const DIVIDER: Color = Color::from_rgba(0.72, 0.82, 0.95, 0.13);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Instances,
    Catalog,
    Modpacks,
    Downloads,
    Accounts,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformChoice {
    Java,
    Horizon,
}

impl From<PlatformChoice> for GamePlatform {
    fn from(value: PlatformChoice) -> Self {
        match value {
            PlatformChoice::Java => Self::Java,
            PlatformChoice::Horizon => Self::Horizon,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    SelectPlatform(PlatformChoice),
    SelectLoader(LoaderKind),
    SearchChanged(String),
    InstanceNameChanged(String),
    InstanceVersionChanged(String),
    CreateInstance,
    LaunchInstance(String),
    DeleteInstance(String),
    OfflineNameChanged(String),
    AddOfflineAccount,
    DeleteAccount(String),
    MicrosoftClientIdChanged(String),
    SaveMicrosoftClientId,
    StartMicrosoftLogin,
    DeviceCodeReady(Result<DeviceCodePrompt, String>),
    OpenMicrosoftVerification,
    PollMicrosoftLogin,
    MicrosoftPollFinished(Result<PollResult, String>),
    Info(&'static str),
}

#[derive(Debug)]
pub struct App {
    page: Page,
    platform: PlatformChoice,
    loader: LoaderKind,
    search: String,
    notice: Option<String>,
    art: image::Handle,
    data: PersistedState,
    instance_name: String,
    instance_version: String,
    offline_name: String,
    client_id: String,
    device_code: Option<DeviceCodePrompt>,
    auth_busy: bool,
}

impl Default for App {
    fn default() -> Self {
        let data = store::load();
        Self {
            page: Page::Home,
            platform: PlatformChoice::Java,
            loader: LoaderKind::Vanilla,
            search: String::new(),
            notice: None,
            art: image::Handle::from_bytes(&include_bytes!("../assets/hero.jpg")[..]),
            instance_name: String::new(),
            instance_version: String::new(),
            offline_name: String::new(),
            client_id: data.settings.microsoft_client_id.clone(),
            device_code: None,
            auth_busy: false,
            data,
        }
    }
}

pub fn update(app: &mut App, message: Message) -> iced::Task<Message> {
    match message {
        Message::Navigate(page) => {
            app.page = page;
            app.notice = None;
        }
        Message::SelectPlatform(platform) => {
            app.platform = platform;
            app.loader = match platform {
                PlatformChoice::Java => LoaderKind::Vanilla,
                PlatformChoice::Horizon => LoaderKind::HorizonContent,
            };
            app.notice = None;
        }
        Message::SelectLoader(loader) => {
            if loader.is_valid_for(app.platform.into()) {
                app.loader = loader;
            }
        }
        Message::SearchChanged(value) => {
            app.search = value;
            app.page = Page::Catalog;
            app.notice = None;
        }
        Message::InstanceNameChanged(value) => app.instance_name = value,
        Message::InstanceVersionChanged(value) => app.instance_version = value,
        Message::CreateInstance => {
            let name = app.instance_name.trim().to_owned();
            let version = app.instance_version.trim().to_owned();

            if name.is_empty() || version.is_empty() {
                app.notice = Some("Укажи название сборки и версию Minecraft.".into());
            } else {
                match store::create_instance(name, version, app.platform.into(), app.loader) {
                    Ok(instance) => {
                        app.data.instances.push(instance);
                        match store::save(&app.data) {
                            Ok(()) => {
                                app.instance_name.clear();
                                app.notice = Some("Сборка создана и сохранена.".into());
                            }
                            Err(error) => {
                                app.notice = Some(format!("Сборка создана в памяти, но не сохранена: {error}"));
                            }
                        }
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
            app.page = Page::Instances;
        }
        Message::LaunchInstance(id) => {
            if let Some(instance) = app.data.instances.iter().find(|item| item.id.0 == id) {
                app.notice = Some(match instance.state {
                    InstanceState::Ready => "Запуск будет подключён после реализации игрового runtime.".into(),
                    _ => "Эта сборка пока не установлена. Установщик Minecraft будет следующим этапом.".into(),
                });
            }
        }
        Message::DeleteInstance(id) => {
            app.data.instances.retain(|instance| instance.id.0 != id);
            app.notice = store::save(&app.data)
                .err()
                .map(|error| format!("Не удалось сохранить удаление: {error}"))
                .or_else(|| Some("Сборка удалена из библиотеки.".into()));
        }
        Message::OfflineNameChanged(value) => app.offline_name = value,
        Message::AddOfflineAccount => {
            match store::create_offline_account(&app.offline_name) {
                Ok(account) => {
                    if app.data.accounts.iter().any(|item| item.id == account.id) {
                        app.notice = Some("Этот offline-профиль уже добавлен.".into());
                    } else {
                        app.data.accounts.push(account);
                        app.offline_name.clear();
                        app.notice = store::save(&app.data)
                            .err()
                            .map(|error| format!("Профиль добавлен, но не сохранён: {error}"))
                            .or_else(|| Some("Offline-профиль добавлен.".into()));
                    }
                }
                Err(error) => app.notice = Some(error),
            }
            app.page = Page::Accounts;
        }
        Message::DeleteAccount(id) => {
            if let Some(account) = app.data.accounts.iter().find(|item| item.id == id) {
                if account.kind == AccountKind::Microsoft {
                    store::remove_refresh_token(&account.id);
                }
            }
            app.data.accounts.retain(|account| account.id != id);
            app.notice = store::save(&app.data)
                .err()
                .map(|error| format!("Не удалось сохранить удаление: {error}"))
                .or_else(|| Some("Аккаунт удалён из GlowstoneCL.".into()));
        }
        Message::MicrosoftClientIdChanged(value) => app.client_id = value,
        Message::SaveMicrosoftClientId => {
            app.data.settings.microsoft_client_id = app.client_id.trim().to_owned();
            app.notice = store::save(&app.data)
                .err()
                .map(|error| format!("Не удалось сохранить Client ID: {error}"))
                .or_else(|| Some("Microsoft Client ID сохранён.".into()));
        }
        Message::StartMicrosoftLogin => {
            if app.client_id.trim().is_empty() {
                app.notice = Some("Сначала добавь Microsoft Client ID и сохрани его.".into());
                return iced::Task::none();
            }
            app.auth_busy = true;
            app.notice = Some("Запрашиваю код входа у Microsoft…".into());
            return iced::Task::perform(
                auth::request_device_code(app.client_id.trim().to_owned()),
                Message::DeviceCodeReady,
            );
        }
        Message::DeviceCodeReady(result) => {
            app.auth_busy = false;
            match result {
                Ok(prompt) => {
                    app.notice = Some("Код готов. Открой страницу Microsoft, подтверди вход и затем проверь авторизацию.".into());
                    app.device_code = Some(prompt);
                }
                Err(error) => app.notice = Some(error),
            }
        }
        Message::OpenMicrosoftVerification => {
            if let Some(prompt) = &app.device_code {
                if let Err(error) = webbrowser::open(&prompt.verification_uri) {
                    app.notice = Some(format!("Не удалось открыть браузер: {error}. Адрес: {}", prompt.verification_uri));
                }
            }
        }
        Message::PollMicrosoftLogin => {
            let Some(prompt) = app.device_code.clone() else {
                app.notice = Some("Сначала запроси код Microsoft.".into());
                return iced::Task::none();
            };
            app.auth_busy = true;
            app.notice = Some("Проверяю подтверждение входа…".into());
            return iced::Task::perform(
                auth::poll_device_code(
                    app.client_id.trim().to_owned(),
                    prompt.device_code,
                    prompt.interval,
                ),
                Message::MicrosoftPollFinished,
            );
        }
        Message::MicrosoftPollFinished(result) => {
            app.auth_busy = false;
            match result {
                Ok(PollResult::Pending(message)) => app.notice = Some(message),
                Ok(PollResult::SignedIn { account, refresh_token }) => {
                    let account_id = account.id.clone();
                    app.data.accounts.retain(|current| current.id != account_id);
                    app.data.accounts.push(account.clone());
                    app.device_code = None;

                    let token_status = match store::store_refresh_token(&account_id, &refresh_token) {
                        Ok(()) => "Токен обновления сохранён в системном хранилище.",
                        Err(_) => "Не удалось сохранить refresh token в системном хранилище. Для следующего запуска может потребоваться повторный вход.",
                    };
                    let save_status = store::save(&app.data);
                    app.notice = Some(match save_status {
                        Ok(()) => format!("Minecraft-профиль {} подключён. {token_status}", account.name),
                        Err(error) => format!("Профиль {} подключён, но список аккаунтов не удалось сохранить: {error}", account.name),
                    });
                }
                Err(error) => app.notice = Some(error),
            }
        }
        Message::Info(message) => app.notice = Some(message.to_owned()),
    }

    iced::Task::none()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let content = row![
        sidebar(app),
        column![
            top_bar(app),
            row![
                scrollable(main_content(app))
                    .height(Length::Fill)
                    .width(Length::Fill),
                scrollable(right_rail(app))
                    .height(Length::Fill)
                    .width(Length::Fixed(278.0)),
            ]
            .spacing(14)
            .height(Length::Fill)
            .width(Length::Fill),
        ]
        .spacing(14)
        .height(Length::Fill)
        .width(Length::Fill),
    ]
    .spacing(14)
    .height(Length::Fill);

    stack![
        canvas(GlowBackdrop)
            .width(Length::Fill)
            .height(Length::Fill),
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(12),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .into()
}

#[derive(Debug, Clone, Copy)]
struct GlowBackdrop;

impl<Message> canvas::Program<Message> for GlowBackdrop {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(iced::Point::ORIGIN, bounds.size());
        frame.fill(&background, BG);
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.72, bounds.height * 0.12),
            440.0,
            Color::from_rgb8(45, 89, 174),
        );
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.42, bounds.height * 0.95),
            360.0,
            Color::from_rgb8(81, 61, 157),
        );
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.98, bounds.height * 0.72),
            280.0,
            Color::from_rgb8(38, 121, 166),
        );
        vec![frame.into_geometry()]
    }
}

fn draw_glow(frame: &mut canvas::Frame, center: iced::Point, radius: f32, color: Color) {
    for layer in 0..24 {
        let amount = layer as f32 / 23.0;
        let current_radius = radius * (1.0 - amount * 0.94);
        let alpha = 0.001 + (1.0 - amount).powf(2.0) * 0.008;
        let circle = canvas::Path::circle(center, current_radius);
        frame.fill(&circle, Color::from_rgba(color.r, color.g, color.b, alpha));
    }
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let brand = container(
        row![
            container(icon("diamond", CYAN, 24.0))
                .width(48)
                .height(48)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT)),
            column![
                text("GlowstoneCL").size(16).color(TEXT),
                text("UNIVERSAL LAUNCHER").size(8).color(CYAN),
            ]
            .spacing(2),
        ]
        .align_y(Alignment::Center)
        .spacing(10),
    )
    .padding(12)
    .width(Length::Fill)
    .style(|_| glass_style(Color::from_rgba(0.08, 0.12, 0.21, 0.68), EDGE));

    let library = column![
        section_label("БИБЛИОТЕКА"),
        nav_button("home", "Главная", Page::Home, app.page),
        nav_button("instances", "Мои сборки", Page::Instances, app.page),
    ]
    .spacing(6);

    let discover = column![
        section_label("ОТКРЫТЬ"),
        nav_button("puzzle", "Каталог модов", Page::Catalog, app.page),
        nav_button("package", "Модпаки", Page::Modpacks, app.page),
    ]
    .spacing(6);

    let system = column![
        section_label("СИСТЕМА"),
        nav_button("download", "Загрузки", Page::Downloads, app.page),
        nav_button("user", "Аккаунты", Page::Accounts, app.page),
        nav_button("settings", "Настройки", Page::Settings, app.page),
    ]
    .spacing(6);

    let platforms = column![
        section_label("ПЛАТФОРМЫ"),
        platform_nav("java", "Minecraft Java", PlatformChoice::Java, app.platform),
        platform_nav("horizon", "Minecraft Horizon", PlatformChoice::Horizon, app.platform),
    ]
    .spacing(6);

    let current_account = app
        .data
        .accounts
        .first()
        .map(|account| account.name.as_str())
        .unwrap_or("Гость");
    let profile_subtitle = if app.data.accounts.is_empty() {
        "Не авторизован"
    } else {
        "Аккаунт подключён"
    };

    let profile = container(
        row![
            container(icon("user", CYAN, 18.0))
                .width(38)
                .height(38)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(CYAN_DARK, EDGE)),
            column![
                text(current_account.to_owned()).size(10).color(TEXT),
                row![
                    icon("check", if app.data.accounts.is_empty() { SUBTLE } else { GREEN }, 10.0),
                    text(profile_subtitle).size(9).color(MUTED),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
            icon("chevron-down", MUTED, 14.0),
        ]
        .align_y(Alignment::Center)
        .spacing(9),
    )
    .padding(10)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL_LIGHT, EDGE));

    container(
        column![
            brand,
            Space::new().height(23),
            library,
            Space::new().height(14),
            discover,
            Space::new().height(14),
            system,
            Space::new().height(14),
            rule::horizontal(1).style(|_| rule::Style {
                color: DIVIDER,
                radius: iced::border::Radius::default(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }),
            Space::new().height(8),
            platforms,
            Space::new().height(Length::Fill),
            profile,
        ]
        .spacing(0)
        .height(Length::Fill),
    )
    .width(Length::Fixed(238.0))
    .height(Length::Fill)
    .padding(12)
    .style(|_| glass_style(SIDEBAR, EDGE))
    .into()
}

fn section_label(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(9).color(SUBTLE))
        .padding([5, 10])
        .into()
}

fn nav_button(
    icon_name: &'static str,
    label: &'static str,
    page: Page,
    current: Page,
) -> Element<'static, Message> {
    let active = page == current;
    let background = if active {
        Color::from_rgba(0.19, 0.42, 0.62, 0.58)
    } else {
        Color::TRANSPARENT
    };
    let edge = if active { EDGE_BRIGHT } else { Color::TRANSPARENT };
    let foreground = if active { TEXT } else { MUTED };
    let icon_color = if active { CYAN } else { SUBTLE };

    button(
        row![
            icon(icon_name, icon_color, 17.0),
            text(label).size(11).color(foreground),
        ]
        .spacing(13)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Navigate(page))
    .width(Length::Fill)
    .padding([10, 12])
    .style(move |theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(background.into());
        style.text_color = foreground;
        style.border = Border::default().rounded(11).width(1).color(edge);
        style
    })
    .into()
}

fn platform_nav(
    icon_name: &'static str,
    label: &'static str,
    platform: PlatformChoice,
    current: PlatformChoice,
) -> Element<'static, Message> {
    let active = platform == current;
    let foreground = if active { TEXT } else { MUTED };
    let accent = if platform == PlatformChoice::Java { GREEN } else { ORANGE };

    button(
        row![
            icon(icon_name, accent, 17.0),
            text(label).size(10).color(foreground),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .on_press(Message::SelectPlatform(platform))
    .width(Length::Fill)
    .padding([8, 10])
    .style(move |theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(
            if active {
                Color::from_rgba(0.15, 0.24, 0.33, 0.48)
            } else {
                Color::TRANSPARENT
            }
            .into(),
        );
        style.text_color = foreground;
        style.border = Border::default().rounded(9);
        style
    })
    .into()
}

fn top_bar(app: &App) -> Element<'_, Message> {
    let search = container(
        row![
            icon("search", MUTED, 18.0),
            text_input("Поиск по сборкам, модам и модпакам...", &app.search)
                .on_input(Message::SearchChanged)
                .size(12)
                .padding([10, 8])
                .width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([2, 14])
    .width(Length::Fill)
    .style(|_| glass_style(Color::from_rgba(0.18, 0.22, 0.34, 0.68), EDGE));

    row![
        search,
        round_action("community", Message::Info("Ссылку на сообщество добавим позже.")),
        round_action("globe", Message::Navigate(Page::Catalog)),
        round_action("settings", Message::Navigate(Page::Settings)),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([3, 2])
    .into()
}

fn round_action(icon_name: &'static str, message: Message) -> Element<'static, Message> {
    button(icon(icon_name, TEXT, 16.0))
        .on_press(message)
        .width(40)
        .height(40)
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(PANEL_LIGHT.into());
            style.text_color = TEXT;
            style.border = Border::default().rounded(20).width(1).color(EDGE);
            style
        })
        .into()
}

fn main_content(app: &App) -> Element<'_, Message> {
    match app.page {
        Page::Home => home_page(app),
        Page::Instances => instances_page(app),
        Page::Accounts => accounts_page(app),
        Page::Settings => settings_page(app),
        Page::Catalog => catalog_page(),
        Page::Modpacks => modpacks_page(),
        Page::Downloads => downloads_page(),
    }
}

fn home_page(app: &App) -> Element<'_, Message> {
    let hero = hero_banner(app);
    let platforms = row![
        platform_card(app.art.clone(), PlatformChoice::Java, app.platform == PlatformChoice::Java),
        platform_card(app.art.clone(), PlatformChoice::Horizon, app.platform == PlatformChoice::Horizon),
    ]
    .spacing(14)
    .width(Length::Fill);

    let mut build_cards = row![]
        .spacing(11)
        .width(Length::Fill);
    for (index, instance) in app.data.instances.iter().take(4).enumerate() {
        build_cards = build_cards.push(pack_card(
            app.art.clone(),
            instance.name.clone(),
            format!("{} · {}", instance.game_version, loader_label(instance.loader)),
            index,
        ));
    }

    let builds_content: Element<'_, Message> = if app.data.instances.is_empty() {
        container(
            row![
                icon("instances", CYAN, 25.0),
                column![
                    text("Пока нет игровых сборок").size(12).color(TEXT),
                    text("Создай первый профиль Java или Horizon — он появится здесь.")
                        .size(9)
                        .color(MUTED),
                ]
                .spacing(3),
                Space::new().width(Length::Fill),
                tonal_button("Создать", Message::Navigate(Page::Instances)),
            ]
            .align_y(Alignment::Center)
            .spacing(12),
        )
        .padding(15)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE))
        .into()
    } else {
        build_cards.into()
    };

    let builds = column![
        row![
            column![
                text("Твои сборки").size(16).color(TEXT),
                text("Недавно созданные игровые профили").size(9).color(MUTED),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            button(text("Все сборки").size(10).color(CYAN))
                .on_press(Message::Navigate(Page::Instances))
                .style(button::text),
        ]
        .align_y(Alignment::Center),
        builds_content,
    ]
    .spacing(10);

    let mut content = column![
        hero,
        platforms,
        builds,
    ]
    .spacing(16)
    .width(Length::Fill);

    if let Some(notice) = &app.notice {
        content = content.push(
            container(text(notice).size(10).color(TEXT))
                .padding([10, 14])
                .width(Length::Fill)
                .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT)),
        );
    }

    content.into()
}

fn hero_banner(app: &App) -> Element<'_, Message> {
    let overlay = container(
        row![
            column![
                text("GlowstoneCL").size(34).color(TEXT),
                text("Твой Minecraft. Твои правила.").size(16).color(TEXT),
                text("Java Edition и Horizon — в одной библиотеке.").size(11).color(MUTED),
                Space::new().height(12),
                row![
                    primary_button("Играть", Message::Info("Выбери установленную сборку в разделе «Мои сборки»."))
                        ,
                    round_action("chevron-down", Message::Navigate(Page::Instances)),
                    Space::new().width(Length::Fill),
                    tonal_button("Настроить сборки", Message::Navigate(Page::Instances)),
                ]
                .spacing(9)
                .align_y(Alignment::Center),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .padding(25),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
        background: Some(Color::from_rgba(0.025, 0.045, 0.085, 0.37).into()),
        border: Border::default().rounded(18).width(1).color(EDGE),
        text_color: Some(TEXT),
        ..Default::default()
    });

    container(
        stack![
            image::Image::new(app.art.clone())
                .width(Length::Fill)
                .height(Length::Fixed(265.0))
                .content_fit(ContentFit::Cover)
                .border_radius(18.0),
            overlay,
        ]
        .width(Length::Fill)
        .height(Length::Fixed(265.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(265.0))
    .style(|_| glass_style(PANEL, EDGE_BRIGHT))
    .into()
}

fn platform_card(art: image::Handle, platform: PlatformChoice, selected: bool) -> Element<'static, Message> {
    let (title, subtitle, description, tint, accent, tags, crop) = match platform {
        PlatformChoice::Java => (
            "Minecraft Java",
            "Классическая Java Edition",
            "От Vanilla до крупных модпаков",
            Color::from_rgba(0.035, 0.19, 0.09, 0.64),
            GREEN,
            ["Forge", "Fabric", "NeoForge", "Quilt"],
            Rectangle { x: 0, y: 115, width: 700, height: 255 },
        ),
        PlatformChoice::Horizon => (
            "Minecraft Horizon",
            "Нативный Rust runtime",
            "Отдельная экосистема контентных и native-модов",
            Color::from_rgba(0.23, 0.10, 0.045, 0.66),
            ORANGE,
            ["HMOD", "HNMOD", "Content", "Native"],
            Rectangle { x: 190, y: 80, width: 510, height: 270 },
        ),
    };

    let mut chips = row![];
    for tag in tags {
        chips = chips.push(tag_chip(tag));
    }
    chips = chips.spacing(6);

    let foreground = container(
        row![
            column![
                text(title).size(15).color(TEXT),
                text(subtitle).size(10).color(TEXT),
                text(description).size(9).color(MUTED),
                Space::new().height(5),
                chips,
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            button(icon("arrow-right", TEXT, 18.0))
                .on_press(Message::SelectPlatform(platform))
                .width(40)
                .height(40)
                .style(|theme, status| {
                    let mut style = button::secondary(theme, status);
                    style.background = Some(Color::from_rgba(0.12, 0.17, 0.24, 0.82).into());
                    style.text_color = TEXT;
                    style.border = Border::default().rounded(20).color(EDGE);
                    style
                }),
        ]
        .align_y(Alignment::Center)
        .spacing(12)
        .padding(17),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style {
        background: Some(tint.into()),
        border: Border::default()
            .rounded(16)
            .width(if selected { 1.5 } else { 1.0 })
            .color(if selected { accent } else { EDGE }),
        text_color: Some(TEXT),
        ..Default::default()
    });

    container(
        stack![
            image::Image::new(art)
                .crop(crop)
                .width(Length::Fill)
                .height(Length::Fixed(178.0))
                .content_fit(ContentFit::Cover)
                .border_radius(16.0),
            foreground,
        ]
        .width(Length::Fill)
        .height(Length::Fixed(178.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(178.0))
    .style(move |_| glass_style(
        if platform == PlatformChoice::Java {
            Color::from_rgba(0.04, 0.15, 0.07, 0.55)
        } else {
            Color::from_rgba(0.18, 0.09, 0.06, 0.55)
        },
        if selected { accent } else { EDGE },
    ))
    .into()
}

fn tag_chip(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(8).color(TEXT))
        .padding([5, 8])
        .style(|_| glass_style(Color::from_rgba(0.06, 0.09, 0.14, 0.72), EDGE))
        .into()
}

fn pack_card(
    art: image::Handle,
    title: String,
    subtitle: String,
    index: usize,
) -> Element<'static, Message> {
    let crops = [
        Rectangle { x: 0, y: 20, width: 300, height: 220 },
        Rectangle { x: 220, y: 0, width: 320, height: 240 },
        Rectangle { x: 360, y: 60, width: 330, height: 220 },
        Rectangle { x: 80, y: 160, width: 400, height: 210 },
    ];
    let crop = crops[index.min(3)];

    container(
        stack![
            image::Image::new(art)
                .crop(crop)
                .width(Length::Fill)
                .height(Length::Fixed(140.0))
                .content_fit(ContentFit::Cover)
                .border_radius(13.0),
            container(
                column![
                    Space::new().height(Length::Fill),
                    text(title).size(10).color(TEXT),
                    text(subtitle).size(8).color(MUTED),
                ]
                .spacing(3)
                .padding(10),
            )
            .width(Length::Fill)
            .height(Length::Fixed(140.0))
            .style(|_| container::Style {
                background: Some(Color::from_rgba(0.015, 0.025, 0.045, 0.55).into()),
                border: Border::default().rounded(13).color(EDGE),
                text_color: Some(TEXT),
                ..Default::default()
            }),
        ]
        .width(Length::Fill)
        .height(Length::Fixed(140.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(140.0))
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn right_rail(app: &App) -> Element<'_, Message> {
    let news = container(
        column![
            row![
                text("Новости").size(14).color(TEXT),
                Space::new().width(Length::Fill),
                button(text("Все").size(9).color(CYAN))
                    .on_press(Message::Info("Новостные ленты появятся после подключения источников."))
                    .style(button::text),
            ]
            .align_y(Alignment::Center),
            rule::horizontal(1).style(|_| rule::Style {
                color: DIVIDER,
                radius: iced::border::Radius::default(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }),
            news_item("minecraft", "Minecraft Java", "Новости и релизы игры", CYAN),
            news_item("forge", "Forge и NeoForge", "Обновления загрузчиков", ORANGE),
            news_item("horizon", "Horizon", "Новости Rust runtime", PURPLE),
        ]
        .spacing(13),
    )
    .padding(15)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let last_instance = app.data.instances.last().map(|item| item.name.clone()).unwrap_or_else(|| "Пока нет сборок".into());
    let last_account = app.data.accounts.first().map(|item| item.name.clone()).unwrap_or_else(|| "Не авторизован".into());

    let quick = container(
        column![
            text("Быстрый запуск").size(14).color(TEXT),
            quick_item("instances", "Последняя сборка", last_instance, Page::Instances),
            quick_item("user", "Последний профиль", last_account, Page::Accounts),
        ]
        .spacing(13),
    )
    .padding(15)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let status = container(
        row![
            container(icon("check", GREEN, 16.0))
                .width(36)
                .height(36)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(Color::from_rgba(0.04, 0.24, 0.12, 0.72), EDGE)),
            column![
                text("Лаунчер готов").size(10).color(TEXT),
                text(format!("{} сборок · {} аккаунтов", app.data.instances.len(), app.data.accounts.len()))
                    .size(8)
                    .color(MUTED),
            ]
            .spacing(3),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .padding(14)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    column![news, quick, status]
        .spacing(13)
        .width(Length::Fill)
        .into()
}

fn news_item(
    icon_name: &'static str,
    title: &'static str,
    description: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    row![
        container(icon(icon_name, accent, 18.0))
            .width(38)
            .height(38)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| glass_style(
                Color::from_rgba(accent.r, accent.g, accent.b, 0.14),
                EDGE,
            )),
        column![
            text(title).size(10).color(TEXT),
            text(description).size(9).color(MUTED),
            text("Источник будет подключён").size(8).color(SUBTLE),
        ]
        .spacing(4),
    ]
    .spacing(10)
    .align_y(Alignment::Start)
    .into()
}

fn quick_item(
    icon_name: &'static str,
    title: &'static str,
    subtitle: String,
    page: Page,
) -> Element<'static, Message> {
    button(
        row![
            container(icon(icon_name, CYAN, 16.0))
                .width(34)
                .height(34)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(CYAN_DARK, EDGE)),
            column![
                text(title).size(9).color(MUTED),
                text(subtitle).size(10).color(TEXT),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
            icon("chevron-right", CYAN, 13.0),
        ]
        .align_y(Alignment::Center)
        .spacing(9),
    )
    .on_press(Message::Navigate(page))
    .width(Length::Fill)
    .padding(4)
    .style(|theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(Color::TRANSPARENT.into());
        style.text_color = TEXT;
        style
    })
    .into()
}

fn instances_page(app: &App) -> Element<'_, Message> {
    let loaders: &[LoaderKind] = match app.platform {
        PlatformChoice::Java => &[
            LoaderKind::Vanilla,
            LoaderKind::Fabric,
            LoaderKind::Forge,
            LoaderKind::NeoForge,
            LoaderKind::Quilt,
        ],
        PlatformChoice::Horizon => &[
            LoaderKind::HorizonContent,
            LoaderKind::HorizonNative,
        ],
    };

    let mut loader_buttons = row![];
    for loader in loaders {
        loader_buttons = loader_buttons.push(loader_button(*loader, app.loader));
    }
    loader_buttons = loader_buttons.spacing(7);

    let form = container(
        column![
            row![
                text("Создать сборку").size(16).color(TEXT),
                Space::new().width(Length::Fill),
                status_chip("НОВЫЙ ПРОФИЛЬ", CYAN_DARK, CYAN),
            ]
            .align_y(Alignment::Center),
            text("Название").size(9).color(MUTED),
            text_input("Например, My Survival", &app.instance_name)
                .on_input(Message::InstanceNameChanged)
                .padding(10)
                .size(11),
            text("Версия игры").size(9).color(MUTED),
            text_input("Например, 1.21.1 или stable", &app.instance_version)
                .on_input(Message::InstanceVersionChanged)
                .padding(10)
                .size(11),
            text("Платформа").size(9).color(MUTED),
            row![
                choice_button("Minecraft Java", app.platform == PlatformChoice::Java, Message::SelectPlatform(PlatformChoice::Java)),
                choice_button("Horizon", app.platform == PlatformChoice::Horizon, Message::SelectPlatform(PlatformChoice::Horizon)),
            ]
            .spacing(8),
            text("Загрузчик").size(9).color(MUTED),
            loader_buttons,
            primary_button("Создать сборку", Message::CreateInstance),
            text("Пока создаётся профиль и его настройки. Загрузка файлов игры и запуск будут подключены следующим этапом.")
                .size(9)
                .color(SUBTLE),
        ]
        .spacing(8),
    )
    .padding(18)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let mut list = column![
        row![
            column![
                text("Твоя библиотека").size(16).color(TEXT),
                text(format!("{} игровых профилей", app.data.instances.len())).size(9).color(MUTED),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10)
    .width(Length::Fill);

    if app.data.instances.is_empty() {
        list = list.push(
            container(
                column![
                    icon("instances", SUBTLE, 28.0),
                    text("Пока нет сборок").size(13).color(TEXT),
                    text("Создай профиль выше. Java и Horizon будут храниться независимо.")
                        .size(9)
                        .color(MUTED),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .padding(24)
            .width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE)),
        );
    } else {
        for instance in &app.data.instances {
            list = list.push(instance_card(instance));
        }
    }

    column![form, list]
        .spacing(16)
        .width(Length::Fill)
        .into()
}

fn loader_button(loader: LoaderKind, current: LoaderKind) -> Element<'static, Message> {
    let active = loader == current;
    button(text(loader_label(loader)).size(9).color(if active { TEXT } else { MUTED }))
        .on_press(Message::SelectLoader(loader))
        .padding([8, 10])
        .style(move |theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(if active { CYAN_DARK } else { PANEL_LIGHT }.into());
            style.text_color = if active { CYAN } else { MUTED };
            style.border = Border::default().rounded(10).width(1).color(if active { EDGE_BRIGHT } else { EDGE });
            style
        })
        .into()
}

fn choice_button(label: &'static str, active: bool, message: Message) -> Element<'static, Message> {
    button(text(label).size(10).color(if active { TEXT } else { MUTED }))
        .on_press(message)
        .padding([9, 12])
        .style(move |theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(if active { CYAN_DARK } else { PANEL_LIGHT }.into());
            style.text_color = if active { CYAN } else { MUTED };
            style.border = Border::default().rounded(11).width(1).color(if active { EDGE_BRIGHT } else { EDGE });
            style
        })
        .into()
}

fn instance_card(instance: &GameInstance) -> Element<'static, Message> {
    let platform = match instance.platform {
        GamePlatform::Java => "Minecraft Java",
        GamePlatform::Horizon => "Minecraft Horizon",
    };
    let state = match instance.state {
        InstanceState::NotInstalled => "Не установлена",
        InstanceState::Ready => "Готова",
        InstanceState::Broken => "Требует проверки",
        InstanceState::Updating => "Обновляется",
    };
    let account_id = instance.id.0.clone();

    container(
        row![
            container(icon(
                if instance.platform == GamePlatform::Java { "java" } else { "horizon" },
                if instance.platform == GamePlatform::Java { GREEN } else { ORANGE },
                20.0,
            ))
            .width(44)
            .height(44)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_| glass_style(PANEL_LIGHT, EDGE)),
            column![
                text(instance.name.clone()).size(12).color(TEXT),
                text(format!("{} · {} · {}", platform, instance.game_version, loader_label(instance.loader)))
                    .size(9)
                    .color(MUTED),
                text(state).size(8).color(SUBTLE),
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            button(icon("play", CYAN, 15.0))
                .on_press(Message::LaunchInstance(account_id.clone()))
                .width(36)
                .height(36)
                .style(|theme, status| {
                    let mut style = button::secondary(theme, status);
                    style.background = Some(CYAN_DARK.into());
                    style.text_color = CYAN;
                    style.border = Border::default().rounded(18).color(EDGE_BRIGHT);
                    style
                }),
            button(icon("trash", RED, 14.0))
                .on_press(Message::DeleteInstance(account_id))
                .width(34)
                .height(34)
                .style(button::text),
        ]
        .spacing(11)
        .align_y(Alignment::Center),
    )
    .padding(13)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn accounts_page(app: &App) -> Element<'_, Message> {
    let offline_form = container(
        column![
            row![
                icon("user", PURPLE, 18.0),
                text("Локальный offline-профиль").size(14).color(TEXT),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            text("Для одиночной игры и серверов с отключённой online-проверкой. Не даёт доступ к официальной авторизации.")
                .size(9)
                .color(MUTED),
            row![
                text_input("Игровой ник (3–16 символов)", &app.offline_name)
                    .on_input(Message::OfflineNameChanged)
                    .padding(10)
                    .size(10)
                    .width(Length::Fill),
                primary_button("Добавить", Message::AddOfflineAccount),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding(16)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let microsoft_form = container(
        column![
            row![
                icon("microsoft", CYAN, 18.0),
                text("Microsoft account").size(14).color(TEXT),
                Space::new().width(Length::Fill),
                status_chip("ОФИЦИАЛЬНЫЙ ВХОД", CYAN_DARK, CYAN),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text("GlowstoneCL использует Device Code Flow. Нужен Client ID зарегистрированного public-client приложения Microsoft.")
                .size(9)
                .color(MUTED),
            text("Microsoft Client ID").size(9).color(MUTED),
            row![
                text_input("Вставь Application (client) ID", &app.client_id)
                    .on_input(Message::MicrosoftClientIdChanged)
                    .padding(10)
                    .size(10)
                    .width(Length::Fill),
                quiet_button("Сохранить", Message::SaveMicrosoftClientId),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                primary_button(
                    if app.auth_busy { "Подключение..." } else { "Войти через Microsoft" },
                    Message::StartMicrosoftLogin,
                ),
                if app.auth_busy {
                    icon("loader", CYAN, 16.0)
                } else {
                    icon("shield", GREEN, 16.0)
                },
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            microsoft_device_code(app),
            text("Client ID не является секретом. Client Secret в настольном приложении не используется и не должен встраиваться в клиент.")
                .size(8)
                .color(SUBTLE),
        ]
        .spacing(10),
    )
    .padding(16)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let mut account_list = column![
        row![
            column![
                text("Добавленные аккаунты").size(15).color(TEXT),
                text(format!("{} профилей", app.data.accounts.len())).size(9).color(MUTED),
            ]
            .spacing(2),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(9)
    .width(Length::Fill);

    if app.data.accounts.is_empty() {
        account_list = account_list.push(
            container(
                column![
                    icon("user", SUBTLE, 26.0),
                    text("Аккаунтов пока нет").size(12).color(TEXT),
                    text("Добавь offline-профиль или войди через Microsoft.")
                        .size(9)
                        .color(MUTED),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .padding(20)
            .width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE)),
        );
    } else {
        for account in &app.data.accounts {
            account_list = account_list.push(account_card(account));
        }
    }

    column![microsoft_form, offline_form, account_list]
        .spacing(14)
        .width(Length::Fill)
        .into()
}

fn microsoft_device_code(app: &App) -> Element<'_, Message> {
    let Some(prompt) = &app.device_code else {
        return Space::new().height(0).into();
    };

    container(
        column![
            text("Подтверди вход").size(12).color(TEXT),
            text(&prompt.message).size(9).color(MUTED),
            text(&prompt.user_code).size(26).color(CYAN),
            row![
                quiet_button("Открыть страницу Microsoft", Message::OpenMicrosoftVerification),
                primary_button(
                    if app.auth_busy { "Проверяю..." } else { "Я подтвердил вход" },
                    Message::PollMicrosoftLogin,
                ),
            ]
            .spacing(8),
            text(&prompt.verification_uri).size(9).color(CYAN),
        ]
        .spacing(8)
        .align_x(Alignment::Start),
    )
    .padding(14)
    .width(Length::Fill)
    .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT))
    .into()
}

fn account_card(account: &AccountProfile) -> Element<'static, Message> {
    let (kind_label, accent) = match account.kind {
        AccountKind::Offline => ("OFFLINE", PURPLE),
        AccountKind::Microsoft => ("MICROSOFT", CYAN),
    };
    let id = account.id.clone();

    container(
        row![
            container(icon("user", accent, 18.0))
                .width(42)
                .height(42)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| glass_style(
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.14),
                    EDGE,
                )),
            column![
                text(account.name.clone()).size(11).color(TEXT),
                text(format!("{} · {}", kind_label, account.uuid)).size(8).color(MUTED),
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            button(icon("trash", RED, 14.0))
                .on_press(Message::DeleteAccount(id))
                .width(34)
                .height(34)
                .style(button::text),
        ]
        .align_y(Alignment::Center)
        .spacing(10),
    )
    .padding(12)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn settings_page(app: &App) -> Element<'_, Message> {
    let data_path = if cfg!(target_os = "linux") {
        "~/.local/share/GlowstoneCL/state.json"
    } else {
        "User data directory / GlowstoneCL / state.json"
    };

    column![
        settings_card("ВНЕШНИЙ ВИД", "Шрифт интерфейса", "Используется системный Noto Sans с поддержкой кириллицы. Если он не установлен, система выберет резервный sans-serif."),
        settings_card("LINUX DISPLAY", "Нативный Wayland и X11", "GlowstoneCL поддерживает оба backend. В Wayland-сессии можно запускать приложение напрямую через Wayland без XWayland."),
        settings_card("ХРАНЕНИЕ", "Профили и сборки", data_path),
        settings_card("БЕЗОПАСНОСТЬ", "Microsoft refresh token", "Токены сохраняются через системное хранилище паролей. Если Secret Service недоступен, приложение покажет предупреждение."),
        container(
            column![
                text("Данные сейчас").size(12).color(TEXT),
                text(format!("{} сборок", app.data.instances.len())).size(10).color(MUTED),
                text(format!("{} аккаунтов", app.data.accounts.len())).size(10).color(MUTED),
            ]
            .spacing(6),
        )
        .padding(16)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE)),
    ]
    .spacing(12)
    .into()
}

fn settings_card(
    eyebrow: &'static str,
    title: &'static str,
    description: &'static str,
) -> Element<'static, Message> {
    container(
        column![
            text(eyebrow).size(9).color(CYAN),
            text(title).size(14).color(TEXT),
            text(description).size(9).color(MUTED),
        ]
        .spacing(7),
    )
    .padding(16)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn catalog_page() -> Element<'static, Message> {
    column![
        section_header("ИСТОЧНИКИ КОНТЕНТА", "Каталоги будут фильтроваться по платформе и версии игры."),
        source_card("MODRINTH", "Моды, модпаки, шейдеры и ресурспаки", "API ещё не подключён", CYAN),
        source_card("CURSEFORGE", "Моды и готовые сборки", "Требуется официальная API-интеграция", PURPLE),
    ]
    .spacing(14)
    .into()
}

fn source_card(
    name: &'static str,
    description: &'static str,
    status: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    container(
        column![
            text(name).size(11).color(accent),
            text(description).size(11).color(TEXT),
            status_chip(status, PANEL_LIGHT, MUTED),
        ]
        .spacing(8),
    )
    .padding(17)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn modpacks_page() -> Element<'static, Message> {
    column![
        section_header("МОДПАКИ", "Импортируй готовые сборки или управляй собственными."),
        container(
            column![
                icon("package", PURPLE, 26.0),
                text("Модпаки появятся здесь").size(15).color(TEXT),
                text("Планируется импорт Modrinth .mrpack и CurseForge ZIP. Установка будет создавать отдельный игровой экземпляр.")
                    .size(10)
                    .color(MUTED),
                primary_button("Перейти к моим сборкам", Message::Navigate(Page::Instances)),
            ]
            .spacing(9),
        )
        .padding(20)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE)),
    ]
    .spacing(14)
    .into()
}

fn downloads_page() -> Element<'static, Message> {
    column![
        section_header("ОЧЕРЕДЬ ЗАДАЧ", "Загрузка игры, библиотек, модов и обновлений."),
        container(
            column![
                icon("download", CYAN, 26.0),
                text("Нет активных загрузок").size(15).color(TEXT),
                text("Когда будет подключён установщик, здесь появятся прогресс, скорость и проверка целостности.")
                    .size(10)
                    .color(MUTED),
            ]
            .spacing(9),
        )
        .padding(20)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE)),
    ]
    .spacing(14)
    .into()
}

fn section_header(title: &'static str, subtitle: &'static str) -> Element<'static, Message> {
    row![
        column![
            text(title).size(9).color(CYAN),
            text(subtitle).size(11).color(MUTED),
        ]
        .spacing(3),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn status_chip(
    label: &'static str,
    background: Color,
    foreground: Color,
) -> Element<'static, Message> {
    container(text(label).size(8).color(foreground))
        .padding([6, 9])
        .style(move |_| glass_style(background, EDGE))
        .into()
}

fn primary_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(10))
        .on_press(message)
        .padding([10, 15])
        .style(|theme, status| {
            let mut style = button::primary(theme, status);
            style.background = Some(CYAN.into());
            style.text_color = Color::from_rgb8(8, 24, 34);
            style.border = Border::default().rounded(11).width(1).color(CYAN);
            style.shadow = Shadow {
                color: Color::from_rgba(0.1, 0.7, 0.95, 0.2),
                offset: Vector::new(0.0, 3.0),
                blur_radius: 12.0,
            };
            style
        })
        .into()
}

fn tonal_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(9).color(TEXT))
        .on_press(message)
        .padding([10, 14])
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(Color::from_rgba(0.14, 0.20, 0.31, 0.82).into());
            style.text_color = TEXT;
            style.border = Border::default().rounded(20).width(1).color(EDGE);
            style
        })
        .into()
}

fn quiet_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(9).color(TEXT))
        .on_press(message)
        .padding([10, 14])
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(PANEL_LIGHT.into());
            style.text_color = TEXT;
            style.border = Border::default().rounded(11).width(1).color(EDGE);
            style
        })
        .into()
}

fn loader_label(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Vanilla => "Vanilla",
        LoaderKind::Forge => "Forge",
        LoaderKind::NeoForge => "NeoForge",
        LoaderKind::Fabric => "Fabric",
        LoaderKind::Quilt => "Quilt",
        LoaderKind::HorizonContent => "Horizon Content",
        LoaderKind::HorizonNative => "Horizon Native",
    }
}

fn icon(
    name: &'static str,
    color: Color,
    size: f32,
) -> Element<'static, Message> {
    let path = match name {
        "diamond" => "M12 2 21 9 12 22 3 9 12 2Z M3 9h18 M8 9l4 13 4-13 M8 9l4-7 4 7",
        "home" => "M3 10.5 12 3l9 7.5V21h-6v-6H9v6H3V10.5Z",
        "instances" => "M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z",
        "puzzle" => "M8 4a2 2 0 1 1 4 0v2h4v4h2a2 2 0 1 1 0 4h-2v4h-4v-2a2 2 0 1 0-4 0v2H4v-4h2a2 2 0 1 0 0-4H4V6h4V4Z",
        "package" => "m12 3 9 5v8l-9 5-9-5V8l9-5Z M3 8l9 5 9-5 M12 13v8 M7.5 5.5l9 5",
        "download" => "M12 3v12m0 0 4-4m-4 4-4-4 M4 18v3h16v-3",
        "user" => "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z M4 21a8 8 0 0 1 16 0",
        "settings" => "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-1.8 3.1-.2-.1a1.7 1.7 0 0 0-1.9.2l-.1.1h-3.6l-.1-.2a1.7 1.7 0 0 0-1.6-1 1.7 1.7 0 0 0-1.1.4l-.2.1-3.1-1.8.1-.2a1.7 1.7 0 0 0-.2-1.9l-.1-.1v-3.6l.2-.1a1.7 1.7 0 0 0 .6-2.7l-.1-.2 1.8-3.1.2.1a1.7 1.7 0 0 0 1.9-.2l.1-.1h3.6l.1.2a1.7 1.7 0 0 0 2.7.6l.2-.1 3.1 1.8-.1.2a1.7 1.7 0 0 0 .2 1.9l.1.1V15Z",
        "search" => "M10.5 18a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15Z M16 16l5 5",
        "community" => "M16 21v-2a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v2 M9.5 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z M20 8v6 M17 11h6",
        "globe" => "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z M2 12h20 M12 2a15 15 0 0 1 0 20 M12 2a15 15 0 0 0 0 20",
        "chevron-down" => "m6 9 6 6 6-6",
        "chevron-right" => "m9 18 6-6-6-6",
        "arrow-right" => "M4 12h16 M13 5l7 7-7 7",
        "play" => "m8 5 12 7-12 7V5Z",
        "plus" => "M12 4v16 M4 12h16",
        "trash" => "M4 7h16 M10 11v6 M14 11v6 M6 7l1 14h10l1-14 M9 7V4h6v3",
        "check" => "m5 12 4 4L19 6",
        "shield" => "M12 3 20 6v5c0 5-3.5 8.5-8 10-4.5-1.5-8-5-8-10V6l8-3Z M9 12l2 2 4-4",
        "microsoft" => "M3 3h8v8H3z M13 3h8v8h-8z M3 13h8v8H3z M13 13h8v8h-8z",
        "loader" => "M20 12a8 8 0 1 1-2.3-5.7",
        "forge" => "M12 2c2 4-2 5 0 8 1-2 4-3 4-6 4 5 5 9 2 13a8 8 0 0 1-12 0c-3-4-1-9 3-12 0 4 1 5 3 6 2-3-1-5 0-9Z",
        "horizon" => "M3 18 8 6l4 8 4-8 5 12 M3 18h18",
        _ => "M4 4h16v16H4z",
    };

    let stroke = format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
    );
    let markup = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{stroke}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="{path}"/></svg>"#
    );

    svg::Svg::new(svg::Handle::from_memory(markup.into_bytes()))
        .width(size)
        .height(size)
        .into()
}

fn glass_style(background: Color, edge: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: Border::default().rounded(16).width(1).color(edge),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.24),
            offset: Vector::new(0.0, 7.0),
            blur_radius: 20.0,
        },
        text_color: Some(TEXT),
        ..Default::default()
    }
}
